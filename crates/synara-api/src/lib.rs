use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeDir};
use uuid::Uuid;

use synara_core::EntityId;
use synara_orchestrator::{Command, Orchestrator};
use synara_protocol::{negotiate_revision, ApiError, Envelope, ReplayResponse, WsNegotiation, CURRENT_REVISION, SERVER_CAPABILITIES};
use synara_providers::{ProviderMetadata, ProviderRegistry};
use synara_transport::EventBus;

#[derive(Clone)]
pub struct ApiState {
    pub name: Arc<str>,
    pub epoch: u64,
    pub server_instance_id: Arc<str>,
    pub orchestrator: Arc<Mutex<Orchestrator>>,
    pub providers: Arc<ProviderRegistry>,
    pub events: EventBus<serde_json::Value>,
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

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub id: EntityId,
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
        .route("/api/v1/workspaces", get(list_workspaces).post(create_workspace))
        .route("/api/v1/threads", get(list_threads).post(create_thread))
        .route("/api/v1/messages", get(list_messages).post(send_message))
        .route("/ws", get(websocket))
        .layer(CorsLayer::permissive())
        .fallback_service(ServeDir::new(
            std::env::var("SYNARA_WEB_DIR").unwrap_or_else(|_| "frontend/web".into()),
        ))
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
            Json(ApiError { code: code.into(), message: message.into() }),
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
        capabilities: SERVER_CAPABILITIES.iter().map(|v| (*v).to_string()).collect(),
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
    let events = stored.into_iter().map(|e| Envelope {
        revision: e.version,
        sequence: e.sequence,
        payload: e.payload,
    }).collect();
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
        .map(|(id, project_id, root_path, created_sequence)| WorkspaceResponse {
            id,
            project_id,
            root_path,
            created_sequence,
        })
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
        .map(|(id, workspace_id, title, created_sequence)| ThreadResponse {
            id,
            workspace_id,
            title,
            created_sequence,
        })
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
        .map(|(id, thread_id, role, content, created_sequence)| MessageResponse {
            id,
            thread_id,
            role,
            content,
            created_sequence,
        })
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
    let id = guard.dispatch(Command::CreateProject {
        name: req.name,
        root_path: req.root_path,
    }).map_err(internal_error)?;
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
    let id = guard.dispatch(Command::CreateWorkspace {
        project_id: req.project_id,
        root_path: req.root_path,
    }).map_err(internal_error)?;
    publish_latest(&s, &guard).map_err(internal_error)?;
    Ok(Json(CommandResponse { id }))
}

async fn create_thread(
    State(s): State<ApiState>,
    Json(req): Json<CreateThreadRequest>,
) -> Result<Json<CommandResponse>, (axum::http::StatusCode, Json<ApiError>)> {
    let mut guard = s.orchestrator.lock().await;
    let id = guard.dispatch(Command::CreateThread {
        workspace_id: req.workspace_id,
        title: req.title,
    }).map_err(internal_error)?;
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
    let id = guard.dispatch(Command::SendMessage {
        thread_id: req.thread_id,
        content: req.content,
    }).map_err(internal_error)?;
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
    let client_min = q.min_revision.unwrap_or(q.revision.unwrap_or(CURRENT_REVISION));
    let client_max = q.max_revision.unwrap_or(q.revision.unwrap_or(CURRENT_REVISION));
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
            Json(ApiError { code: code.into(), message: message.into() }),
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
    if let Some(event) = guard.db.events_after(latest.saturating_sub(1))?.into_iter().find(|e| e.sequence == latest) {
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
