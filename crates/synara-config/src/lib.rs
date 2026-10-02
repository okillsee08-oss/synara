//! Layered configuration independent of presentation.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SynaraConfig {
    pub provider: BTreeMap<String, serde_json::Value>,
    pub security: BTreeMap<String, serde_json::Value>,
    pub terminal: BTreeMap<String, serde_json::Value>,
    pub ui: BTreeMap<String, serde_json::Value>,
    pub automation: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigLayer {
    Defaults,
    System,
    User,
    Project,
    Workspace,
    Thread,
    Session,
}
pub trait ConfigSource {
    fn load(&self, layer: ConfigLayer) -> anyhow::Result<SynaraConfig>;
}
pub fn merge(base: &mut SynaraConfig, overlay: SynaraConfig) {
    base.provider.extend(overlay.provider);
    base.security.extend(overlay.security);
    base.terminal.extend(overlay.terminal);
    base.ui.extend(overlay.ui);
    base.automation.extend(overlay.automation);
}
