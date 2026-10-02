use synara_core::PolicyDecision;

#[derive(Clone, Debug, Default)]
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
}
