use serde::{Deserialize, Serialize};

pub const CURRENT_REVISION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WsNegotiation {
    pub epoch: u64,
    pub negotiated_revision: u32,
    pub server_instance_id: String,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub revision: u32,
    pub sequence: u64,
    pub payload: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayRequest {
    pub epoch: u64,
    pub last_sequence: u64,
    pub revision: u32,
    pub client_build: String,
    pub server_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayResponse<T> {
    pub epoch: u64,
    pub revision: u32,
    pub server_instance_id: String,
    pub from_sequence: u64,
    pub latest_sequence: u64,
    pub events: Vec<Envelope<T>>,
    pub snapshot_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}
