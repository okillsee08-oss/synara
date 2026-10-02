use synara_db::Database;
use synara_orchestrator::{Command, Orchestrator};

#[test]
fn commands_append_durable_events() {
    let db = Database::open_memory().unwrap();
    let mut orchestrator = Orchestrator::new(db).unwrap();

    let id = orchestrator.dispatch(Command::CreateProject {
        name: "demo".into(),
        root_path: "/tmp/demo".into(),
    }).unwrap();

    assert_eq!(orchestrator.db.latest_sequence().unwrap(), 1);
    let events = orchestrator.db.events_after(0).unwrap();
    assert_eq!(events[0].entity_id, id);
    assert_eq!(events[0].event_type, "ProjectCreated");
}
