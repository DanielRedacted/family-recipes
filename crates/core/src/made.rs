//! Meals made: "Made today" on a recipe logs it here, and the home page stops suggesting dinners with
//! a protein made in the last few days (`ui/src/lib/made.ts`). Any device may log a meal; recipe
//! files are never touched. Kept in the app's data folder (`made.json`).

use crate::auth;
use crate::server::{AppState, err, write_private};
use crate::vault::VaultPath;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const FILE: &str = "made.json";
/// Sent on the change stream (`/api/events`) when a meal is logged.
pub const EVENT: &str = "made";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Made {
    /// Unix seconds
    pub at: u64,
    pub path: String,
    /// the recipe's title and tags when it was made, so a later rename or edit doesn't change the log
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Default)]
pub struct Store(Mutex<Option<Vec<Made>>>);

type Res = Result<Response, Response>;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/made", get(list).post(post))
}

fn with<R>(s: &AppState, f: impl FnOnce(&mut Vec<Made>) -> R) -> R {
    let mut g = s.ext.made.0.lock().unwrap();
    f(g.get_or_insert_with(|| std::fs::read(s.data_dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()))
}

/// Tags as the reader sees them (`tagsOf` in ui/src/lib/search.ts): a list, or a comma-separated string.
fn tags_of(fm: Option<&serde_json::Value>) -> Vec<String> {
    match fm.and_then(|f| f.get("tags")) {
        Some(serde_json::Value::Array(a)) => a.iter().map(|t| t.as_str().map_or_else(|| t.to_string(), str::to_string)).collect(),
        Some(serde_json::Value::String(t)) => t.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect(),
        _ => vec![],
    }
}

/// Log `path` as made now; returns the whole log.
pub fn log(s: &AppState, path: &str) -> Result<Vec<Made>, Response> {
    let p = VaultPath::parse(path).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let m = {
        let idx = s.index.read().unwrap();
        let e = idx.files.get(&p).filter(|e| e.recipe.is_some()).ok_or_else(|| err(StatusCode::NOT_FOUND, "that recipe is not in the recipes folder"))?;
        Made { at: auth::now(), path: p.as_str().to_string(), title: e.title.clone(), tags: tags_of(e.frontmatter.as_ref()) }
    };
    let all = with(s, |l| {
        l.push(m);
        write_private(&s.data_dir.join(FILE), &serde_json::to_vec_pretty(&*l).unwrap()).map(|_| l.clone())
    })
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let _ = s.events.send(EVENT.into());
    Ok(all)
}

async fn list(State(s): State<Arc<AppState>>) -> Res {
    Ok(Json(with(&s, |l| l.clone())).into_response())
}

#[derive(Deserialize)]
struct PostBody {
    path: String,
}

async fn post(State(s): State<Arc<AppState>>, Json(b): Json<PostBody>) -> Res {
    Ok(Json(log(&s, &b.path)?).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn logs_a_recipe_with_its_tags() {
        let tmp = std::env::temp_dir().join(format!("recipes-made-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("v/Meat")).unwrap();
        std::fs::write(tmp.join("v/Meat/Chops.md"), "---\ntags:\n  - Dinner\n  - Pork\n---\n# Chops\n\n## Ingredients\n\n- 2 pork chops\n\n## Directions\n\n1. Fry.\n").unwrap();
        let cfg = crate::server::Config { vault: tmp.join("v"), data_dir: tmp.join("d"), shell: None, pdf_text: None };
        let s = crate::server::init(cfg).unwrap();
        let mut rx = s.events.subscribe();

        let l = log(&s, "Meat/Chops.md").unwrap();
        assert_eq!((l.len(), l[0].title.as_str(), l[0].tags.clone()), (1, "Chops", vec!["Dinner".to_string(), "Pork".to_string()]));
        assert_eq!(rx.try_recv().unwrap(), EVENT);
        assert!(log(&s, "Meat/Missing.md").is_err());

        *s.ext.made.0.lock().unwrap() = None; // read back from disk
        assert_eq!(with(&s, |l| l.len()), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
