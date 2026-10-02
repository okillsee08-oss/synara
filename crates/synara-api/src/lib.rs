use axum::{routing::get,Router};use std::sync::Arc;
#[derive(Clone)]pub struct ApiState{pub name:Arc<str>}
pub fn router(state:ApiState)->Router{Router::new().route("/health",get(health)).with_state(state)}
async fn health(axum::extract::State(s):axum::extract::State<ApiState>)->String{format!("{{\"status\":\"ok\",\"service\":\"{}\"}}",s.name)}
