use anyhow::Result;
use synara_security::SecurityPolicy;

#[derive(Clone, Debug)]
pub enum Action {
    Click { x: i32, y: i32 },
    Type { text: String },
    Key { key: String },
    Scroll { dx: i32, dy: i32 },
}

impl Action {
    pub fn validate(&self) -> Result<()> {
        match self {
            Action::Click { x, y } if *x < 0 || *y < 0 => {
                anyhow::bail!("click coordinates cannot be negative")
            }
            Action::Click { x, y } if *x > 1_000_000 || *y > 1_000_000 => {
                anyhow::bail!("click coordinates are out of bounds")
            }
            Action::Scroll { dx, dy } if *dx == 0 && *dy == 0 => {
                anyhow::bail!("scroll delta cannot be zero")
            }
            Action::Key { key } if key.trim().is_empty() => {
                anyhow::bail!("key cannot be empty")
            }
            Action::Type { text } if text.is_empty() => {
                anyhow::bail!("typed text cannot be empty")
            }
            _ => Ok(()),
        }
    }
}

pub trait ComputerDriver: Send + Sync {
    fn execute(&self, action: Action, policy: &SecurityPolicy) -> Result<()>;
    fn screenshot(&self) -> Result<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_validation_blocks_invalid_input() {
        assert!(Action::Click { x: -1, y: 0 }.validate().is_err());
        assert!(Action::Click { x: 1_000_001, y: 0 }.validate().is_err());
        assert!(Action::Key { key: " ".into() }.validate().is_err());
        assert!(Action::Type { text: "x".into() }.validate().is_ok());
    }
}
