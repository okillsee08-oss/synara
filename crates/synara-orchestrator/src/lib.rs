pub mod agent;

use anyhow::Result;
use serde_json::json;
use synara_core::{EntityId, Sequence};
use synara_db::Database;
use synara_events::Event;

#[derive(Debug)]
pub enum Command {
    CreateProject { name: String, root_path: String },
    CreateThread { workspace_id: EntityId, title: Option<String> },
    SendMessage { thread_id: EntityId, content: String },
    StartTurn { thread_id: EntityId },
    StopTurn { thread_id: EntityId },
    ApproveTool { tool_call_id: EntityId, approved: bool },
}

pub struct Orchestrator {
    pub db: Database,
    next_sequence: Sequence,
}

impl Orchestrator {
    pub fn new(db: Database) -> Result<Self> {
        let next_sequence = db.latest_sequence()? + 1;
        Ok(Self { db, next_sequence })
    }

    pub fn dispatch(&mut self, command: Command) -> Result<EntityId> {
        let (id, scope, event_type, payload) = match command {
            Command::CreateProject { name, root_path } => (
                EntityId::new(), "project", "ProjectCreated",
                json!({ "name": name, "root_path": root_path }),
            ),
            Command::CreateThread { workspace_id, title } => (
                EntityId::new(), "thread", "ThreadCreated",
                json!({ "workspace_id": workspace_id, "title": title }),
            ),
            Command::SendMessage { thread_id, content } => (
                EntityId::new(), "message", "MessageCreated",
                json!({ "thread_id": thread_id, "content": content }),
            ),
            Command::StartTurn { thread_id } => {
                let turn_id = EntityId::new();
                (turn_id, "turn", "TurnStarted", json!({ "thread_id": thread_id, "turn_id": turn_id }))
            },
            Command::StopTurn { thread_id } => {
                let turn_id = EntityId::new();
                (turn_id, "turn", "TurnStopped", json!({ "thread_id": thread_id, "turn_id": turn_id }))
            },
            Command::ApproveTool { tool_call_id, approved } => (
                tool_call_id, "approval",
                if approved { "ToolApproved" } else { "ToolRejected" },
                json!({ "tool_call_id": tool_call_id, "approved": approved }),
            ),
        };

        let event = Event::new(
            scope,
            id,
            event_type,
            payload,
            1,
            self.next_sequence,
        );
        self.db.append_event(&event)?;
        self.next_sequence += 1;
        Ok(id)
    }
}
