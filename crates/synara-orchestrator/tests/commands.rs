use synara_db::Database;
use synara_orchestrator::{Command, Orchestrator};

#[test]
fn commands_append_durable_events() {
    let db = Database::open_memory().unwrap();
    let mut orchestrator = Orchestrator::new(db).unwrap();

    let id = orchestrator
        .dispatch(Command::CreateProject {
            name: "demo".into(),
            root_path: "/tmp/demo".into(),
        })
        .unwrap();

    assert_eq!(orchestrator.db.latest_sequence().unwrap(), 1);
    let events = orchestrator.db.events_after(0).unwrap();
    assert_eq!(events[0].entity_id, id);
    assert_eq!(events[0].event_type, "ProjectCreated");
}

#[test]
fn rejects_children_of_missing_parents() {
    let db = Database::open_memory().unwrap();
    let mut orchestrator = Orchestrator::new(db).unwrap();

    assert!(
        orchestrator
            .dispatch(Command::CreateWorkspace {
                project_id: synara_core::EntityId::new(),
                root_path: "/tmp/ws".into(),
            })
            .is_err()
    );

    assert!(
        orchestrator
            .dispatch(Command::CreateThread {
                workspace_id: synara_core::EntityId::new(),
                title: None,
            })
            .is_err()
    );

    assert!(
        orchestrator
            .dispatch(Command::StartTurn {
                thread_id: synara_core::EntityId::new(),
            })
            .is_err()
    );
}

#[test]
fn creates_parent_chain_before_children() {
    let db = Database::open_memory().unwrap();
    let mut orchestrator = Orchestrator::new(db).unwrap();

    let project_id = orchestrator
        .dispatch(Command::CreateProject {
            name: "demo".into(),
            root_path: "/tmp/demo".into(),
        })
        .unwrap();
    let workspace_id = orchestrator
        .dispatch(Command::CreateWorkspace {
            project_id,
            root_path: "/tmp/demo".into(),
        })
        .unwrap();
    let thread_id = orchestrator
        .dispatch(Command::CreateThread {
            workspace_id,
            title: Some("main".into()),
        })
        .unwrap();
    let turn_id = orchestrator
        .dispatch(Command::StartTurn { thread_id })
        .unwrap();

    assert_eq!(orchestrator.db.project_count().unwrap(), 1);
    assert_eq!(orchestrator.db.workspace_count().unwrap(), 1);
    assert_eq!(orchestrator.db.thread_count().unwrap(), 1);
    assert_eq!(orchestrator.db.turn_count().unwrap(), 1);

    orchestrator
        .dispatch(Command::StopTurn { turn_id })
        .unwrap();
}

#[test]
fn task_and_subagent_lifecycle_is_durable() {
    let db = Database::open_memory().unwrap();
    let mut orchestrator = Orchestrator::new(db).unwrap();
    let project_id = orchestrator
        .dispatch(Command::CreateProject {
            name: "demo".into(),
            root_path: "/tmp/demo".into(),
        })
        .unwrap();
    let workspace_id = orchestrator
        .dispatch(Command::CreateWorkspace {
            project_id,
            root_path: "/tmp/demo".into(),
        })
        .unwrap();
    let thread_id = orchestrator
        .dispatch(Command::CreateThread {
            workspace_id,
            title: Some("agent".into()),
        })
        .unwrap();
    let task_id = orchestrator
        .dispatch(Command::CreateTask {
            thread_id,
            name: "build feature".into(),
        })
        .unwrap();

    orchestrator
        .dispatch(Command::StartTask { task_id })
        .unwrap();
    let subagent_id = orchestrator
        .dispatch(Command::CreateSubagent {
            task_id,
            provider_kind: Some("codex".into()),
        })
        .unwrap();
    orchestrator
        .dispatch(Command::StopSubagent { subagent_id })
        .unwrap();
    orchestrator
        .dispatch(Command::CompleteTask { task_id })
        .unwrap();

    assert_eq!(orchestrator.db.task_count().unwrap(), 1);
    assert_eq!(orchestrator.db.subagent_count().unwrap(), 1);
}
