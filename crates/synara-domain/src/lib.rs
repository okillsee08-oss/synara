//! Canonical Synara domain model. It contains no UI or transport code.
use serde::{Deserialize, Serialize};
use synara_core::EntityId;

macro_rules! id_type {
    ($n:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $n(pub EntityId);

        impl $n {
            pub fn new() -> Self {
                Self(EntityId::new())
            }
        }
    };
}

id_type!(ProjectId);
id_type!(WorkspaceId);
id_type!(ThreadId);
id_type!(MessageId);
id_type!(TurnId);
id_type!(TaskId);
id_type!(SubagentId);
id_type!(ProviderId);
id_type!(ProviderSessionId);
id_type!(ModelId);
id_type!(AgentId);
id_type!(ToolCallId);
id_type!(ApprovalId);
id_type!(AttachmentId);
id_type!(WorktreeId);
id_type!(TerminalSessionId);
id_type!(AutomationId);
id_type!(McpServerId);
id_type!(BrowserSessionId);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub project_id: ProjectId,
    pub root_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thread {
    pub id: ThreadId,
    pub workspace_id: WorkspaceId,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub thread_id: ThreadId,
    pub role: MessageRole,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TurnStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub id: TurnId,
    pub thread_id: ThreadId,
    pub status: TurnStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: ProviderId,
    pub kind: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderSession {
    pub id: ProviderSessionId,
    pub provider_id: ProviderId,
    pub thread_id: ThreadId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub turn_id: TurnId,
    pub name: String,
    pub arguments_json: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approval {
    pub id: ApprovalId,
    pub tool_call_id: ToolCallId,
    pub approved: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Worktree {
    pub id: WorktreeId,
    pub project_id: ProjectId,
    pub path: String,
    pub branch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: TerminalSessionId,
    pub workspace_id: WorkspaceId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Automation {
    pub id: AutomationId,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServer {
    pub id: McpServerId,
    pub name: String,
    pub transport: String,
}
