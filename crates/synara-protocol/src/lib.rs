use serde::{Deserialize, Serialize};

pub const MIN_SUPPORTED_REVISION: u32 = 1;
pub const MAX_SUPPORTED_REVISION: u32 = 1;
pub const CURRENT_REVISION: u32 = MAX_SUPPORTED_REVISION;

pub const SERVER_CAPABILITIES: &[&str] = &[
    "replay",
    "snapshot",
    "websocket",
];

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevisionNegotiationError {
    InvalidRange,
    NoCompatibleRevision,
}

pub fn negotiate_revision(
    client_min: u32,
    client_max: u32,
) -> Result<u32, RevisionNegotiationError> {
    if client_min > client_max {
        return Err(RevisionNegotiationError::InvalidRange);
    }

    let min = client_min.max(MIN_SUPPORTED_REVISION);
    let max = client_max.min(MAX_SUPPORTED_REVISION);

    if min > max {
        return Err(RevisionNegotiationError::NoCompatibleRevision);
    }

    Ok(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_overlapping_revision() {
        assert_eq!(negotiate_revision(1, 1), Ok(1));
        assert_eq!(negotiate_revision(1, 9), Ok(1));
    }

    #[test]
    fn rejects_non_overlapping_revision() {
        assert_eq!(
            negotiate_revision(2, 3),
            Err(RevisionNegotiationError::NoCompatibleRevision)
        );
        assert_eq!(
            negotiate_revision(0, 0),
            Err(RevisionNegotiationError::NoCompatibleRevision)
        );
    }

    #[test]
    fn rejects_inverted_range() {
        assert_eq!(
            negotiate_revision(2, 1),
            Err(RevisionNegotiationError::InvalidRange)
        );
    }
}
