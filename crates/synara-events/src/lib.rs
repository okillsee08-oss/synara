use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use synara_core::{EntityId, Revision, Sequence, Timestamp};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub event_id: EntityId,
    pub sequence: Sequence,
    pub timestamp: Timestamp,
    pub scope: String,
    pub entity_id: EntityId,
    pub event_type: String,
    pub version: Revision,
    pub payload: Value,
}

impl Event {
    pub fn new(
        scope: impl Into<String>,
        entity_id: EntityId,
        event_type: impl Into<String>,
        payload: Value,
        version: Revision,
        sequence: Sequence,
    ) -> Self {
        Self {
            event_id: EntityId::new(),
            sequence,
            timestamp: Utc::now(),
            scope: scope.into(),
            entity_id,
            event_type: event_type.into(),
            version,
            payload,
        }
    }
}
