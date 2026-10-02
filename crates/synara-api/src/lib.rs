use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use synara_core::EntityId;
use synara_orchestrator::{Command, Orchestrator};
use synara_protocol::{ApiError, Envelope, ReplayResponse, WsNegotiation, CURRENT_REVISION};
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
    pub min_revision: Option<u32>,
    pub max_revision: Option<u32>,
    pub client_build: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeSummary {
    pub latest_sequence: u64,
    pub projects: u64,
    pub threads: u64,
    pub messages: u64,
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
pub struct CreateWorkspaceRequest {\n    pub project_id: EntityId,\n    pub root_path: String,\n}\n\n#[derive(Debug, Deserialize)]\npub struct CreateThreadRequest {
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

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws/negotiate", get(negotiate))
        .route("/api/v1/events", get(events))
        .route("/api/v1/providers", get(providers))
        .route("/api/v1/summary", get(summary))
        .route("/api/v1/projects", post(create_project))
        .route("/api/v1/workspaces", post(create_workspace))\n        .route("/api/v1/threads", post(create_thread))
        .route("/api/v1/messages", post(send_message))
        .route("/ws", get(websocket))
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
    if client_min > CURRENT_REVISION || client_max < CURRENT_REVISION {
        return Err((
            axum::http::StatusCode::UPGRADE_REQUIRED,
            Json(ApiError {
                code: "REVISION_MISMATCH".into(),
                message: format!("server supports revision {CURRENT_REVISION}"),
            }),
        ));
    }
    if let Some(epoch) = q.epoch {
        if epoch != s.epoch {
            return Err((
                axum::http::StatusCode::CONFLICT,
                Json(ApiError {
                    code: "EPOCH_MISMATCH".into(),
                    message: "client epoch does not match this server".into(),
                }),
            ));
        }
    }
    let _ = q.client_build;
    Ok(Json(WsNegotiation {
        epoch: s.epoch,
        negotiated_revision: CURRENT_REVISION,
        server_instance_id: s.server_instance_id.to_string(),
        capabilities: vec!["replay".into(), "snapshot".into(), "websocket".into()],
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
        messages: db.message_count().map_err(internal_error)?,
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
}

async fn websocket(
    State(s): State<ApiState>,
    Query(q): Query<WebSocketQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let state = s.clone();
    ws.on_upgrade(move |mut socket| async move {
        let mut live = state.events.subscribe();
        use axum::extract::ws::Message;
        let replay = {
            let guard = state.orchestrator.lock().await;
            guard.db.events_after(q.last_sequence).ok()
        };

        let hello = serde_json::json!({
            "type": "hello",
            "epoch": state.epoch,
            "revision": CURRENT_REVISION,
            "server_instance_id": state.server_instance_id,
        });
        let _ = socket.send(Message::Text(hello.to_string().into())).await;
        if let Some(events) = replay {
            for event in events {
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
                            let _ = socket.send(Message::Text(serde_json::json!({"type":"resync_required"}).to_string().into())).await;
                            break;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
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
