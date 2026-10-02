use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{broadcast, Mutex},
};
use uuid::Uuid;

use synara_process::{ManagedProcess, ProcessSpec};

use super::{ProviderAdapter, ProviderMetadata, ProviderRuntimeEvent};

struct SessionState {
    remote_thread_id: Option<String>,
    process: Option<Arc<Mutex<ManagedProcess>>>,
    running: bool,
    terminal_sent: bool,
}

pub struct CodexProvider {
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SessionState>>>>>,
    events: broadcast::Sender<ProviderRuntimeEvent>,
}

impl CodexProvider {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(512);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            events,
        }
    }

    fn emit_from_json(
        &self,
        session_id: &str,
        line: &str,
        state: &Arc<Mutex<SessionState>>,
    ) -> Option<ProviderRuntimeEvent> {
        let value: Value = serde_json::from_str(line).ok()?;
        parse_codex_event(session_id, &value, state)
    }
}

#[async_trait]
impl ProviderAdapter for CodexProvider {
    fn metadata(&self) -> ProviderMetadata {
        ProviderMetadata {
            kind: "codex".into(),
            display_name: "Codex".into(),
        }
    }

    async fn start_session(&self, _thread: &str) -> Result<String> {
        let session = Uuid::new_v4().to_string();
        self.sessions.lock().await.insert(
            session.clone(),
            Arc::new(Mutex::new(SessionState {
                remote_thread_id: None,
                process: None,
                running: false,
                terminal_sent: false,
            })),
        );
        let _ = self.events.send(ProviderRuntimeEvent::Started {
            session: session.clone(),
        });
        Ok(session)
    }

    async fn send_turn(&self, session: &str, prompt: &str) -> Result<()> {
        let state = self
            .sessions
            .lock()
            .await
            .get(session)
            .cloned()
            .ok_or_else(|| anyhow!("unknown Codex session: {session}"))?;

        let mut guard = state.lock().await;
        if guard.running {
            return Err(anyhow!("Codex session is already running: {session}"));
        }

        let args = if let Some(thread_id) = guard.remote_thread_id.clone() {
            vec![
                "exec".into(),
                "--json".into(),
                "resume".into(),
                thread_id,
                prompt.into(),
            ]
        } else {
            vec!["exec".into(), "--json".into(), prompt.into()]
        };

        let mut process = ManagedProcess::spawn(ProcessSpec {
            program: "codex".into(),
            args,
            cwd: None,
        })
        .await?;

        let stdout = process
            .take_stdout()
            .ok_or_else(|| anyhow!("Codex process has no stdout"))?;
        let process = Arc::new(Mutex::new(process));

        guard.process = Some(process.clone());
        guard.running = true;
        guard.terminal_sent = false;
        drop(guard);

        let events = self.events.clone();
        let session_id = session.to_string();
        let parser_state = state.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if value.get("type").and_then(Value::as_str) == Some("thread.started") {
                        if let Some(thread_id) = value.get("thread_id").and_then(Value::as_str) {
                            parser_state.lock().await.remote_thread_id = Some(thread_id.to_string());
                        }
                    }
                    if let Some(event) = parse_codex_event(&session_id, &value) {
                        let terminal = matches!(
                            event,
                            ProviderRuntimeEvent::Completed { .. } | ProviderRuntimeEvent::Failed { .. }
                        );
                        if terminal {
                            let mut state = parser_state.lock().await;
                            if state.terminal_sent {
                                continue;
                            }
                            state.terminal_sent = true;
                        }
                        let _ = events.send(event);
                    }
                }
            }
        });

        let events = self.events.clone();
        let sessions = self.sessions.clone();
        let session_id = session.to_string();
        tokio::spawn(async move {
            let process = { state.lock().await.process.clone() };
            let status = if let Some(process) = process {
                process.lock().await.wait().await.ok()
            } else {
                None
            };

            let terminal = {
                let mut guard = state.lock().await;
                guard.process = None;
                guard.running = false;
                if !guard.terminal_sent {
                    guard.terminal_sent = true;
                    true
                } else {
                    false
                }
            };

            if terminal {
                let event = match status {
                    Some(status) if status.success() => ProviderRuntimeEvent::Completed {
                        session: session_id.clone(),
                    },
                    Some(status) => ProviderRuntimeEvent::Failed {
                        session: session_id.clone(),
                        error: format!("Codex exited with status {status}"),
                    },
                    None => ProviderRuntimeEvent::Failed {
                        session: session_id.clone(),
                        error: "Codex process ended without an exit status".into(),
                    },
                };
                let _ = events.send(event);
            }

            let _ = sessions;
        });

        Ok(())
    }

    async fn interrupt(&self, session: &str) -> Result<()> {
        let state = self
            .sessions
            .lock()
            .await
            .get(session)
            .cloned()
            .ok_or_else(|| anyhow!("unknown Codex session: {session}"))?;

        let mut guard = state.lock().await;
        if !guard.running {
            return Ok(());
        }

        if let Some(process) = guard.process.as_mut() {
            process.terminate().await?;
        }
        guard.process = None;
        guard.running = false;
        if !guard.terminal_sent {
            guard.terminal_sent = true;
            let _ = self.events.send(ProviderRuntimeEvent::Failed {
                session: session.to_string(),
                error: "Codex turn interrupted".into(),
            });
        }
        Ok(())
    }

    fn subscribe(&self) -> Option<broadcast::Receiver<ProviderRuntimeEvent>> {
        Some(self.events.subscribe())
    }
}

fn parse_codex_event(
    session_id: &str,
    value: &Value,
) -> Option<ProviderRuntimeEvent> {
    match value.get("type").and_then(Value::as_str)? {
        "thread.started" => None
        "item.completed" => {
            let item = value.get("item")?;
            match item.get("type").and_then(Value::as_str) {
                Some("agent_message") => item
                    .get("text")
                    .and_then(Value::as_str)
                    .map(|text| ProviderRuntimeEvent::TextDelta {
                        session: session_id.to_string(),
                        text: text.to_string(),
                    }),
                Some("command_execution") => item
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|command| ProviderRuntimeEvent::ToolCall {
                        session: session_id.to_string(),
                        name: "command_execution".into(),
                        arguments: serde_json::json!({ "command": command }),
                    }),
                _ => None,
            }
        }
        "turn.completed" => Some(ProviderRuntimeEvent::Completed {
            session: session_id.to_string(),
        }),
        "turn.failed" => {
            let error = value
                .get("error")
                .and_then(|v| v.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("Codex turn failed");
            Some(ProviderRuntimeEvent::Failed {
                session: session_id.to_string(),
                error: error.to_string(),
            })
        }
        "error" => {
            let error = value
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| value.get("error").and_then(Value::as_str))
                .unwrap_or("Codex reported an error");
            Some(ProviderRuntimeEvent::Failed {
                session: session_id.to_string(),
                error: error.to_string(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_agent_message() {
        let (_tx, _rx) = broadcast::channel(8);
        let state = Arc::new(Mutex::new(SessionState {
            remote_thread_id: None,
            process: None,
            running: true,
            terminal_sent: false,
        }));
        let event = parse_codex_event(
            "session",
            &serde_json::json!({
                "type": "item.completed",
                "item": {"type": "agent_message", "text": "hello"}
            }),
            &state,
        );
        assert!(matches!(
            event,
            Some(ProviderRuntimeEvent::TextDelta { text, .. }) if text == "hello"
        ));
    }

    #[tokio::test]
    async fn parses_turn_failure() {
        let state = Arc::new(Mutex::new(SessionState {
            remote_thread_id: None,
            process: None,
            running: true,
            terminal_sent: false,
        }));
        let event = parse_codex_event(
            "session",
            &serde_json::json!({
                "type": "turn.failed",
                "error": {"message": "boom"}
            }),
        );
        assert!(matches!(
            event,
            Some(ProviderRuntimeEvent::Failed { error, .. }) if error == "boom"
        ));
    }
}
