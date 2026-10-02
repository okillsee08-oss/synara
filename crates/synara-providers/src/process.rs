use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use uuid::Uuid;

use synara_process::{ManagedProcess, ProcessSpec};
use super::{ProviderAdapter, ProviderMetadata};

pub struct CliProvider {
    meta: ProviderMetadata,
    program: String,
    sessions: Arc<Mutex<HashMap<String, ManagedProcess>>>,
}

impl CliProvider {
    pub fn new(kind: &str, name: &str, program: &str) -> Self {
        Self {
            meta: ProviderMetadata { kind: kind.into(), display_name: name.into() },
            program: program.into(),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn remove_session(&self, session: &str) -> Option<ManagedProcess> {
        self.sessions.lock().await.remove(session)
    }
}

#[async_trait]
impl ProviderAdapter for CliProvider {
    fn metadata(&self) -> ProviderMetadata { self.meta.clone() }

    async fn start_session(&self, _thread: &str) -> Result<String> {
        let spec = ProcessSpec {
            program: self.program.clone(),
            args: Vec::new(),
            cwd: None,
        };
        let child = ManagedProcess::spawn(spec).await?;
        let session = Uuid::new_v4().to_string();
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
        }
        Ok(())
    }
}
