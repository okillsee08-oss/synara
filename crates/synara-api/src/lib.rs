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

#[derive(Clone)]
pub struct ApiState {
    pub name: Arc<str>,
    pub epoch: u64,
    pub server_instance_id: Arc<str>,
    pub orchestrator: Arc<Mutex<Orchestrator>>,
}

#[derive(Debug, Deserialize)]
pub struct NegotiationQuery {
    pub epoch: Option<u64>,
    pub min_revision: Option<u32>,
    pub max_revision: Option<u32>,
    pub client_build: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub id: EntityId,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws/negotiate", get(negotiate))
        .route("/api/v1/events", get(events))
        .route("/api/v1/projects", post(create_project))
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
    Ok(Json(CommandResponse { id }))
}

async fn websocket(
    State(s): State<ApiState>,
    ws: WebSocketUpgrade,
) -> Response {
    let state = s.clone();
    ws.on_upgrade(move |mut socket| async move {
        use axum::extract::ws::Message;
        let hello = serde_json::json!({
            "type": "hello",
            "epoch": state.epoch,
            "revision": CURRENT_REVISION,
            "server_instance_id": state.server_instance_id,
        });
        let _ = socket.send(Message::Text(hello.to_string().into())).await;
        while let Some(Ok(message)) = socket.recv().await {
            match message {
                Message::Ping(data) => {
                    let _ = socket.send(Message::Pong(data)).await;
                }
                Message::Text(text) if text == "close" => break,
                Message::Close(_) => break,
                _ => {}
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
