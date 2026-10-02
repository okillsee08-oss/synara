use anyhow::Result;
use std::{net::SocketAddr, sync::Arc};
use synara_api::{ApiState, router};
use synara_config::ServerConfig;
use synara_db::Database;
use synara_diagnostics::init;
use synara_orchestrator::Orchestrator;
use synara_providers::ProviderRegistry;
use synara_transport::EventBus;
use tokio::sync::Mutex;

pub async fn run() -> Result<()> {
    init();

    let config = ServerConfig::from_env()?;
    let db = Database::open(&config.db_path)?;
    let mut orchestrator = Orchestrator::new(db)?;
    let recovered = orchestrator.recover_interrupted_runtime()?;
    if recovered > 0 {
        println!("Recovered {recovered} interrupted runtime item(s)");
    }

    let mut providers = ProviderRegistry::new();
    providers.register_builtins();
    let provider_subscriptions = providers.subscribe_all();

    let state = ApiState {
        name: "synara".into(),
        epoch: config.epoch,
        server_instance_id: Arc::from(uuid::Uuid::new_v4().to_string()),
        orchestrator: Arc::new(Mutex::new(orchestrator)),
        providers: Arc::new(providers),
        events: EventBus::new(2048),
        terminals: Arc::new(Mutex::new(std::collections::HashMap::new())),
        web_dir: config.web_dir.clone(),
        client_build: Arc::from(config.client_build.clone()),
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
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!(
                            "provider {} event subscriber lagged by {} event(s)",
                            metadata.kind, skipped
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    let app = router(state);
    let addr: SocketAddr = config.addr;

    println!("Synara Rust server listening on http://{addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
