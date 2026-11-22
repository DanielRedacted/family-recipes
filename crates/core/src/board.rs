//! The kitchen whiteboard: short notes everyone sees on the home page. A note lasts until a moment
//! the writer's browser chose (the end of today, or of a chosen date) or until someone deletes it.
//! Kept in the app's data folder (`messages.json`).

use crate::auth::{self, Who};
use crate::server::{AppState, err, write_private};
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const FILE: &str = "messages.json";
/// Sent on the change stream (`/api/events`) when the whiteboard changes.
pub const EVENT: &str = "messages";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub id: String,
    pub text: String,
    /// the writer's name when it was written, and their id
    pub author: String,
    pub user: String,
    pub created: u64,
    /// Unix seconds; None = until deleted
    pub expires: Option<u64>,
}

impl Message {
    fn live(&self, now: u64) -> bool {
        self.expires.is_none_or(|e| e > now)
    }
}

#[derive(Default)]
pub struct Store(Mutex<Option<Vec<Message>>>);

type Res = Result<Response, Response>;

/// Run `f` on the notes still showing (expired ones are dropped first); `f` returns whether it changed them.
fn with<R>(s: &AppState, f: impl FnOnce(&mut Vec<Message>) -> Result<(R, bool), Response>) -> Result<R, Response> {
    let mut g = s.ext.board.0.lock().unwrap();
    let list = g.get_or_insert_with(|| std::fs::read(s.data_dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    let now = auth::now();
    let n = list.len();
    list.retain(|m| m.live(now));
    let (r, changed) = f(list)?;
    if changed || list.len() != n {
        write_private(&s.data_dir.join(FILE), &serde_json::to_vec_pretty(&*list).unwrap()).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    if changed {
        let _ = s.events.send(EVENT.into());
    }
    Ok(r)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/messages", get(list).post(post)).route("/api/messages/{id}", delete(remove))
}

/// Newest first.
async fn list(State(s): State<Arc<AppState>>) -> Res {
    let mut l = with(&s, |l| Ok((l.clone(), false)))?;
    l.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(Json(l).into_response())
}

#[derive(Deserialize)]
struct PostBody {
    text: String,
    expires: Option<u64>,
}

async fn post(State(s): State<Arc<AppState>>, Extension(Who(who)): Extension<Who>, Json(b): Json<PostBody>) -> Res {
    let who = who.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "sign in first"))?;
    let text = b.text.trim().to_string();
    if text.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "write something first"));
    }
    if text.chars().count() > 500 {
        return Err(err(StatusCode::BAD_REQUEST, "keep it under 500 characters"));
    }
    if b.expires.is_some_and(|e| e <= auth::now()) {
        return Err(err(StatusCode::BAD_REQUEST, "that date has already passed"));
    }
    let m = Message { id: format!("{:016x}", rand::random::<u64>()), text, author: who.name, user: who.id, created: auth::now(), expires: b.expires };
    with(&s, |l| {
        l.push(m.clone());
        Ok(((), true))
    })?;
    Ok((StatusCode::CREATED, Json(m)).into_response())
}

/// Anyone signed in may wipe any note off the board.
async fn remove(State(s): State<Arc<AppState>>, Extension(Who(who)): Extension<Who>, AxPath(id): AxPath<String>) -> Res {
    who.ok_or_else(|| err(StatusCode::UNAUTHORIZED, "sign in first"))?;
    with(&s, |l| {
        let n = l.len();
        l.retain(|m| m.id != id);
        if l.len() == n {
            return Err(err(StatusCode::NOT_FOUND, "that note is already gone"));
        }
        Ok(((), true))
    })?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn notes_expire_or_stay() {
        let tmp = std::env::temp_dir().join(format!("recipes-board-{}", std::process::id()));
        let cfg = crate::server::Config { vault: tmp.join("v"), data_dir: tmp.join("d"), shell: None, pdf_text: None };
        let s = crate::server::init(cfg).unwrap();
        let mut rx = s.events.subscribe();
        let now = auth::now();
        let note = |id: &str, expires| Message { id: id.into(), text: id.into(), author: "Ann".into(), user: "a".into(), created: now, expires };
        with(&s, |l| {
            l.extend([note("gone", Some(now - 1)), note("today", Some(now + 3600)), note("forever", None)]);
            Ok(((), true))
        })
        .unwrap();
        assert_eq!(rx.try_recv().unwrap(), EVENT);

        *s.ext.board.0.lock().unwrap() = None; // read back from disk
        let ids = with(&s, |l| Ok((l.iter().map(|m| m.id.clone()).collect::<Vec<_>>(), false))).unwrap();
        assert_eq!(ids, ["today", "forever"]);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
