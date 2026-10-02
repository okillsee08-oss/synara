pub mod agent;

use anyhow::Result;
use serde_json::json;
use synara_core::{EntityId, Sequence};
use synara_db::Database;
use synara_events::Event;
use synara_providers::ProviderRuntimeEvent;

#[derive(Debug)]
pub enum Command {
    CreateProject { name: String, root_path: String },
    CreateWorkspace { project_id: EntityId, root_path: String },
    CreateThread { workspace_id: EntityId, title: Option<String> },
    SendMessage { thread_id: EntityId, content: String },
    StartTurn { thread_id: EntityId },
    StopTurn { turn_id: EntityId },
    ApproveTool { tool_call_id: EntityId, approved: bool },
}

pub struct Orchestrator {
    pub db: Database,
}

impl Orchestrator {
    pub fn new(db: Database) -> Result<Self> {
        Ok(Self { db })
    }

    pub fn dispatch(&mut self, command: Command) -> Result<EntityId> {
        let (id, scope, event_type, payload) = match command {
            Command::CreateProject { name, root_path } => (
                EntityId::new(),
                "project",
                "ProjectCreated",
                json!({ "name": name, "root_path": root_path }),
            ),
            Command::CreateWorkspace { project_id, root_path } => (
                EntityId::new(),
                "workspace",
                "WorkspaceCreated",
                json!({ "project_id": project_id, "root_path": root_path }),
            ),
            Command::CreateThread { workspace_id, title } => (
                EntityId::new(),
                "thread",
                "ThreadCreated",
                json!({ "workspace_id": workspace_id, "title": title }),
            ),
            Command::SendMessage { thread_id, content } => (
                EntityId::new(),
                "message",
                "MessageCreated",
                json!({ "thread_id": thread_id, "content": content }),
            ),
            Command::StartTurn { thread_id } => {
                let turn_id = EntityId::new();
                (
                    turn_id,
                    "turn",
                    "TurnStarted",
                    json!({ "thread_id": thread_id, "turn_id": turn_id }),
                )
            }
            Command::StopTurn { turn_id } => (
                turn_id,
                "turn",
                "TurnStopped",
                json!({ "turn_id": turn_id }),
            ),
            Command::ApproveTool {
                tool_call_id,
                approved,
            } => (
                tool_call_id,
                "approval",
                if approved {
                    "ToolApproved"
                } else {
                    "ToolRejected"
                },
                json!({ "tool_call_id": tool_call_id, "approved": approved }),
            ),
        };

        let sequence = self.db.latest_sequence()? + 1;
        let event = Event::new(scope, id, event_type, payload, 1, sequence);
        self.db.append_event(&event)?;
        Ok(id)
    }

    pub fn record_provider_event(
        &mut self,
        provider_kind: &str,
        runtime_event: ProviderRuntimeEvent,
    ) -> Result<Sequence> {
        let (session, event_type, payload) = match runtime_event {
            ProviderRuntimeEvent::Started { session } => (
                session.clone(),
                "ProviderSessionStarted",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                }),
            ),
            ProviderRuntimeEvent::TextDelta { session, text } => (
                session.clone(),
                "ProviderTextDelta",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                    "text": text,
                }),
            ),
            ProviderRuntimeEvent::ToolCall {
                session,
                name,
                arguments,
            } => (
                session.clone(),
                "ProviderToolCall",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                    "name": name,
                    "arguments": arguments,
                }),
            ),
            ProviderRuntimeEvent::Completed { session } => (
                session.clone(),
                "ProviderCompleted",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                }),
            ),
            ProviderRuntimeEvent::Failed { session, error } => (
                session.clone(),
                "ProviderFailed",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                    "error": error,
                }),
            ),
        };

        let entity_id = session.parse().unwrap_or_else(|_| EntityId::new());
        let sequence = self.db.latest_sequence()? + 1;
        let event = Event::new("provider", entity_id, event_type, payload, 1, sequence);
        self.db.append_event(&event)?;
        Ok(sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_events_become_durable_events() {
        let db = Database::open_memory().unwrap();
        let mut orchestrator = Orchestrator::new(db).unwrap();
        let session = EntityId::new().to_string();

        let sequence = orchestrator
            .record_provider_event(
                "test",
                ProviderRuntimeEvent::TextDelta {
                    session: session.clone(),
                    text: "hello".into(),
                },
            )
            .unwrap();

        assert_eq!(sequence, 1);
        let events = orchestrator.db.events_after(0).unwrap();
        assert_eq!(events[0].event_type, "ProviderTextDelta");
        assert_eq!(events[0].payload["provider_kind"], "test");
    }
}
