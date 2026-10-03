pub mod agent;

use anyhow::Result;
use serde_json::json;
use synara_core::{EntityId, Sequence};
use synara_db::Database;
use synara_events::Event;
use synara_providers::ProviderRuntimeEvent;

#[derive(Debug)]
pub enum Command {
    CreateProject {
        name: String,
        root_path: String,
    },
    CreateWorkspace {
        project_id: EntityId,
        root_path: String,
    },
    CreateThread {
        workspace_id: EntityId,
        title: Option<String>,
    },
    SendMessage {
        thread_id: EntityId,
        content: String,
    },
    StartTurn {
        thread_id: EntityId,
    },
    StopTurn {
        turn_id: EntityId,
    },
    FailTurn {
        turn_id: EntityId,
        error: String,
    },
    CreateTask {
        thread_id: EntityId,
        name: String,
    },
    StartTask {
        task_id: EntityId,
    },
    CompleteTask {
        task_id: EntityId,
    },
    FailTask {
        task_id: EntityId,
        error: String,
    },
    CancelTask {
        task_id: EntityId,
    },
    CreateSubagent {
        task_id: EntityId,
        provider_kind: Option<String>,
    },
    StopSubagent {
        subagent_id: EntityId,
    },
    CreateAutomation {
        name: String,
        action: String,
        interval_seconds: u64,
        retry_attempts: u32,
        retry_delay_seconds: u64,
    },
    EnableAutomation {
        automation_id: EntityId,
    },
    DisableAutomation {
        automation_id: EntityId,
    },
    FireAutomation {
        automation_id: EntityId,
    },
    ApproveTool {
        tool_call_id: EntityId,
        approved: bool,
    },
}

pub struct Orchestrator {
    pub db: Database,
}

impl Orchestrator {
    pub fn validate_message(content: &str) -> Result<()> {
        if content.trim().is_empty() { anyhow::bail!("message content must not be empty"); }
        Ok(())
    }

    pub fn new(db: Database) -> Result<Self> {
        Ok(Self { db })
    }

    pub fn dispatch(&mut self, command: Command) -> Result<EntityId> {
        match &command {
            Command::CreateWorkspace { project_id, .. } => {
                if !self.db.project_exists(*project_id)? {
                    anyhow::bail!("project does not exist: {project_id}");
                }
            }
            Command::CreateThread { workspace_id, .. } => {
                if !self.db.workspace_exists(*workspace_id)? {
                    anyhow::bail!("workspace does not exist: {workspace_id}");
                }
            }
            Command::SendMessage { thread_id, .. } => {
                if !self.db.thread_exists(*thread_id)? {
                    anyhow::bail!("thread does not exist: {thread_id}");
                }
            }
            Command::StartTurn { thread_id } => {
                if !self.db.thread_exists(*thread_id)? {
                    anyhow::bail!("thread does not exist: {thread_id}");
                }
                if self.db.running_turn_exists(*thread_id)? {
                    anyhow::bail!("thread already has a running turn: {thread_id}");
                }
            }
            Command::StopTurn { turn_id } | Command::FailTurn { turn_id, .. } => {
                if !self.db.turn_exists(*turn_id)? {
                    anyhow::bail!("turn does not exist: {turn_id}");
                }
            }
            Command::CreateTask { thread_id, .. } => {
                if !self.db.thread_exists(*thread_id)? {
                    anyhow::bail!("thread does not exist: {thread_id}");
                }
            }
            Command::StartTask { task_id }
            | Command::CompleteTask { task_id }
            | Command::FailTask { task_id, .. }
            | Command::CancelTask { task_id } => {
                if !self.db.task_exists(*task_id)? {
                    anyhow::bail!("task does not exist: {task_id}");
                }
            }
            Command::CreateSubagent { task_id, .. } => {
                if !self.db.task_exists(*task_id)? {
                    anyhow::bail!("task does not exist: {task_id}");
                }
            }
            Command::ApproveTool { tool_call_id, .. } => {
                if !self.db.tool_call_exists(*tool_call_id)? {
                    anyhow::bail!("tool call does not exist: {tool_call_id}");
                }
            }
            Command::StopSubagent { subagent_id } => {
                if !self.db.subagent_exists(*subagent_id)? {
                    anyhow::bail!("subagent does not exist: {subagent_id}");
                }
            }
            Command::EnableAutomation { automation_id }
            | Command::DisableAutomation { automation_id }
            | Command::FireAutomation { automation_id } => {
                if !self.db.automation_exists(*automation_id)? {
                    anyhow::bail!("automation does not exist: {automation_id}");
                }
            }
            Command::CreateAutomation { .. }
            | Command::CreateProject { .. }
            | Command::ApproveTool { .. } => {}
        }

        let (id, scope, event_type, payload) = match command {
            Command::CreateProject { name, root_path } => (
                EntityId::new(),
                "project",
                "ProjectCreated",
                json!({ "name": name, "root_path": root_path }),
            ),
            Command::CreateWorkspace {
                project_id,
                root_path,
            } => (
                EntityId::new(),
                "workspace",
                "WorkspaceCreated",
                json!({ "project_id": project_id, "root_path": root_path }),
            ),
            Command::CreateThread {
                workspace_id,
                title,
            } => (
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
            Command::FailTurn { turn_id, error } => (
                turn_id,
                "turn",
                "TurnFailed",
                json!({ "turn_id": turn_id, "error": error }),
            ),
            Command::CreateTask { thread_id, name } => {
                let task_id = EntityId::new();
                (
                    task_id,
                    "task",
                    "TaskCreated",
                    json!({
                        "thread_id": thread_id,
                        "name": name,
                        "task_id": task_id,
                    }),
                )
            }
            Command::StartTask { task_id } => (
                task_id,
                "task",
                "TaskStarted",
                json!({ "task_id": task_id }),
            ),
            Command::CompleteTask { task_id } => (
                task_id,
                "task",
                "TaskCompleted",
                json!({ "task_id": task_id }),
            ),
            Command::FailTask { task_id, error } => (
                task_id,
                "task",
                "TaskFailed",
                json!({ "task_id": task_id, "error": error }),
            ),
            Command::CancelTask { task_id } => (
                task_id,
                "task",
                "TaskCancelled",
                json!({ "task_id": task_id }),
            ),
            Command::CreateSubagent {
                task_id,
                provider_kind,
            } => {
                let subagent_id = EntityId::new();
                (
                    subagent_id,
                    "subagent",
                    "SubagentCreated",
                    json!({
                        "task_id": task_id,
                        "provider_kind": provider_kind,
                        "subagent_id": subagent_id,
                    }),
                )
            }
            Command::StopSubagent { subagent_id } => (
                subagent_id,
                "subagent",
                "SubagentStopped",
                json!({ "subagent_id": subagent_id }),
            ),
            Command::CreateAutomation {
                name,
                action,
                interval_seconds,
                retry_attempts,
                retry_delay_seconds,
            } => {
                let automation_id = EntityId::new();
                (
                    automation_id,
                    "automation",
                    "AutomationCreated",
                    json!({
                        "name": name,
                        "action": action,
                        "interval_seconds": interval_seconds,
                        "enabled": true,
                        "retry_attempts": retry_attempts,
                        "retry_delay_seconds": retry_delay_seconds,
                        "automation_id": automation_id,
                    }),
                )
            }
            Command::EnableAutomation { automation_id } => (
                automation_id,
                "automation",
                "AutomationEnabled",
                json!({ "automation_id": automation_id }),
            ),
            Command::DisableAutomation { automation_id } => (
                automation_id,
                "automation",
                "AutomationDisabled",
                json!({ "automation_id": automation_id }),
            ),
            Command::FireAutomation { automation_id } => (
                automation_id,
                "automation",
                "AutomationFired",
                json!({ "automation_id": automation_id }),
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

    pub fn recover_interrupted_runtime(&mut self) -> Result<u64> {
        let sessions = self.db.list_running_provider_sessions()?;
        let tasks = self.db.list_running_tasks()?;
        let subagents = self.db.list_running_subagents()?;
        let mut recovered = 0;

        for (session, provider_kind, _thread) in sessions {
            let _ = self.record_provider_event(
                &provider_kind,
                ProviderRuntimeEvent::Failed {
                    session: session.to_string(),
                    error: "runtime interrupted by server restart".into(),
                },
            )?;
            recovered += 1;
        }

        for task_id in tasks {
            self.dispatch(Command::FailTask {
                task_id,
                error: "task interrupted by server restart".into(),
            })?;
            recovered += 1;
        }

        for subagent_id in subagents {
            self.dispatch(Command::StopSubagent { subagent_id })?;
            recovered += 1;
        }

        Ok(recovered)
    }

    pub fn record_provider_event(
        &mut self,
        provider_kind: &str,
        runtime_event: ProviderRuntimeEvent,
    ) -> Result<Sequence> {
        let (entity_id, event_type, payload) = match runtime_event {
            ProviderRuntimeEvent::Started { session, thread } => (
                session.parse().unwrap_or_else(|_| EntityId::new()),
                "ProviderSessionStarted",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                    "thread": thread,
                }),
            ),
            ProviderRuntimeEvent::TextDelta { session, text } => (
                session.parse().unwrap_or_else(|_| EntityId::new()),
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
                EntityId::new(),
                "ProviderToolCall",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                    "name": name,
                    "arguments": arguments,
                }),
            ),
            ProviderRuntimeEvent::Completed { session } => (
                session.parse().unwrap_or_else(|_| EntityId::new()),
                "ProviderCompleted",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                }),
            ),
            ProviderRuntimeEvent::Failed { session, error } => (
                session.parse().unwrap_or_else(|_| EntityId::new()),
                "ProviderFailed",
                json!({
                    "provider_kind": provider_kind,
                    "session": session,
                    "error": error,
                }),
            ),
        };

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
    fn dispatch_rejects_missing_parent_entities() {
        let db = Database::open_memory().unwrap();
        let mut orchestrator = Orchestrator::new(db).unwrap();

        let missing = EntityId::new();
        assert!(orchestrator
            .dispatch(Command::CreateWorkspace {
                project_id: missing,
                root_path: "/tmp/workspace".into(),
            })
            .is_err());
        assert!(orchestrator
            .dispatch(Command::CreateThread {
                workspace_id: missing,
                title: None,
            })
            .is_err());
    }

    #[test]
    fn dispatch_rejects_two_running_turns_for_one_thread() {
        let db = Database::open_memory().unwrap();
        let mut orchestrator = Orchestrator::new(db).unwrap();

        let project = orchestrator
            .dispatch(Command::CreateProject {
                name: "test".into(),
                root_path: "/tmp/project".into(),
            })
            .unwrap();
        let workspace = orchestrator
            .dispatch(Command::CreateWorkspace {
                project_id: project,
                root_path: "/tmp/project".into(),
            })
            .unwrap();
        let thread = orchestrator
            .dispatch(Command::CreateThread {
                workspace_id: workspace,
                title: Some("thread".into()),
            })
            .unwrap();

        orchestrator.dispatch(Command::StartTurn { thread_id: thread }).unwrap();
        assert!(orchestrator
            .dispatch(Command::StartTurn { thread_id: thread })
            .is_err());
    }

    #[test]
    fn recovery_is_idempotent_after_first_pass() {
        let db = Database::open_memory().unwrap();
        let mut orchestrator = Orchestrator::new(db).unwrap();

        let project = orchestrator
            .dispatch(Command::CreateProject {
                name: "test".into(),
                root_path: "/tmp/project".into(),
            })
            .unwrap();
        let workspace = orchestrator
            .dispatch(Command::CreateWorkspace {
                project_id: project,
                root_path: "/tmp/project".into(),
            })
            .unwrap();
        let thread = orchestrator
            .dispatch(Command::CreateThread {
                workspace_id: workspace,
                title: None,
            })
            .unwrap();
        let task = orchestrator
            .dispatch(Command::CreateTask {
                thread_id: thread,
                name: "recover".into(),
            })
            .unwrap();
        orchestrator.dispatch(Command::StartTask { task_id: task }).unwrap();

        assert_eq!(orchestrator.recover_interrupted_runtime().unwrap(), 1);
        assert_eq!(orchestrator.recover_interrupted_runtime().unwrap(), 0);
    }

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
