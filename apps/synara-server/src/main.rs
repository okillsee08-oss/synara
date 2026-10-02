use anyhow::Result;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use synara_api::{router, ApiState};
use synara_db::Database;
use synara_diagnostics::init;
use synara_orchestrator::Orchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    init();

    let db_path = std::env::var_os("SYNARA_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("synara.db"));
    let db = Database::open(db_path)?;
    let orchestrator = Orchestrator::new(db)?;

    let state = ApiState {
        name: "synara".into(),
        epoch: 1,
        server_instance_id: Arc::from(uuid::Uuid::new_v4().to_string()),
        orchestrator: Arc::new(Mutex::new(orchestrator)),
    };

    let app = router(state);
    let addr: SocketAddr = std::env::var("SYNARA_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3210".into())
        .parse()?;

    println!("Synara Rust server listening on http://{addr}");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
