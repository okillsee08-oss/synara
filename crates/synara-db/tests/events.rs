use serde_json::json;
use synara_core::EntityId;
use synara_db::Database;
use synara_events::Event;

#[test]
fn events_round_trip_in_sequence_order() {
    let db = Database::open_memory().unwrap();
    let id = EntityId::new();
    db.append_event(&Event::new("test", id, "First", json!({"n": 1}), 1, 1))
        .unwrap();
    db.append_event(&Event::new("test", id, "Second", json!({"n": 2}), 1, 2))
        .unwrap();

    assert_eq!(db.latest_sequence().unwrap(), 2);
    let events = db.events_after(1).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, 2);
    assert_eq!(events[0].event_type, "Second");
}

#[test]
fn projection_failure_rolls_back_event_append() {
    let db = Database::open_memory().unwrap();
    let project_id = EntityId::new();

    db.append_event(&Event::new(
        "project",
        project_id,
        "ProjectCreated",
        json!({"name": "one", "root_path": "/tmp/one"}),
        1,
        1,
    ))
    .unwrap();

    let duplicate = db.append_event(&Event::new(
        "project",
        project_id,
        "ProjectCreated",
        json!({"name": "two", "root_path": "/tmp/two"}),
        1,
        2,
    ));
    assert!(duplicate.is_err());
    assert_eq!(db.latest_sequence().unwrap(), 1);
    assert_eq!(db.project_count().unwrap(), 1);
}

#[test]
fn provider_session_lifecycle_is_projected() {
    let db = Database::open_memory().unwrap();
    let session = EntityId::new();

    db.append_event(&Event::new(
        "provider",
        session,
        "ProviderSessionStarted",
        json!({"provider_kind": "test", "session": session.to_string()}),
        1,
        1,
    ))
    .unwrap();

    assert_eq!(db.provider_session_count().unwrap(), 1);

    db.append_event(&Event::new(
        "provider",
        session,
        "ProviderCompleted",
        json!({"provider_kind": "test", "session": session.to_string()}),
        1,
        2,
    ))
    .unwrap();

    assert_eq!(db.provider_session_count().unwrap(), 1);
}
