//! Meals made: "Made today" on a recipe logs it here, and the home page stops suggesting dinners with
//! a protein made in the last few days (`ui/src/lib/made.ts`). Any device may log a meal; recipe
//! files are never touched. Kept in the app's data folder (`made.json`).
//! A recipe kept open a long time without "Made today" is logged as `assumed` (by the browser,
//! `ASSUME_AFTER_MIN` in made.ts); an assumption can be confirmed or removed, a real entry can't.

use crate::auth::{self, Who};
use crate::server::{AppState, err, write_private};
use crate::vault::VaultPath;
use axum::extract::{Path as AxPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Extension, Json, Router};
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
    /// who logged it (their name then); None for entries from before this was kept
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// logged because the recipe stayed open, not because someone said so
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub assumed: bool,
}

/// An assumption is not made when the recipe was logged this recently, and a real entry replaces
/// an assumption this recent (the server doesn't know the browser's day, so "about today").
const SAME_MEAL: u64 = 12 * 3600;

#[derive(Default)]
pub struct Store(Mutex<Option<Vec<Made>>>);

type Res = Result<Response, Response>;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/made", get(list).post(post)).route("/api/made/{at}", put(confirm).delete(remove))
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

/// Run `f` on the log; when it returns true the log is saved and the change announced. Returns the whole log.
fn change(s: &AppState, f: impl FnOnce(&mut Vec<Made>) -> Result<bool, Response>) -> Result<Vec<Made>, Response> {
    let (all, changed) = with(s, |l| {
        let changed = f(l)?;
        if changed {
            write_private(&s.data_dir.join(FILE), &serde_json::to_vec_pretty(&*l).unwrap()).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        }
        Ok::<_, Response>((l.clone(), changed))
    })?;
    if changed {
        let _ = s.events.send(EVENT.into());
    }
    Ok(all)
}

/// Log `path` as made now by `by`; returns the whole log.
pub fn log(s: &AppState, path: &str, by: Option<String>, assumed: bool) -> Result<Vec<Made>, Response> {
    let p = VaultPath::parse(path).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let m = {
        let idx = s.index.read().unwrap();
        let e = idx.files.get(&p).filter(|e| e.recipe.is_some()).ok_or_else(|| err(StatusCode::NOT_FOUND, "that recipe is not in the recipes folder"))?;
        Made { at: auth::now(), path: p.as_str().to_string(), title: e.title.clone(), tags: tags_of(e.frontmatter.as_ref()), by, assumed }
    };
    change(s, |l| {
        let recent = |x: &Made| x.path == m.path && x.at + SAME_MEAL > m.at;
        if assumed && l.iter().any(recent) {
            return Ok(false);
        }
        if !assumed {
            l.retain(|x| !(x.assumed && recent(x)));
        }
        l.push(m);
        Ok(true)
    })
}

async fn list(State(s): State<Arc<AppState>>) -> Res {
    Ok(Json(with(&s, |l| l.clone())).into_response())
}

#[derive(Deserialize)]
struct PostBody {
    path: String,
    #[serde(default)]
    assumed: bool,
}

async fn post(State(s): State<Arc<AppState>>, Extension(Who(who)): Extension<Who>, Json(b): Json<PostBody>) -> Res {
    Ok(Json(log(&s, &b.path, who.map(|a| a.name), b.assumed)?).into_response())
}

/// An entry is named by its time and recipe.
#[derive(Deserialize)]
struct Which {
    path: String,
}

/// Find the assumed entry `at`/`path` and apply `f` to the log at its index.
fn assumption(s: &AppState, at: u64, path: &str, f: impl FnOnce(&mut Vec<Made>, usize)) -> Result<Vec<Made>, Response> {
    change(s, |l| {
        let i = l.iter().position(|m| m.at == at && m.path == path).ok_or_else(|| err(StatusCode::NOT_FOUND, "that meal is no longer in the log"))?;
        if !l[i].assumed {
            return Err(err(StatusCode::BAD_REQUEST, "only a guessed meal can be changed"));
        }
        f(l, i);
        Ok(true)
    })
}

/// "Yes, cooked": the assumption becomes a real entry, at the time it was guessed.
async fn confirm(State(s): State<Arc<AppState>>, Extension(Who(who)): Extension<Who>, AxPath(at): AxPath<u64>, Query(q): Query<Which>) -> Res {
    let all = assumption(&s, at, &q.path, |l, i| {
        l[i].assumed = false;
        if let Some(a) = who {
            l[i].by = Some(a.name);
        }
    })?;
    Ok(Json(all).into_response())
}

/// "Not cooked": the assumption is removed.
async fn remove(State(s): State<Arc<AppState>>, AxPath(at): AxPath<u64>, Query(q): Query<Which>) -> Res {
    let all = assumption(&s, at, &q.path, |l, i| {
        l.remove(i);
    })?;
    Ok(Json(all).into_response())
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

        let l = log(&s, "Meat/Chops.md", Some("Ann".into()), false).unwrap();
        assert_eq!((l.len(), l[0].title.as_str(), l[0].tags.clone()), (1, "Chops", vec!["Dinner".to_string(), "Pork".to_string()]));
        assert_eq!(l[0].by.as_deref(), Some("Ann"));
        assert_eq!(rx.try_recv().unwrap(), EVENT);
        assert!(log(&s, "Meat/Missing.md", None, false).is_err());

        *s.ext.made.0.lock().unwrap() = None; // read back from disk
        assert_eq!(with(&s, |l| l.len()), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn assumptions_give_way_to_facts() {
        let tmp = std::env::temp_dir().join(format!("recipes-made-guess-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("v")).unwrap();
        std::fs::write(tmp.join("v/Chops.md"), "# Chops\n\n## Ingredients\n\n- 2 pork chops\n\n## Directions\n\n1. Fry.\n").unwrap();
        let cfg = crate::server::Config { vault: tmp.join("v"), data_dir: tmp.join("d"), shell: None, pdf_text: None };
        let s = crate::server::init(cfg).unwrap();

        // a guess, then no second guess the same day
        let l = log(&s, "Chops.md", Some("Ann".into()), true).unwrap();
        assert!(l[0].assumed);
        assert_eq!(log(&s, "Chops.md", Some("Ann".into()), true).unwrap().len(), 1);
        // pressing "Made today" replaces the guess
        let l = log(&s, "Chops.md", Some("Bob".into()), false).unwrap();
        assert_eq!((l.len(), l[0].assumed, l[0].by.as_deref()), (1, false, Some("Bob")));
        // and a real entry can't be removed or "confirmed"
        assert!(assumption(&s, l[0].at, "Chops.md", |_, _| {}).is_err());

        // a guess the same day as a real entry is not made either
        with(&s, |l| l.clear());
        let l = log(&s, "Chops.md", None, true).unwrap();
        let at = l[0].at;
        assert_eq!(assumption(&s, at, "Chops.md", |l, i| l[i].assumed = false).unwrap()[0].assumed, false);
        with(&s, |l| l[0].assumed = true);
        assert!(assumption(&s, at, "Chops.md", |l, i| { l.remove(i); }).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
