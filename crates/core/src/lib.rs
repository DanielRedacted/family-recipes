pub mod ai;
pub mod auth;
pub mod extract;
pub mod fetch;
pub mod import;
pub mod lan;
pub mod recipe;
pub mod server;
pub mod vault;

use auth::{Caller, Role};
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use server::AppState;
use std::sync::{Arc, OnceLock};

/// State owned by subsystems.
pub struct Extensions {
    pub lan: lan::Lan,
    pub app: OnceLock<Router>,
    pub jobs: import::Jobs,
}

impl Default for Extensions {
    fn default() -> Self {
        Self { lan: Default::default(), app: OnceLock::new(), jobs: import::Jobs::new() }
    }
}

pub fn extra_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/setup/trust-local", post(trust_local))
        .route("/ca.crt", get(lan::ca_cert))
        .merge(import::routes())
        .merge(ai::routes())
}

async fn trust_local(State(s): State<Arc<AppState>>, caller: Caller) -> Result<Response, Response> {
    caller.need(Role::Admin)?;
    let s2 = s.clone();
    tokio::task::spawn_blocking(move || lan::trust_on_this_mac(&s2))
        .await
        .unwrap()
        .map_err(|e| server::err(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn lan_status(s: &AppState) -> serde_json::Value {
    serde_json::to_value(&*s.ext.lan.status.read().unwrap()).unwrap_or_default()
}

/// PDF text extraction for search (None = disabled).
pub fn pdf_text() -> Option<vault::PdfText> {
    Some(import::pdf_text)
}

pub fn ai_key_set(_: &AppState) -> bool {
    ai::key_set()
}

/// React to settings changes: start/stop network sharing, keep-awake.
pub fn settings_changed(s: &Arc<AppState>) {
    let (want, awake) = {
        let st = s.settings.read().unwrap();
        (st.lan, st.keep_awake)
    };
    let running = s.ext.lan.status.read().unwrap().running;
    if want && !running {
        if let Some(app) = s.ext.app.get().cloned() {
            let s = s.clone();
            tokio::spawn(async move { lan::start(s, app).await });
        }
    } else if !want && running {
        lan::stop(s);
    } else if running {
        lan::set_keep_awake(s, awake);
    }
}

/// Build the router once and start network sharing (if enabled) plus the IP watcher.
pub async fn start_network(state: Arc<AppState>) -> Router {
    let app = server::router(state.clone());
    let _ = state.ext.app.set(app.clone());
    lan::start(state.clone(), app.clone()).await;
    lan::spawn_ip_watch(state);
    app
}
