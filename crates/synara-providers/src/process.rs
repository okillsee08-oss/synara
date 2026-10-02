use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use synara_process::{ManagedProcess, ProcessSpec};
use super::{ProviderAdapter, ProviderMetadata};

pub struct CliProvider {
    meta: ProviderMetadata,
    program: String,
    process: Arc<Mutex<Option<ManagedProcess>>>,
}

impl CliProvider {
    pub fn new(kind: &str, name: &str, program: &str) -> Self {
        Self {
            meta: ProviderMetadata { kind: kind.into(), display_name: name.into() },
            program: program.into(),
            process: Arc::new(Mutex::new(None)),
        }
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
        *self.process.lock().await = Some(child);
        Ok(Uuid::new_v4().to_string())
    }

    async fn send_turn(&self, _session: &str, prompt: &str) -> Result<()> {
        let mut guard = self.process.lock().await;
        if let Some(proc) = guard.as_mut() {
            if let Some(stdin) = proc.child.stdin.as_mut() {
                stdin.write_all(prompt.as_bytes()).await?;
                stdin.write_all(b"\n").await?;
                stdin.flush().await?;
            }
        }
        Ok(())
    }

    async fn interrupt(&self, _session: &str) -> Result<()> {
        if let Some(mut process) = self.process.lock().await.take() {
            process.terminate().await?;
        }
        Ok(())
    }
}
