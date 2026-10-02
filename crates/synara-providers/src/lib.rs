pub mod codex;
pub mod process;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub kind: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProviderRuntimeEvent {
    Started {
        session: String,
    },
    TextDelta {
        session: String,
        text: String,
    },
    ToolCall {
        session: String,
        name: String,
        arguments: Value,
    },
    Completed {
        session: String,
    },
    Failed {
        session: String,
        error: String,
    },
}

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn metadata(&self) -> ProviderMetadata;

    async fn start_session(&self, thread: &str) -> Result<String>;

    async fn send_turn(&self, session: &str, prompt: &str) -> Result<()>;

    async fn interrupt(&self, session: &str) -> Result<()>;

    fn subscribe(&self) -> Option<broadcast::Receiver<ProviderRuntimeEvent>> {
        None
    }
}

#[derive(Default)]
pub struct ProviderRegistry {
    adapters: Vec<Arc<dyn ProviderAdapter>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<A>(&mut self, adapter: A)
    where
        A: ProviderAdapter + 'static,
    {
        self.adapters.push(Arc::new(adapter));
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn ProviderAdapter>> {
        self.adapters
            .iter()
            .find(|adapter| adapter.metadata().kind == kind)
            .cloned()
    }

    pub fn list(&self) -> Vec<ProviderMetadata> {
        self.adapters.iter().map(|adapter| adapter.metadata()).collect()
    }

    pub fn subscribe_all(
        &self,
    ) -> Vec<(ProviderMetadata, broadcast::Receiver<ProviderRuntimeEvent>)> {
        self.adapters
            .iter()
            .filter_map(|adapter| {
                adapter
                    .subscribe()
                    .map(|receiver| (adapter.metadata(), receiver))
            })
            .collect()
    }

    pub fn register_builtins(&mut self) {
        self.register(codex::CodexProvider::new());
        self.register(process::CliProvider::new("claudeAgent", "Claude Agent", "claude"));
        self.register(process::CliProvider::new("opencode", "OpenCode", "opencode"));
        self.register(process::CliProvider::new("pi", "Pi", "pi"));
        self.register(process::CliProvider::new("cursor", "Cursor", "cursor"));
        self.register(process::CliProvider::new("devin", "Devin", "devin"));
        self.register(process::CliProvider::new("grok", "Grok", "grok"));
        self.register(process::CliProvider::new("droid", "Droid", "droid"));
        self.register(process::CliProvider::new("antigravity", "Antigravity", "antigravity"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_contains_codex() {
        let mut registry = ProviderRegistry::new();
        registry.register_builtins();
        assert!(registry.get("codex").is_some());
    }
}
