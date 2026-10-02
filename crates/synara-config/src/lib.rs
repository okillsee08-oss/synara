//! Layered configuration independent of presentation.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub db_path: PathBuf,
    pub web_dir: PathBuf,
    pub epoch: u64,
    pub client_build: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:3210".parse().expect("valid default address"),
            db_path: PathBuf::from("synara.db"),
            web_dir: PathBuf::from("frontend/web"),
            epoch: 1,
            client_build: "V1-Proto".into(),
        }
    }
}

impl ServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let defaults = Self::default();
        Ok(Self {
            addr: std::env::var("SYNARA_ADDR")
                .ok()
                .map(|v| v.parse())
                .transpose()?
                .unwrap_or(defaults.addr),
            db_path: std::env::var_os("SYNARA_DB")
                .map(PathBuf::from)
                .unwrap_or(defaults.db_path),
            web_dir: std::env::var_os("SYNARA_WEB_DIR")
                .map(PathBuf::from)
                .unwrap_or(defaults.web_dir),
            epoch: std::env::var("SYNARA_EPOCH")
                .ok()
                .map(|v| v.parse())
                .transpose()?
                .unwrap_or(defaults.epoch),
            client_build: std::env::var("SYNARA_CLIENT_BUILD")
                .unwrap_or(defaults.client_build),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_server_config_is_local() {
        let config = ServerConfig::default();
        assert_eq!(config.addr, "127.0.0.1:3210".parse().unwrap());
        assert_eq!(config.epoch, 1);
        assert_eq!(config.client_build, "V1-Proto");
    }
}
