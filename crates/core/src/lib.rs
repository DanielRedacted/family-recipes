pub mod auth;
pub mod recipe;
pub mod server;
pub mod vault;

use axum::Router;
use server::AppState;
use std::sync::Arc;

/// State owned by later subsystems (import jobs, LAN status).
#[derive(Default)]
pub struct Extensions {}

pub fn extra_routes() -> Router<Arc<AppState>> {
    Router::new()
}

pub fn lan_status(_: &AppState) -> serde_json::Value {
    serde_json::Value::Null
}

pub fn ai_key_set(_: &AppState) -> bool {
    false
}

pub fn settings_changed(_: &Arc<AppState>) {}
