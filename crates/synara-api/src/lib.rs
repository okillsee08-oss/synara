pub const API_PROTOCOL_VERSION: u32 = 1;

use axum::{
    Json, Router,
    extract::{Path, Query, State, WebSocketUpgrade},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeDir};

use synara_core::EntityId;
use synara_orchestrator::{Command, Orchestrator};
use synara_protocol::{
    ApiError, CURRENT_REVISION, Envelope, ReplayResponse, SERVER_CAPABILITIES, WsNegotiation,
    negotiate_revision,
};
use synara_providers::{ProviderMetadata, ProviderRegistry};
use synara_terminal::Terminal;
use synara_transport::EventBus;

#[derive(Clone)]
pub struct ApiState {
    pub name: Arc<str>,
    pub epoch: u64,
    pub server_instance_id: Arc<str>,
    pub orchestrator: Arc<Mutex<Orchestrator>>,
    pub providers: Arc<ProviderRegistry>,
    pub events: EventBus<serde_json::Value>,
    pub terminals: Arc<Mutex<HashMap<String, Arc<Mutex<Terminal>>>>>,
    pub web_dir: PathBuf,
    pub client_build: Arc<str>,
}

#[derive(Debug, Deserialize)]
pub struct NegotiationQuery {
    pub epoch: Option<u64>,
    #[serde(alias = "minRevision")]
    pub min_revision: Option<u32>,
    #[serde(alias = "maxRevision")]
    pub max_revision: Option<u32>,
    #[serde(alias = "clientBuild")]
    pub client_build: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeSummary {
    pub latest_sequence: u64,
    pub projects: u64,
    pub threads: u64,
    pub workspaces: u64,
    pub turns: u64,
    pub approvals: u64,
    pub messages: u64,
    pub provider_sessions: u64,
    pub tasks: u64,
    pub subagents: u64,
    pub provider_count: usize,
}

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub service: String,
    pub revision: u32,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub root_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub project_id: EntityId,
    pub root_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateThreadRequest {
    pub workspace_id: EntityId,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub thread_id: EntityId,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct StartTurnRequest {
    pub thread_id: EntityId,
    pub provider_kind: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub thread_id: EntityId,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubagentRequest {
    pub task_id: EntityId,
    pub provider_kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FailTaskRequest {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: EntityId,
    pub thread_id: EntityId,
    pub name: String,
    pub status: String,
    pub created_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct SubagentResponse {
    pub id: EntityId,
    pub task_id: EntityId,
    pub provider_kind: Option<String>,
    pub status: String,
    pub created_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub id: EntityId,
    pub turn_id: Option<EntityId>,
    pub name: String,
    pub arguments: serde_json::Value,
    pub status: String,
    pub created_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct ApprovalResponse {
    pub id: EntityId,
    pub tool_call_id: EntityId,
    pub approved: Option<bool>,
    pub updated_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct StartTurnResponse {
    pub message_id: EntityId,
    pub turn_id: EntityId,
    pub provider_session_id: String,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub id: EntityId,
}

#[derive(Debug, Deserialize)]
pub struct CreateTerminalRequest {
    pub shell: Option<String>,
    pub cwd: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalInputRequest {
    pub input: String,
}

#[derive(Debug, Deserialize)]
pub struct TerminalResizeRequest {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Serialize)]
pub struct TerminalResponse {
    pub id: String,
    pub shell: String,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: EntityId,
    pub name: String,
    pub root_path: String,
    pub created_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceResponse {
    pub id: EntityId,
    pub project_id: EntityId,
    pub root_path: String,
    pub created_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct ThreadResponse {
    pub id: EntityId,
    pub workspace_id: EntityId,
    pub title: Option<String>,
    pub created_sequence: u64,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: EntityId,
    pub thread_id: EntityId,
    pub role: String,
    pub content: String,
    pub created_sequence: u64,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws/negotiate", get(negotiate))
        .route("/api/v1/events", get(events))
        .route("/api/v1/providers", get(providers))
        .route("/api/v1/summary", get(summary))
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route(
            "/api/v1/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route("/api/v1/threads", get(list_threads).post(create_thread))
        .route("/api/v1/messages", get(list_messages).post(send_message))
        .route("/api/v1/turns", post(start_turn))
        .route(
            "/api/v1/terminals",
            get(list_terminals).post(create_terminal),
        )
        .route("/api/v1/terminals/:id/input", post(write_terminal))
        .route("/api/v1/terminals/:id/resize", post(resize_terminal))
        .route("/api/v1/terminals/:id/kill", post(kill_terminal))
        .route("/ws/terminals/:id", get(terminal_websocket))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route("/api/v1/tasks/:id/start", post(start_task))
        .route("/api/v1/tasks/:id/complete", post(complete_task))
        .route("/api/v1/tasks/:id/cancel", post(cancel_task))
        .route("/api/v1/tasks/:id/fail", post(fail_task))
        .route(
            "/api/v1/subagents",
            get(list_subagents).post(create_subagent),
        )
        .route("/api/v1/subagents/:id/stop", post(stop_subagent))
        .route("/api/v1/tool-calls", get(list_tool_calls))
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/tool-calls/:id/approve", post(approve_tool))
        .route("/api/v1/tool-calls/:id/reject", post(reject_tool))
        .route("/ws", get(websocket))
        .layer(CorsLayer::permissive())
        .fallback_service(ServeDir::new(state.web_dir.clone()))
        .with_state(state)
}

async fn health(State(s): State<ApiState>) -> Json<Health> {
    Json(Health {
        status: "ok",
        service: s.name.to_string(),
        revision: CURRENT_REVISION,
    })
}

async fn negotiate(
    State(s): State<ApiState>,
    Query(q): Query<NegotiationQuery>,
) -> Result<Json<WsNegotiation>, (axum::http::StatusCode, Json<ApiError>)> {
    let client_min = q.min_revision.unwrap_or(CURRENT_REVISION);
    let client_max = q.max_revision.unwrap_or(CURRENT_REVISION);
    let negotiated_revision = negotiate_revision(client_min, client_max).map_err(|error| {
        let (code, message) = match error {
            synara_protocol::RevisionNegotiationError::InvalidRange => (
                "INVALID_REVISION_RANGE",
                "min_revision must be less than or equal to max_revision",
            ),
            synara_protocol::RevisionNegotiationError::NoCompatibleRevision => (
                "REVISION_MISMATCH",
                "client and server have no compatible protocol revision",
            ),
        };
        (
            axum::http::StatusCode::UPGRADE_REQUIRED,
            Json(ApiError {
                code: code.into(),
                message: message.into(),
            }),
        )
    })?;
    if let Some(epoch) = q.epoch {
        if epoch != s.epoch {
            return Err((
                axum::http::StatusCode::UPGRADE_REQUIRED,
                Json(ApiError {
                    code: "EPOCH_MISMATCH".into(),
                    message: "client epoch does not match this server".into(),
                }),
            ));
        }
    }
    if let Some(build) = q.client_build.as_deref() {
        if build.trim().is_empty() {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(ApiError {
                    code: "INVALID_CLIENT_BUILD".into(),
                    message: "client_build cannot be empty".into(),
                }),
            ));
        }
    }
    Ok(Json(WsNegotiation {
        epoch: s.epoch,
        negotiated_revision,
        server_instance_id: s.server_instance_id.to_string(),
        capabilities: SERVER_CAPABILITIES
            .iter()
            .map(|v| (*v).to_string())
            .collect(),
    }))
}

async fn summary(
    State(s): State<ApiState>,
) -> Result<Json<RuntimeSummary>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let db = &guard.db;
    Ok(Json(RuntimeSummary {
        latest_sequence: db.latest_sequence().map_err(internal_error)?,
        projects: db.project_count().map_err(internal_error)?,
        threads: db.thread_count().map_err(internal_error)?,
        workspaces: db.workspace_count().map_err(internal_error)?,
        turns: db.turn_count().map_err(internal_error)?,
        approvals: db.approval_count().map_err(internal_error)?,
        messages: db.message_count().map_err(internal_error)?,
        provider_sessions: db.provider_session_count().map_err(internal_error)?,
        tasks: db.task_count().map_err(internal_error)?,
        subagents: db.subagent_count().map_err(internal_error)?,
        provider_count: s.providers.list().len(),
    }))
}

async fn providers(State(s): State<ApiState>) -> Json<Vec<ProviderMetadata>> {
    Json(s.providers.list())
}

async fn events(
    State(s): State<ApiState>,
    Query(q): Query<ReplayQuery>,
) -> Result<Json<ReplayResponse<serde_json::Value>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let latest = guard.db.latest_sequence().map_err(internal_error)?;
    let stored = guard.db.events_after(q.after).map_err(internal_error)?;
    let events = stored
        .into_iter()
        .map(|e| Envelope {
            revision: e.version,
            sequence: e.sequence,
            payload: e.payload,
        })
        .collect();
    Ok(Json(ReplayResponse {
        epoch: s.epoch,
        revision: CURRENT_REVISION,
        server_instance_id: s.server_instance_id.to_string(),
        from_sequence: q.after,
        latest_sequence: latest,
        events,
        snapshot_required: q.after > latest,
    }))
}

#[derive(Debug, Deserialize)]
struct ReplayQuery {
    #[serde(default)]
    after: u64,
}

async fn start_turn(
    State(s): State<ApiState>,
    Json(req): Json<StartTurnRequest>,
) -> Result<Json<StartTurnResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.prompt.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_REQUEST".into(),
                message: "prompt is required".into(),
            }),
        ));
    }

    let provider = s.providers.get(&req.provider_kind).ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "PROVIDER_NOT_FOUND".into(),
                message: format!("unknown provider: {}", req.provider_kind),
            }),
        )
    })?;

    let (message_id, turn_id) = {
        let mut guard = s.orchestrator.lock().await;
        let message_id = guard
            .dispatch(Command::SendMessage {
                thread_id: req.thread_id,
                content: req.prompt.clone(),
            })
            .map_err(internal_error)?;
        publish_latest(&s, &guard).map_err(internal_error)?;

        let turn_id = guard
            .dispatch(Command::StartTurn {
                thread_id: req.thread_id,
            })
            .map_err(internal_error)?;
        publish_latest(&s, &guard).map_err(internal_error)?;
        (message_id, turn_id)
    };

    let provider_session_id = match provider.start_session(&req.thread_id.to_string()).await {
        Ok(session) => session,
        Err(error) => {
            let mut guard = s.orchestrator.lock().await;
            let _ = guard.dispatch(Command::FailTurn {
                turn_id,
                error: error.to_string(),
            });
            let _ = publish_latest(&s, &guard);
            return Err(internal_error(error));
        }
    };

    if let Err(error) = provider.send_turn(&provider_session_id, &req.prompt).await {
        let mut guard = s.orchestrator.lock().await;
        let _ = guard.dispatch(Command::FailTurn {
            turn_id,
            error: error.to_string(),
        });
        let _ = publish_latest(&s, &guard);
        return Err(internal_error(error));
    }

    Ok(Json(StartTurnResponse {
        message_id,
        turn_id,
        provider_session_id,
    }))
}

async fn list_terminals(State(s): State<ApiState>) -> Json<Vec<TerminalResponse>> {
    let terminals = s.terminals.lock().await;
    Json(
        terminals
            .iter()
            .map(|(id, _)| TerminalResponse {
                id: id.clone(),
                shell: "managed".into(),
                cwd: None,
                cols: 0,
                rows: 0,
            })
            .collect(),
    )
}

async fn create_terminal(
    State(s): State<ApiState>,
    Json(req): Json<CreateTerminalRequest>,
) -> Result<Json<TerminalResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let shell = req.shell.unwrap_or_else(|| {
        if cfg!(windows) {
            "cmd.exe".into()
        } else {
            "/bin/sh".into()
        }
    });
    let cols = req.cols.unwrap_or(120).max(1);
    let rows = req.rows.unwrap_or(36).max(1);
    let terminal = Terminal::spawn(
        &shell,
        req.cwd.as_deref().map(std::path::Path::new),
        cols,
        rows,
    )
    .map_err(internal_error)?;
    let id = EntityId::new().to_string();
    let terminal = Arc::new(Mutex::new(terminal));
    terminal
        .lock()
        .await
        .start_output_stream()
        .map_err(internal_error)?;
    s.terminals.lock().await.insert(id.clone(), terminal);

    Ok(Json(TerminalResponse {
        id,
        shell,
        cwd: req.cwd,
        cols,
        rows,
    }))
}

async fn write_terminal(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<TerminalInputRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let terminal = s.terminals.lock().await.get(&id).cloned().ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "TERMINAL_NOT_FOUND".into(),
                message: format!("terminal not found: {id}"),
            }),
        )
    })?;
    terminal
        .lock()
        .await
        .write(req.input.as_bytes())
        .map_err(internal_error)?;
    Ok(Json(CommandResponse {
        id: EntityId::new(),
    }))
}

async fn resize_terminal(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<TerminalResizeRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.cols == 0 || req.rows == 0 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_SIZE".into(),
                message: "cols and rows must be greater than zero".into(),
            }),
        ));
    }
    let terminal = s.terminals.lock().await.get(&id).cloned().ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "TERMINAL_NOT_FOUND".into(),
                message: format!("terminal not found: {id}"),
            }),
        )
    })?;
    terminal
        .lock()
        .await
        .resize(req.cols, req.rows)
        .map_err(internal_error)?;
    Ok(Json(CommandResponse {
        id: EntityId::new(),
    }))
}

async fn kill_terminal(
    State(s): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let terminal = s.terminals.lock().await.remove(&id);
    let terminal = terminal.ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "TERMINAL_NOT_FOUND".into(),
                message: format!("terminal not found: {id}"),
            }),
        )
    })?;
    terminal.lock().await.kill().map_err(internal_error)?;
    Ok(Json(CommandResponse {
        id: EntityId::new(),
    }))
}

async fn terminal_websocket(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, (axum::http::StatusCode, Json<ApiError>)> {
    let terminal = s.terminals.lock().await.get(&id).cloned().ok_or_else(|| {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "TERMINAL_NOT_FOUND".into(),
                message: format!("terminal not found: {id}"),
            }),
        )
    })?;
    Ok(ws.on_upgrade(move |mut socket| async move {
        use axum::extract::ws::Message;
        let mut output = terminal.lock().await.subscribe();
        loop {
            tokio::select! {
                value = output.recv() => {
                    match value {
                        Ok(chunk) => {
                            if socket.send(Message::Binary(chunk.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let _ = socket.send(Message::Text(
                                serde_json::json!({"type":"resync_required","reason":"terminal_output_lag"}).to_string().into()
                            )).await;
                            break;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                incoming = socket.recv() => {
                    match incoming {
                        Some(Ok(Message::Text(text))) if text == "close" => break,
                        Some(Ok(Message::Ping(data))) => {
                            let _ = socket.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(_)) => break,
                        Some(Ok(_)) => {}
                    }
                }
            }
        }
    }))
}

async fn list_tool_calls(
    State(s): State<ApiState>,
) -> Result<Json<Vec<ToolCallResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let items = guard
        .db
        .list_tool_calls()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, turn_id, name, arguments, status, created_sequence)| ToolCallResponse {
                id,
                turn_id,
                name,
                arguments: serde_json::from_str(&arguments).unwrap_or(serde_json::Value::Null),
                status,
                created_sequence,
            },
        )
        .collect();
    Ok(Json(items))
}

async fn list_approvals(
    State(s): State<ApiState>,
) -> Result<Json<Vec<ApprovalResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let items = guard
        .db
        .list_approvals()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, tool_call_id, approved, updated_sequence)| ApprovalResponse {
                id,
                tool_call_id,
                approved,
                updated_sequence,
            },
        )
        .collect();
    Ok(Json(items))
}

async fn approve_tool(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let result = guard
        .dispatch(Command::ApproveTool {
            tool_call_id: id,
            approved: true,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id: result }))
}

async fn reject_tool(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let result = guard
        .dispatch(Command::ApproveTool {
            tool_call_id: id,
            approved: false,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id: result }))
}

async fn list_tasks(
    State(s): State<ApiState>,
) -> Result<Json<Vec<TaskResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let tasks = guard
        .db
        .list_tasks()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, thread_id, name, status, created_sequence)| TaskResponse {
                id,
                thread_id,
                name,
                status,
                created_sequence,
            },
        )
        .collect();
    Ok(Json(tasks))
}

async fn list_subagents(
    State(s): State<ApiState>,
) -> Result<Json<Vec<SubagentResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let subagents = guard
        .db
        .list_subagents()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, task_id, provider_kind, status, created_sequence)| SubagentResponse {
                id,
                task_id,
                provider_kind,
                status,
                created_sequence,
            },
        )
        .collect();
    Ok(Json(subagents))
}

async fn create_task(
    State(s): State<ApiState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.name.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_REQUEST".into(),
                message: "task name is required".into(),
            }),
        ));
    }
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CreateTask {
            thread_id: req.thread_id,
            name: req.name,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn create_subagent(
    State(s): State<ApiState>,
    Json(req): Json<CreateSubagentRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CreateSubagent {
            task_id: req.task_id,
            provider_kind: req.provider_kind,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn start_task(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::StartTask { task_id: id })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn complete_task(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CompleteTask { task_id: id })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn cancel_task(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CancelTask { task_id: id })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn fail_task(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
    Json(req): Json<FailTaskRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.error.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_REQUEST".into(),
                message: "error is required".into(),
            }),
        ));
    }
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::FailTask {
            task_id: id,
            error: req.error,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn stop_subagent(
    State(s): State<ApiState>,
    Path(id): Path<EntityId>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::StopSubagent { subagent_id: id })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn list_projects(
    State(s): State<ApiState>,
) -> Result<Json<Vec<ProjectResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let projects = guard
        .db
        .list_projects()
        .map_err(internal_error)?
        .into_iter()
        .map(|(id, name, root_path, created_sequence)| ProjectResponse {
            id,
            name,
            root_path,
            created_sequence,
        })
        .collect();
    Ok(Json(projects))
}

async fn list_workspaces(
    State(s): State<ApiState>,
) -> Result<Json<Vec<WorkspaceResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let workspaces = guard
        .db
        .list_workspaces()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, project_id, root_path, created_sequence)| WorkspaceResponse {
                id,
                project_id,
                root_path,
                created_sequence,
            },
        )
        .collect();
    Ok(Json(workspaces))
}

async fn list_threads(
    State(s): State<ApiState>,
) -> Result<Json<Vec<ThreadResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let threads = guard
        .db
        .list_threads()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, workspace_id, title, created_sequence)| ThreadResponse {
                id,
                workspace_id,
                title,
                created_sequence,
            },
        )
        .collect();
    Ok(Json(threads))
}

async fn list_messages(
    State(s): State<ApiState>,
) -> Result<Json<Vec<MessageResponse>>, (axum::http::StatusCode, Json<ApiError>)> {
    let guard = s.orchestrator.lock().await;
    let messages = guard
        .db
        .list_messages()
        .map_err(internal_error)?
        .into_iter()
        .map(
            |(id, thread_id, role, content, created_sequence)| MessageResponse {
                id,
                thread_id,
                role,
                content,
                created_sequence,
            },
        )
        .collect();
    Ok(Json(messages))
}

async fn create_project(
    State(s): State<ApiState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.name.trim().is_empty() || req.root_path.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_REQUEST".into(),
                message: "name and root_path are required".into(),
            }),
        ));
    }
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CreateProject {
            name: req.name,
            root_path: req.root_path,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn create_workspace(
    State(s): State<ApiState>,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.root_path.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_REQUEST".into(),
                message: "root_path is required".into(),
            }),
        ));
    }
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CreateWorkspace {
            project_id: req.project_id,
            root_path: req.root_path,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn create_thread(
    State(s): State<ApiState>,
    Json(req): Json<CreateThreadRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::CreateThread {
            workspace_id: req.workspace_id,
            title: req.title,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn send_message(
    State(s): State<ApiState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    if req.content.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiError {
                code: "INVALID_REQUEST".into(),
                message: "content is required".into(),
            }),
        ));
    }
    let mut guard = s.orchestrator.lock().await;
    let id = guard
        .dispatch(Command::SendMessage {
            thread_id: req.thread_id,
            content: req.content,
        })
        .map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

#[derive(Debug, Deserialize)]
struct WebSocketQuery {
    #[serde(default)]
    last_sequence: u64,
    epoch: Option<u64>,
    revision: Option<u32>,
    #[serde(alias = "minRevision")]
    min_revision: Option<u32>,
    #[serde(alias = "maxRevision")]
    max_revision: Option<u32>,
    #[serde(alias = "clientBuild")]
    client_build: Option<String>,
    #[serde(alias = "serverInstance")]
    server_instance: Option<String>,
}

async fn websocket(
    State(s): State<ApiState>,
    Query(q): Query<WebSocketQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, (axum::http::StatusCode, Json<ApiError>)> {
    let client_min = q
        .min_revision
        .unwrap_or(q.revision.unwrap_or(CURRENT_REVISION));
    let client_max = q
        .max_revision
        .unwrap_or(q.revision.unwrap_or(CURRENT_REVISION));
    let negotiated_revision = negotiate_revision(client_min, client_max).map_err(|error| {
        let (code, message) = match error {
            synara_protocol::RevisionNegotiationError::InvalidRange => (
                "INVALID_REVISION_RANGE",
                "min_revision must be less than or equal to max_revision",
            ),
            synara_protocol::RevisionNegotiationError::NoCompatibleRevision => (
                "REVISION_MISMATCH",
                "client and server have no compatible protocol revision",
            ),
        };
        (
            axum::http::StatusCode::UPGRADE_REQUIRED,
            Json(ApiError {
                code: code.into(),
                message: message.into(),
            }),
        )
    })?;
    if let Some(epoch) = q.epoch {
        if epoch != s.epoch {
            return Err((
                axum::http::StatusCode::UPGRADE_REQUIRED,
                Json(ApiError {
                    code: "EPOCH_MISMATCH".into(),
                    message: "client epoch does not match this server".into(),
                }),
            ));
        }
    }
    if let Some(instance) = q.server_instance.as_deref() {
        if instance != s.server_instance_id.as_ref() {
            return Err((
                axum::http::StatusCode::UPGRADE_REQUIRED,
                Json(ApiError {
                    code: "SERVER_INSTANCE_MISMATCH".into(),
                    message: "client is connected to a different server instance".into(),
                }),
            ));
        }
    }
    if let Some(build) = q.client_build.as_deref() {
        if build.trim().is_empty() {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(ApiError {
                    code: "INVALID_CLIENT_BUILD".into(),
                    message: "client_build cannot be empty".into(),
                }),
            ));
        }
    }

    let state = s.clone();
    Ok(ws.on_upgrade(move |mut socket| async move {
        let mut live = state.events.subscribe();
        use axum::extract::ws::Message;
        let (replay, latest) = {
            let guard = state.orchestrator.lock().await;
            match (
                guard.db.events_after(q.last_sequence),
                guard.db.latest_sequence(),
            ) {
                (Ok(events), Ok(latest)) => (Some(events), latest),
                _ => (None, q.last_sequence),
            }
        };

        let mut cursor = q.last_sequence;
        let hello = serde_json::json!({
            "type": "hello",
            "epoch": state.epoch,
            "revision": negotiated_revision,
            "server_instance_id": state.server_instance_id,
            "latest_sequence": latest,
            "snapshot_required": q.last_sequence > latest,
        });
        if socket.send(Message::Text(hello.to_string().into())).await.is_err() {
            return;
        }
        let Some(events) = replay else {
            let _ = socket.send(Message::Text(serde_json::json!({
                "type": "resync_required",
                "reason": "replay_failed"
            }).to_string().into())).await;
            return;
        };
        for event in events {
            cursor = cursor.max(event.sequence);
            let envelope = serde_json::json!({
                "type": "event",
                "revision": event.version,
                "sequence": event.sequence,
                "event_type": event.event_type,
                "entity_id": event.entity_id,
                "payload": event.payload,
            });
            if socket.send(Message::Text(envelope.to_string().into())).await.is_err() {
                return;
            }
        }

        loop {
            tokio::select! {
                incoming = socket.recv() => {
                    match incoming {
                        Some(Ok(Message::Ping(data))) => {
                            let _ = socket.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Text(text))) if text == "close" => break,
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Ok(_)) => {}
                        Some(Err(_)) => break,
                    }
                }
                event = live.recv() => {
                    match event {
                        Ok(envelope) => {
                            if envelope.sequence <= cursor {
                                continue;
                            }
                            cursor = envelope.sequence;
                            let message = serde_json::json!({
                                "type": "event",
                                "revision": envelope.revision,
                                "sequence": envelope.sequence,
                                "payload": envelope.payload,
                            });
                            if socket.send(Message::Text(message.to_string().into())).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let _ = socket.send(Message::Text(serde_json::json!({
                                "type":"resync_required",
                                "last_sequence":cursor
                            }).to_string().into())).await;
                            break;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }))
}

fn internal_error(error: anyhow::Error) -> (axum::http::StatusCode, Json<ApiError>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiError {
            code: "INTERNAL_ERROR".into(),
            message: error.to_string(),
        }),
    )
}

fn publish_latest(s: &ApiState, guard: &Orchestrator) -> anyhow::Result<()> {
    let latest = guard.db.latest_sequence()?;
    if let Some(event) = guard
        .db
        .events_after(latest.saturating_sub(1))?
        .into_iter()
        .find(|e| e.sequence == latest)
    {
        s.events.publish(Envelope {
            revision: event.version,
            sequence: event.sequence,
            payload: serde_json::json!({
                "event_type": event.event_type,
                "entity_id": event.entity_id,
                "payload": event.payload,
            }),
        });
    }
    Ok(())
}
