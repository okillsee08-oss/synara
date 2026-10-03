//! UI-independent primitives shared by every Synara runtime.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(Uuid);

impl EntityId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EntityId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

pub type Sequence = u64;
pub type Revision = u32;
pub type Timestamp = DateTime<Utc>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny,
    Ask,
}

#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(#[from] uuid::Error),
    #[error("validation failed: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_is_unique_and_non_nil() {
        let a = EntityId::new();
        let b = EntityId::new();
        assert!(!a.is_nil());
        assert!(!b.is_nil());
        assert_ne!(a, b);
    }

    #[test]
    fn entity_id_round_trips_through_display_and_from_str() {
        let id = EntityId::new();
        let parsed: EntityId = id.to_string().parse().expect("valid UUID");
        assert_eq!(id, parsed);
    }

    #[test]
    fn entity_id_serde_is_a_uuid_string() {
        let id = EntityId::new();
        let json = serde_json::to_value(id).expect("serializes");
        assert_eq!(json.as_str(), Some(id.to_string().as_str()));
        let decoded: EntityId = serde_json::from_value(json).expect("deserializes");
        assert_eq!(decoded, id);
    }

    #[test]
    fn page_preserves_items_and_cursor() {
        let page = Page {
            items: vec![1, 2, 3],
            next_cursor: Some("next".into()),
        };
        let round_trip: Page<i32> =
            serde_json::from_value(serde_json::to_value(&page).expect("serializes"))
                .expect("deserializes");
        assert_eq!(round_trip, page);
    }

    #[test]
    fn policy_decisions_are_stable() {
        assert_ne!(PolicyDecision::Allow, PolicyDecision::Deny);
        assert_ne!(PolicyDecision::Deny, PolicyDecision::Ask);
    }
}
