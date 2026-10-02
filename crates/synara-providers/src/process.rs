use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{Mutex, broadcast},
};
use uuid::Uuid;

use super::{ProviderAdapter, ProviderMetadata, ProviderRuntimeEvent};
use synara_process::{ManagedProcess, ProcessSpec};

pub struct CliProvider {
    meta: ProviderMetadata,
    program: String,
    sessions: Arc<Mutex<HashMap<String, ManagedProcess>>>,
    events: broadcast::Sender<ProviderRuntimeEvent>,
}

impl CliProvider {
    pub fn new(kind: &str, name: &str, program: &str) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            meta: ProviderMetadata {
                kind: kind.into(),
                display_name: name.into(),
            },
            program: program.into(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            events,
        }
    }

    async fn remove_session(&self, session: &str) -> Option<ManagedProcess> {
        self.sessions.lock().await.remove(session)
    }
}

#[async_trait]
impl ProviderAdapter for CliProvider {
    fn metadata(&self) -> ProviderMetadata {
        self.meta.clone()
    }

    async fn start_session(&self, thread: &str) -> Result<String> {
        let spec = ProcessSpec {
            program: self.program.clone(),
            args: Vec::new(),
            cwd: None,
        };
        let mut child = ManagedProcess::spawn(spec).await?;
        let session = Uuid::new_v4().to_string();
        let _ = self.events.send(ProviderRuntimeEvent::Started {
            session: session.clone(),
            thread: thread.to_string(),
        });
        if let Some(stdout) = child.take_stdout() {
            let events = self.events.clone();
            let session_for_task = session.clone();
            let sessions = self.sessions.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = events.send(ProviderRuntimeEvent::TextDelta {
                        session: session_for_task.clone(),
                        text: line,
                    });
                }
                if sessions.lock().await.remove(&session_for_task).is_some() {
                    let _ = events.send(ProviderRuntimeEvent::Completed {
                        session: session_for_task,
                    });
                }
            });
        }
        self.sessions.lock().await.insert(session.clone(), child);
        Ok(session)
    }

    async fn send_turn(&self, session: &str, prompt: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        let process = sessions
            .get_mut(session)
            .ok_or_else(|| anyhow!("unknown provider session: {session}"))?;
        let stdin = process
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("provider session has no stdin: {session}"))?;
        stdin.write_all(prompt.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn interrupt(&self, session: &str) -> Result<()> {
        if let Some(mut process) = self.remove_session(session).await {
            process.terminate().await?;
            let _ = self.events.send(ProviderRuntimeEvent::Completed {
                session: session.to_string(),
            });
        }
        Ok(())
    }

    fn subscribe(&self) -> Option<broadcast::Receiver<ProviderRuntimeEvent>> {
        Some(self.events.subscribe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sessions_are_isolated() {
        let provider = CliProvider::new("test", "Test", "sh");
        let first = provider.start_session("thread-a").await.unwrap();
        let second = provider.start_session("thread-b").await.unwrap();
        assert_ne!(first, second);
        provider.send_turn(&first, "printf first").await.unwrap();
        provider.send_turn(&second, "printf second").await.unwrap();
        provider.interrupt(&first).await.unwrap();
        provider.interrupt(&second).await.unwrap();
        assert!(provider.sessions.lock().await.is_empty());
    }
}
