//! People who use the app on phones, tablets and other computers: a name, no password, and the
//! devices they signed in on. Kept in the app's data folder (`users.json`), never in the recipes
//! folder, which syncs to GitHub. Every change a person makes is appended to `activity.log`.

use crate::auth::{self, Actor, Caller, Role};
use crate::server::{AppState, err, write_private};
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use std::sync::{Arc, Mutex};

const FILE: &str = "users.json";
const LOG: &str = "activity.log";
const MAX_DEVICES: usize = 10;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Device {
    /// the browser's User-Agent
    pub ua: String,
    pub ip: String,
    /// last sign-in (Unix seconds)
    pub seen: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    /// random, never reused: a removed person's cookie must not name someone new
    pub id: String,
    pub name: String,
    /// written into recipes as their author or who added them (see ui/src/lib/credit.ts)
    #[serde(default)]
    pub full_name: String,
    pub created: u64,
    #[serde(default)]
    pub devices: Vec<Device>,
}

/// Loaded from disk on first use.
#[derive(Default)]
pub struct Store(Mutex<Option<Vec<User>>>);

type Res = Result<Response, Response>;

fn bad(e: impl std::fmt::Display) -> Response {
    err(StatusCode::BAD_REQUEST, e)
}

fn with<R>(s: &AppState, f: impl FnOnce(&mut Vec<User>) -> R) -> R {
    let mut g = s.ext.users.0.lock().unwrap();
    let list = g.get_or_insert_with(|| std::fs::read(s.data_dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
    f(list)
}

fn save(s: &AppState, list: &[User]) -> Result<(), Response> {
    write_private(&s.data_dir.join(FILE), &serde_json::to_vec_pretty(list).unwrap()).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}

pub fn actor_of(s: &AppState, id: &str) -> Option<Actor> {
    with(s, |l| l.iter().find(|u| u.id == id).map(|u| Actor { id: u.id.clone(), name: u.name.clone(), full_name: u.full_name.clone() }))
}

pub fn name_of(s: &AppState, id: &str) -> Option<String> {
    actor_of(s, id).map(|a| a.name)
}

pub fn clean_full_name(raw: &str) -> Result<String, Response> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    match name.chars().count() {
        0 => Err(bad("type your full name, as it should appear on recipes")),
        n if n > 80 => Err(bad("use at most 80 characters for the full name")),
        _ if name.chars().any(char::is_control) => Err(bad("that full name has characters it can't use")),
        _ => Ok(name),
    }
}

pub fn clean_name(raw: &str) -> Result<String, Response> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    match name.chars().count() {
        0 => Err(bad("type your name")),
        n if n > 40 => Err(bad("use at most 40 characters")),
        _ if name.chars().any(char::is_control) => Err(bad("that name has characters it can't use")),
        _ if name.eq_ignore_ascii_case("admin") => Err(bad("“Admin” is the Mac's own name; choose another")),
        _ => Ok(name),
    }
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn new_user(name: String, full_name: String) -> User {
    User { id: format!("{:016x}", rand::random::<u64>()), name, full_name, created: auth::now(), devices: vec![] }
}

/// Sign in as the person with this name (any letter case), adding them if they're new (a full name
/// is then needed), and remember the device. A person without a full name yet gets the one given.
pub fn sign_in(s: &AppState, name: &str, full_name: &str, ua: &str, ip: &str) -> Result<User, Response> {
    let name = clean_name(name)?;
    with(s, |l| {
        let i = match l.iter().position(|u| same(&u.name, &name)) {
            Some(i) => {
                if l[i].full_name.is_empty() && !full_name.trim().is_empty() {
                    l[i].full_name = clean_full_name(full_name)?;
                }
                i
            }
            None => {
                l.push(new_user(name, clean_full_name(full_name)?));
                l.len() - 1
            }
        };
        let ua: String = ua.chars().take(300).collect();
        let devices = &mut l[i].devices;
        match devices.iter_mut().find(|d| d.ua == ua) {
            Some(d) => {
                d.ip = ip.into();
                d.seen = auth::now();
            }
            None => devices.push(Device { ua, ip: ip.into(), seen: auth::now() }),
        }
        devices.sort_by(|a, b| b.seen.cmp(&a.seen));
        devices.truncate(MAX_DEVICES);
        let u = l[i].clone();
        save(s, l)?;
        Ok(u)
    })
}

pub fn remove(s: &AppState, id: &str) -> Result<(), Response> {
    with(s, |l| {
        let n = l.len();
        l.retain(|u| u.id != id);
        if l.len() == n {
            return Err(err(StatusCode::NOT_FOUND, "no such person"));
        }
        save(s, l)
    })
}

/// Requests that change nothing in the recipes or the app's data.
fn worth_logging(path: &str) -> bool {
    let skip = ["/api/session", "/api/shell/", "/api/ai/"].iter().any(|p| path.starts_with(p));
    // starting an import only reads; saving it writes the recipe
    let import_step = path.starts_with("/api/import/") && !path.ends_with("/save");
    !skip && !import_step
}

/// Append one change to the activity log (called by the guard after a successful write).
// ponytail: one append-only file read whole for each person's list; rotate it if it ever gets big.
// Moves and import saves record the endpoint, not the recipe's name (that is in the request body).
pub fn log_write(s: &AppState, a: &Actor, method: &str, path: &str) {
    use std::os::unix::fs::OpenOptionsExt;
    if !worth_logging(path) {
        return;
    }
    let line = json!({ "t": auth::now(), "user": a.id, "name": a.name, "method": method, "path": path }).to_string() + "\n";
    let file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(s.data_dir.join(LOG));
    if let Err(e) = file.and_then(|mut f| f.write_all(line.as_bytes())) {
        tracing::warn!("activity log: {e}");
    }
}

/// The latest changes by one person, newest first.
pub fn activity_of(s: &AppState, id: &str, limit: usize) -> Vec<Value> {
    let text = std::fs::read_to_string(s.data_dir.join(LOG)).unwrap_or_default();
    let mut items: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v["user"] == id).collect();
    items.reverse();
    items.truncate(limit);
    items
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(list).post(add))
        .route("/api/users/{id}", put(rename).delete(delete))
        .route("/api/users/{id}/activity", get(activity))
}

/// Names for the sign-in page (anyone); the desktop app also gets each person's devices.
async fn list(State(s): State<Arc<AppState>>, Caller(role): Caller) -> Response {
    let mut l = with(&s, |l| l.clone());
    l.sort_by_key(|u| u.name.to_lowercase());
    let out: Vec<Value> = if role == Role::Admin {
        l.into_iter().map(|u| serde_json::to_value(u).unwrap()).collect()
    } else {
        l.into_iter().map(|u| json!({ "id": u.id, "name": u.name, "full_name": u.full_name })).collect()
    };
    Json(out).into_response()
}

#[derive(Deserialize)]
struct NameBody {
    name: String,
    full_name: Option<String>,
}

async fn add(State(s): State<Arc<AppState>>, caller: Caller, Json(b): Json<NameBody>) -> Res {
    caller.need(Role::Admin)?;
    let name = clean_name(&b.name)?;
    let full_name = clean_full_name(b.full_name.as_deref().unwrap_or(""))?;
    with(&s, |l| {
        if l.iter().any(|u| same(&u.name, &name)) {
            return Err(err(StatusCode::CONFLICT, format!("{name} is already here")));
        }
        let u = new_user(name, full_name);
        l.push(u.clone());
        save(&s, l)?;
        Ok(Json(u).into_response())
    })
}

async fn rename(State(s): State<Arc<AppState>>, caller: Caller, AxPath(id): AxPath<String>, Json(b): Json<NameBody>) -> Res {
    caller.need(Role::Admin)?;
    let name = clean_name(&b.name)?;
    let full_name = b.full_name.as_deref().map(clean_full_name).transpose()?;
    with(&s, |l| {
        if l.iter().any(|u| u.id != id && same(&u.name, &name)) {
            return Err(err(StatusCode::CONFLICT, format!("{name} is already here")));
        }
        let u = l.iter_mut().find(|u| u.id == id).ok_or_else(|| err(StatusCode::NOT_FOUND, "no such person"))?;
        u.name = name;
        if let Some(f) = full_name {
            u.full_name = f;
        }
        let u = u.clone();
        save(&s, l)?;
        Ok(Json(u).into_response())
    })
}

async fn delete(State(s): State<Arc<AppState>>, caller: Caller, AxPath(id): AxPath<String>) -> Res {
    caller.need(Role::Admin)?;
    remove(&s, &id)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn activity(State(s): State<Arc<AppState>>, caller: Caller, AxPath(id): AxPath<String>) -> Res {
    caller.need(Role::Admin)?;
    let s2 = s.clone();
    let items = tokio::task::spawn_blocking(move || activity_of(&s2, &id, 200)).await.unwrap_or_default();
    Ok(Json(items).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn people_devices_and_their_changes() {
        let tmp = std::env::temp_dir().join(format!("recipes-users-{}", std::process::id()));
        let cfg = crate::server::Config { vault: tmp.join("v"), data_dir: tmp.join("d"), shell: None, pdf_text: None };
        let s = crate::server::init(cfg).unwrap();

        assert!(sign_in(&s, "Ann", "", "Safari on iPhone", "192.168.1.9").is_err(), "a new person needs a full name");
        let ann = sign_in(&s, "  Ann  ", " Ann   Romo ", "Safari on iPhone", "192.168.1.9").unwrap();
        assert_eq!((ann.name.as_str(), ann.full_name.as_str()), ("Ann", "Ann Romo"));
        let again = sign_in(&s, "ann", "", "Chrome on Mac", "192.168.1.10").unwrap();
        assert_eq!(again.id, ann.id, "names match in any letter case");
        assert_eq!(again.devices.len(), 2);
        assert_eq!(again.full_name, "Ann Romo", "signing in again keeps the full name");
        assert_eq!(sign_in(&s, "ANN", "Someone Else", "Chrome on Mac", "192.168.1.11").unwrap().full_name, "Ann Romo");
        assert_eq!(sign_in(&s, "ANN", "", "Chrome on Mac", "192.168.1.11").unwrap().devices.len(), 2, "same browser, one device");
        assert!(clean_name("").is_err() && clean_name("admin").is_err() && clean_name(&"x".repeat(41)).is_err());

        // kept across restarts
        *s.ext.users.0.lock().unwrap() = None;
        assert_eq!(name_of(&s, &ann.id).as_deref(), Some("Ann"));

        let a = actor_of(&s, &ann.id).unwrap();
        log_write(&s, &a, "PUT", "/api/files/Pies/Pumpkin%20Pie.md");
        log_write(&s, &a, "POST", "/api/session");
        log_write(&s, &a, "POST", "/api/import/url");
        log_write(&s, &a, "POST", "/api/import/abc/save");
        log_write(&s, &Actor { id: "admin".into(), name: "Admin".into(), full_name: String::new() }, "DELETE", "/api/trash");
        let log = activity_of(&s, &ann.id, 10);
        assert_eq!(log.iter().map(|v| v["path"].as_str().unwrap()).collect::<Vec<_>>(), ["/api/import/abc/save", "/api/files/Pies/Pumpkin%20Pie.md"]);

        remove(&s, &ann.id).unwrap();
        assert_eq!(name_of(&s, &ann.id), None);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
