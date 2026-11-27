//! Meals made: "Made today" on a recipe logs it here, and the home page stops suggesting dinners with
//! a protein made in the last few days (`ui/src/lib/made.ts`). Any device may log a meal; recipe
//! files are never touched. Kept in the app's data folder (`made.json`).
//! A recipe kept open a long time without "Made today" is logged as `assumed` (by the browser,
//! `ASSUME_AFTER_MIN` in made.ts); an assumption can be confirmed or removed, a real entry can't.

use crate::auth::{self, Who};
use crate::server::{AppState, Bk, Book, FIRST_BOOK, Listener, err, write_private};
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
    /// the cookbook the recipe is in; entries from before cookbooks are the first one's
    #[serde(default = "first_book")]
    pub book: String,
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

fn first_book() -> String {
    FIRST_BOOK.into()
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

/// Log `path` in cookbook `b` as made now by `by`; returns the whole log.
pub fn log(s: &AppState, b: &Book, path: &str, by: Option<String>, assumed: bool) -> Result<Vec<Made>, Response> {
    let p = VaultPath::parse(path).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let m = {
        let idx = b.index.read().unwrap();
        let e = idx.files.get(&p).filter(|e| e.recipe.is_some()).ok_or_else(|| err(StatusCode::NOT_FOUND, "that recipe is not in this cookbook"))?;
        Made { at: auth::now(), book: b.id.clone(), path: p.as_str().to_string(), title: e.title.clone(), tags: tags_of(e.frontmatter.as_ref()), by, assumed }
    };
    change(s, |l| {
        let recent = |x: &Made| x.book == m.book && x.path == m.path && x.at + SAME_MEAL > m.at;
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

/// One history for every cookbook; phones and tablets don't see meals from a Mac-only one.
async fn list(State(s): State<Arc<AppState>>, Extension(l): Extension<Listener>) -> Res {
    let hidden: Vec<String> = if l == Listener::Loopback { vec![] } else { s.settings.read().unwrap().cookbooks.iter().filter(|c| c.mac_only).map(|c| c.id.clone()).collect() };
    Ok(Json(with(&s, |m| m.iter().filter(|x| !hidden.contains(&x.book)).cloned().collect::<Vec<_>>())).into_response())
}

#[derive(Deserialize)]
struct PostBody {
    path: String,
    #[serde(default)]
    assumed: bool,
}

async fn post(State(s): State<Arc<AppState>>, Bk(book): Bk, Extension(Who(who)): Extension<Who>, Json(b): Json<PostBody>) -> Res {
    Ok(Json(log(&s, &book, &b.path, who.map(|a| a.name), b.assumed)?).into_response())
}

/// An entry is named by its time and recipe (and its cookbook, `?book=`).
#[derive(Deserialize)]
struct Which {
    path: String,
}

/// Find the assumed entry `at`/`path` in cookbook `book` and apply `f` to the log at its index.
fn assumption(s: &AppState, book: &str, at: u64, path: &str, f: impl FnOnce(&mut Vec<Made>, usize)) -> Result<Vec<Made>, Response> {
    change(s, |l| {
        let i = l.iter().position(|m| m.at == at && m.book == book && m.path == path).ok_or_else(|| err(StatusCode::NOT_FOUND, "that meal is no longer in the log"))?;
        if !l[i].assumed {
            return Err(err(StatusCode::BAD_REQUEST, "only a guessed meal can be changed"));
        }
        f(l, i);
        Ok(true)
    })
}

/// "Yes, cooked": the assumption becomes a real entry, at the time it was guessed.
async fn confirm(State(s): State<Arc<AppState>>, Bk(b): Bk, Extension(Who(who)): Extension<Who>, AxPath(at): AxPath<u64>, Query(q): Query<Which>) -> Res {
    let all = assumption(&s, &b.id, at, &q.path, |l, i| {
        l[i].assumed = false;
        if let Some(a) = who {
            l[i].by = Some(a.name);
        }
    })?;
    Ok(Json(all).into_response())
}

/// "Not cooked": the assumption is removed.
async fn remove(State(s): State<Arc<AppState>>, Bk(b): Bk, AxPath(at): AxPath<u64>, Query(q): Query<Which>) -> Res {
    let all = assumption(&s, &b.id, at, &q.path, |l, i| {
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
        let b = s.books.read().unwrap()[0].clone();
        let mut rx = s.events.subscribe();

        let l = log(&s, &b, "Meat/Chops.md", Some("Ann".into()), false).unwrap();
        assert_eq!((l.len(), l[0].title.as_str(), l[0].tags.clone()), (1, "Chops", vec!["Dinner".to_string(), "Pork".to_string()]));
        assert_eq!(l[0].by.as_deref(), Some("Ann"));
        assert_eq!(rx.try_recv().unwrap(), EVENT);
        assert!(log(&s, &b, "Meat/Missing.md", None, false).is_err());

        *s.ext.made.0.lock().unwrap() = None; // read back from disk
        assert_eq!(with(&s, |l| (l.len(), l[0].book.clone())), (1, FIRST_BOOK.to_string()));

        // one history for every cookbook: the same path in another is another meal, not "already made"
        std::fs::create_dir_all(tmp.join("w/Meat")).unwrap();
        std::fs::copy(tmp.join("v/Meat/Chops.md"), tmp.join("w/Meat/Chops.md")).unwrap();
        let c = crate::server::Cookbook { id: "b2".into(), path: tmp.join("w"), git_sync: false, mac_only: true };
        s.settings.write().unwrap().cookbooks.push(c.clone());
        let b2 = s.open(&c).unwrap();
        let l = log(&s, &b2, "Meat/Chops.md", None, true).unwrap();
        assert_eq!(l.iter().map(|m| m.book.as_str()).collect::<Vec<_>>(), [FIRST_BOOK, "b2"]);
        assert!(log(&s, &b2, "Meat/Missing.md", None, false).is_err(), "checked against its own cookbook");
        // a meal saved before cookbooks belongs to the first
        let old: Made = serde_json::from_str(r#"{"at":1,"path":"Meat/Chops.md","title":"Chops","tags":[]}"#).unwrap();
        assert_eq!(old.book, FIRST_BOOK);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn assumptions_give_way_to_facts() {
        let tmp = std::env::temp_dir().join(format!("recipes-made-guess-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("v")).unwrap();
        std::fs::write(tmp.join("v/Chops.md"), "# Chops\n\n## Ingredients\n\n- 2 pork chops\n\n## Directions\n\n1. Fry.\n").unwrap();
        let cfg = crate::server::Config { vault: tmp.join("v"), data_dir: tmp.join("d"), shell: None, pdf_text: None };
        let s = crate::server::init(cfg).unwrap();
        let b = s.books.read().unwrap()[0].clone();

        // a guess, then no second guess the same day
        let l = log(&s, &b, "Chops.md", Some("Ann".into()), true).unwrap();
        assert!(l[0].assumed);
        assert_eq!(log(&s, &b, "Chops.md", Some("Ann".into()), true).unwrap().len(), 1);
        // pressing "Made today" replaces the guess
        let l = log(&s, &b, "Chops.md", Some("Bob".into()), false).unwrap();
        assert_eq!((l.len(), l[0].assumed, l[0].by.as_deref()), (1, false, Some("Bob")));
        // and a real entry can't be removed or "confirmed"
        assert!(assumption(&s, &b.id, l[0].at, "Chops.md", |_, _| {}).is_err());

        // a guess the same day as a real entry is not made either
        with(&s, |l| l.clear());
        let l = log(&s, &b, "Chops.md", None, true).unwrap();
        let at = l[0].at;
        assert_eq!(assumption(&s, &b.id, at, "Chops.md", |l, i| l[i].assumed = false).unwrap()[0].assumed, false);
        with(&s, |l| l[0].assumed = true);
        assert!(assumption(&s, &b.id, at, "Chops.md", |l, i| { l.remove(i); }).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
