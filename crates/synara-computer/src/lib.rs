use anyhow::Result;
use synara_security::SecurityPolicy;
#[derive(Clone, Debug)]
pub enum Action {
    Click { x: i32, y: i32 },
    Type { text: String },
    Key { key: String },
    Scroll { dx: i32, dy: i32 },
}
pub trait ComputerDriver: Send + Sync {
    fn execute(&self, action: Action, policy: &SecurityPolicy) -> Result<()>;
    fn screenshot(&self) -> Result<Vec<u8>>;
}
