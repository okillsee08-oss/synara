use anyhow::Result;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use synara_api::{ApiState, router};
use synara_db::Database;
use synara_diagnostics::init;
use synara_orchestrator::Orchestrator;
use synara_providers::ProviderRegistry;
use synara_transport::EventBus;
use tokio::sync::Mutex;

pub async fn run() -> Result<()> {
    init();

    let db_path = std::env::var_os("SYNARA_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("synara.db"));
    let db = Database::open(db_path)?;
    let orchestrator = Orchestrator::new(db)?;

    let mut providers = ProviderRegistry::new();
    providers.register_builtins();
    let provider_subscriptions = providers.subscribe_all();

    let state = ApiState {
        name: "synara".into(),
        epoch: 1,
        server_instance_id: Arc::from(uuid::Uuid::new_v4().to_string()),
        orchestrator: Arc::new(Mutex::new(orchestrator)),
        providers: Arc::new(providers),
        events: EventBus::new(2048),
    };

    for (metadata, mut receiver) in provider_subscriptions {
        let orchestrator = state.orchestrator.clone();
        let events = state.events.clone();
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(runtime_event) => {
                        let mut guard = orchestrator.lock().await;
                        let Ok(sequence) =
                            guard.record_provider_event(&metadata.kind, runtime_event)
                        else {
                            continue;
                        };

                        if let Ok(stored) = guard.db.events_after(sequence.saturating_sub(1)) {
                            if let Some(event) =
                                stored.into_iter().find(|event| event.sequence == sequence)
                            {
                                events.publish(synara_protocol::Envelope {
                                    revision: event.version,
                                    sequence: event.sequence,
                                    payload: serde_json::json!({
                                        "event_type": event.event_type,
                                        "entity_id": event.entity_id,
                                        "payload": event.payload,
                                    }),
                                });
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    let app = router(state);
    let addr: SocketAddr = std::env::var("SYNARA_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3210".into())
        .parse()?;

    println!("Synara Rust server listening on http://{addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
