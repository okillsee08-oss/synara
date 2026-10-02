use serde::Serialize;
use std::sync::Arc;
use synara_protocol::Envelope;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct EventBus<T: Clone + Send + Sync + 'static> {
    tx: broadcast::Sender<Arc<Envelope<T>>>,
}

impl<T: Clone + Send + Sync + 'static> EventBus<T> {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn publish(&self, event: Envelope<T>) {
        let _ = self.tx.send(Arc::new(event));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope<T>>> {
        self.tx.subscribe()
    }
}

pub fn _assert_serializable<T: Serialize>() {}
