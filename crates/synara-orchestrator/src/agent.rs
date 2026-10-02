use anyhow::Result;
use tokio::sync::mpsc;
use synara_providers::{ProviderAdapter, ProviderRuntimeEvent};

pub struct AgentRuntime {
    events: mpsc::Sender<ProviderRuntimeEvent>,
    capacity: usize,
}

impl AgentRuntime {
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<ProviderRuntimeEvent>) {
        let capacity = capacity.max(1);
        let (events, receiver) = mpsc::channel(capacity);
        (Self { events, capacity }, receiver)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub async fn start_turn<A: ProviderAdapter>(
        &self,
        adapter: &A,
        thread_id: &str,
        prompt: &str,
    ) -> Result<String> {
        let session = adapter.start_session(thread_id).await?;
        let _ = self.events.send(ProviderRuntimeEvent::Started {
            session: session.clone(),
        }).await;
        if let Err(error) = adapter.send_turn(&session, prompt).await {
            let _ = self.events.send(ProviderRuntimeEvent::Failed {
                session: session.clone(),
                error: error.to_string(),
            }).await;
            return Err(error);
        }
        Ok(session)
    }

    pub async fn interrupt<A: ProviderAdapter>(
        &self,
        adapter: &A,
        session: &str,
    ) -> Result<()> {
        adapter.interrupt(session).await?;
        let _ = self.events.send(ProviderRuntimeEvent::Completed {
            session: session.to_string(),
        }).await;
        Ok(())
    }
}
