use serde::{Deserialize, Serialize};
use synara_core::PolicyDecision;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SecurityAction {
    Network,
    Shell,
    Filesystem,
    GitPush,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Authorization {
    pub decision: PolicyDecision,
    pub action: SecurityAction,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub allow_network: bool,
    pub allow_shell: bool,
    pub allow_filesystem: bool,
    pub allow_git_push: bool,
}

impl SecurityPolicy {
    pub fn network(&self) -> PolicyDecision {
        if self.allow_network {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Ask
        }
    }

    pub fn shell(&self) -> PolicyDecision {
        if self.allow_shell {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Ask
        }
    }

    pub fn filesystem(&self) -> PolicyDecision {
        if self.allow_filesystem {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Ask
        }
    }

    pub fn git_push(&self) -> PolicyDecision {
        if self.allow_git_push {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Ask
        }
    }

    pub fn authorize(&self, action: SecurityAction) -> Authorization {
        let decision = match action {
            SecurityAction::Network => self.network(),
            SecurityAction::Shell => self.shell(),
            SecurityAction::Filesystem => self.filesystem(),
            SecurityAction::GitPush => self.git_push(),
        };
        let reason = match decision {
            PolicyDecision::Allow => "policy allows this action",
            PolicyDecision::Deny => "policy denies this action",
            PolicyDecision::Ask => "user approval is required for this action",
        };
        Authorization {
            decision,
            action,
            reason: reason.into(),
        }
    }

    pub fn requires_approval(&self, action: SecurityAction) -> bool {
        self.authorize(action).decision == PolicyDecision::Ask
    }

    pub fn is_allowed(&self, action: SecurityAction) -> bool {
        self.authorize(action).decision == PolicyDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_require_approval() {
        let policy = SecurityPolicy::default();
        assert_eq!(
            policy.authorize(SecurityAction::Shell).decision,
            PolicyDecision::Ask
        );
        assert!(!policy.is_allowed(SecurityAction::Network));
    }

    #[test]
    fn enabled_actions_are_allowed() {
        let policy = SecurityPolicy {
            allow_shell: true,
            ..SecurityPolicy::default()
        };
        assert!(policy.is_allowed(SecurityAction::Shell));
        assert!(!policy.is_allowed(SecurityAction::GitPush));
    }
}
