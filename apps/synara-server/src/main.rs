use anyhow::Result;use synara_db::Database;use synara_orchestrator::{Command,Orchestrator};
fn main()->Result<()>{let db=Database::open_memory()?;let mut app=Orchestrator::new(db)?;let id=app.dispatch(Command::CreateProject{name:"Synara".into(),root_path:".".into()})?;println!("Synara Rust core ready: {id}");Ok(())}
