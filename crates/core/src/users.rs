//! People who use the app on phones, tablets and other computers: a name, no password, the
//! devices they signed in on, their favorite recipes and an optional picture. Kept in the app's
//! data folder (`users.json`), never in the recipes folder, which syncs to GitHub. Every change a
//! person makes is appended to `activity.log`.
//!
//! What a person contributed is not kept here: it is read from the recipes' own credits (`author`,
//! `added_by`, `changes`; see ui/src/lib/people.ts), so it outlives the account.

use crate::auth::{self, Actor, Caller, Role, Who};
use crate::server::{AppState, Bk, FIRST_BOOK, Listener, book_param, err, write_private};
use axum::extract::{Path as AxPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

const FILE: &str = "users.json";
const LOG: &str = "activity.log";
/// The log is renamed to this once it passes `LOG_LIMIT`, replacing the one before.
const OLD_LOG: &str = "activity.log.1";
/// about 8,000 changes; the two files together keep the latest 8,000 to 16,000
const LOG_LIMIT: u64 = 1_000_000;
const MAX_DEVICES: usize = 10;
const MAX_FAVORITES: usize = 2000;
/// Sent on the change stream (`/api/events`) when anyone's name, favorites or picture changes.
pub const EVENT: &str = "users";

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
    /// recipe paths this person marked as a favorite in the first cookbook (`FIRST_BOOK`, as
    /// before cookbooks); `favorites_in` has the other cookbooks', by id
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub favorites_in: BTreeMap<String, Vec<String>>,
    /// an emoji, or a small `data:image/…;base64,…` picture
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// names this person was credited under before a rename, so older recipes still count as theirs
    #[serde(default)]
    pub former: Vec<String>,
}

impl User {
    /// This person's favorites in cookbook `book`.
    pub fn favs(&self, book: &str) -> &[String] {
        if book == FIRST_BOOK { &self.favorites } else { self.favorites_in.get(book).map_or(&[], Vec::as_slice) }
    }
    fn favs_mut(&mut self, book: &str) -> &mut Vec<String> {
        if book == FIRST_BOOK { &mut self.favorites } else { self.favorites_in.entry(book.to_string()).or_default() }
    }
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
    write_private(&s.data_dir.join(FILE), &serde_json::to_vec_pretty(list).unwrap()).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let _ = s.events.send(EVENT.into());
    Ok(())
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
    User { id: format!("{:016x}", rand::random::<u64>()), name, full_name, created: auth::now(), devices: vec![], favorites: vec![], favorites_in: BTreeMap::new(), picture: None, former: vec![] }
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

/// A file or folder in cookbook `book` moved from `from` to `to`: favorites follow it.
pub fn moved(s: &AppState, book: &str, from: &str, to: &str) {
    let follow = |p: &str| (p == from).then(|| to.to_string()).or_else(|| p.strip_prefix(from).filter(|r| r.starts_with('/')).map(|r| format!("{to}{r}")));
    with(s, |l| {
        let mut changed = false;
        for u in l.iter_mut() {
            for p in u.favs_mut(book).iter_mut() {
                if let Some(n) = follow(p) {
                    *p = n;
                    changed = true;
                }
            }
        }
        if changed && let Err(e) = save(s, l) {
            tracing::warn!("favorites after a move: {:?}", e.status());
        }
    });
}

/// Apply `add` then `remove` to a person's favorites (adding keeps the order, without repeats).
fn edit_favorites(list: &mut Vec<String>, add: &[String], remove: &[String]) -> Result<(), Response> {
    if add.iter().any(|p| p.is_empty() || p.len() > 512 || p.chars().any(char::is_control)) {
        return Err(bad("that isn't a recipe"));
    }
    for p in add {
        if !list.contains(p) {
            list.push(p.clone());
        }
    }
    list.retain(|p| !remove.contains(p));
    if list.len() > MAX_FAVORITES {
        return Err(bad(format!("keep at most {MAX_FAVORITES} favorites")));
    }
    Ok(())
}

/// An emoji, or a JPEG, PNG or WebP picture as a data URL of at most about 100 KB.
fn clean_picture(raw: &str) -> Result<String, Response> {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("data:") {
        let data = ["image/jpeg;base64,", "image/png;base64,", "image/webp;base64,"].iter().find_map(|t| rest.strip_prefix(t));
        return match data {
            None => Err(bad("use a JPEG, PNG or WebP picture")),
            Some(d) if d.len() > 140_000 => Err(bad("that picture is too large")),
            Some(d) if d.is_empty() || !d.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)) => Err(bad("that picture can't be read")),
            Some(_) => Ok(raw.to_string()),
        };
    }
    match raw.chars().count() {
        0 => Err(bad("choose a picture or an emoji")),
        n if n > 16 => Err(bad("use one emoji")),
        _ if raw.chars().any(|c| c.is_control() || c.is_ascii_alphanumeric() || c == '<') => Err(bad("use one emoji")),
        _ => Ok(raw.to_string()),
    }
}

/// Requests that change nothing in the recipes or the app's data.
fn worth_logging(path: &str) -> bool {
    let skip = ["/api/session", "/api/shell/", "/api/ai/"].iter().any(|p| path.starts_with(p));
    // starting an import only reads; saving it writes the recipe
    let import_step = path.starts_with("/api/import/") && !path.ends_with("/save");
    !skip && !import_step
}

/// Append one change to the activity log (called by the guard after a successful write). `book`:
/// the request's `?book=`, kept when it names a cookbook other than the first.
// Moves and import saves record the endpoint, not the recipe's name (that is in the request body).
pub fn log_write(s: &AppState, a: &Actor, method: &str, path: &str, book: Option<&str>) {
    use std::os::unix::fs::OpenOptionsExt;
    if !worth_logging(path) {
        return;
    }
    let mut v = json!({ "t": auth::now(), "user": a.id, "name": a.name, "method": method, "path": path });
    if let Some(b) = book.filter(|b| *b != FIRST_BOOK && b.len() <= 16 && b.bytes().all(|c| c.is_ascii_hexdigit())) {
        v["book"] = json!(b);
    }
    let line = v.to_string() + "\n";
    let log = s.data_dir.join(LOG);
    if std::fs::metadata(&log).is_ok_and(|m| m.len() > LOG_LIMIT) {
        if let Err(e) = std::fs::rename(&log, s.data_dir.join(OLD_LOG)) {
            tracing::warn!("activity log: {e}");
        }
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(&log);
    if let Err(e) = file.and_then(|mut f| f.write_all(line.as_bytes())) {
        tracing::warn!("activity log: {e}");
    }
}

/// The latest changes by one person, newest first.
pub fn activity_of(s: &AppState, id: &str, limit: usize) -> Vec<Value> {
    let read = |f: &str| std::fs::read_to_string(s.data_dir.join(f)).unwrap_or_default();
    let text = read(OLD_LOG) + &read(LOG);
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
        .route("/api/users/{id}/picture", put(set_picture))
        .route("/api/favorites", post(favorites))
}

/// Names for the sign-in page (anyone). Signed-in people also see each other's pages: when they
/// joined, their favorites (in the request's cookbook), picture and former names. Only the desktop
/// app gets their devices.
async fn list(State(s): State<Arc<AppState>>, Caller(role): Caller, Extension(Who(who)): Extension<Who>, Extension(lis): Extension<Listener>, uri: axum::http::Uri) -> Response {
    // the sign-in page works whatever the address says; an unknown cookbook just has no favorites
    let book = s.requested(book_param(&uri), lis).map(|b| b.id.clone()).unwrap_or_default();
    let mut l = with(&s, |l| l.clone());
    l.sort_by_key(|u| u.name.to_lowercase());
    let out: Vec<Value> = l
        .into_iter()
        .map(|u| match (role, &who) {
            (Role::Admin, _) => {
                let mut v = json!({ "id": u.id, "name": u.name, "full_name": u.full_name, "created": u.created, "devices": u.devices, "picture": u.picture, "former": u.former });
                v["favorites"] = json!(u.favs(&book));
                v
            }
            (_, Some(_)) => json!({ "id": u.id, "name": u.name, "full_name": u.full_name, "created": u.created, "favorites": u.favs(&book), "picture": u.picture, "former": u.former }),
            (_, None) => json!({ "id": u.id, "name": u.name, "full_name": u.full_name }),
        })
        .collect();
    Json(out).into_response()
}

#[derive(Deserialize)]
struct FavoritesBody {
    #[serde(default)]
    add: Vec<String>,
    #[serde(default)]
    remove: Vec<String>,
}

/// Add or remove the signed-in person's favorites (not a whole list, so two devices can't undo each other).
async fn favorites(State(s): State<Arc<AppState>>, Bk(book): Bk, Extension(Who(who)): Extension<Who>, Json(b): Json<FavoritesBody>) -> Res {
    let id = who.map(|a| a.id).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "sign in first"))?;
    with(&s, |l| {
        let u = l.iter_mut().find(|u| u.id == id).ok_or_else(|| err(StatusCode::NOT_FOUND, "the Mac keeps its own favorites; choose someone in Editing as"))?;
        let favs = u.favs_mut(&book.id);
        edit_favorites(favs, &b.add, &b.remove)?;
        let out = favs.clone();
        save(&s, l)?;
        Ok(Json(out).into_response())
    })
}

#[derive(Deserialize)]
struct PictureBody {
    picture: Option<String>,
}

/// A person changes their own picture; the Mac can change anyone's.
async fn set_picture(State(s): State<Arc<AppState>>, Caller(role): Caller, Extension(Who(who)): Extension<Who>, AxPath(id): AxPath<String>, Json(b): Json<PictureBody>) -> Res {
    if role != Role::Admin && who.as_ref().is_none_or(|a| a.id != id) {
        return Err(err(StatusCode::FORBIDDEN, "you can change only your own picture"));
    }
    let picture = b.picture.as_deref().map(clean_picture).transpose()?;
    with(&s, |l| {
        let u = l.iter_mut().find(|u| u.id == id).ok_or_else(|| err(StatusCode::NOT_FOUND, "no such person"))?;
        u.picture = picture;
        save(&s, l)?;
        Ok(StatusCode::NO_CONTENT.into_response())
    })
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

/// The name their recipes are credited under (ops.ts `signer`: the full name, else the name).
fn credited(u: &User) -> &str {
    if u.full_name.is_empty() { &u.name } else { &u.full_name }
}

/// Rename a person, keeping the name they were credited under so their older recipes still count.
fn set_names(u: &mut User, name: String, full_name: Option<String>) {
    let before = credited(u).to_string();
    u.name = name;
    if let Some(f) = full_name {
        u.full_name = f;
    }
    let now = credited(u).to_string();
    if !same(&before, &now) && !u.former.iter().any(|f| same(f, &before)) {
        u.former.push(before);
    }
    u.former.retain(|f| !same(f, &now));
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
        set_names(u, name, full_name);
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

    fn favs_after() -> Vec<String> {
        vec!["Desserts/Pies/Pumpkin Pie.md".into(), "Desserts/Pies/Apple Tart.md".into()]
    }

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
        log_write(&s, &a, "PUT", "/api/files/Pies/Pumpkin%20Pie.md", None);
        log_write(&s, &a, "POST", "/api/session", None);
        log_write(&s, &a, "POST", "/api/import/url", None);
        log_write(&s, &a, "POST", "/api/import/abc/save", Some("ab12cd34"));
        log_write(&s, &Actor { id: "admin".into(), name: "Admin".into(), full_name: String::new() }, "DELETE", "/api/trash", None);
        let log = activity_of(&s, &ann.id, 10);
        assert_eq!(log.iter().map(|v| v["path"].as_str().unwrap()).collect::<Vec<_>>(), ["/api/import/abc/save", "/api/files/Pies/Pumpkin%20Pie.md"]);
        assert_eq!((log[0]["book"].as_str(), log[1].get("book")), (Some("ab12cd34"), None), "the cookbook is kept unless it's the first");

        // a full log moves to activity.log.1 and the person's list still reads both
        let filler = format!("{}\n", json!({ "user": "someone-else", "path": "x".repeat(200) })).repeat(5000);
        std::fs::OpenOptions::new().append(true).open(s.data_dir.join(LOG)).unwrap().write_all(filler.as_bytes()).unwrap();
        log_write(&s, &a, "PUT", "/api/files/Pies/Apple%20Pie.md", Some(FIRST_BOOK));
        assert!(s.data_dir.join(OLD_LOG).is_file());
        assert!(std::fs::metadata(s.data_dir.join(LOG)).unwrap().len() < 1000);
        let log = activity_of(&s, &ann.id, 10);
        assert_eq!(log.iter().map(|v| v["path"].as_str().unwrap()).collect::<Vec<_>>(), ["/api/files/Pies/Apple%20Pie.md", "/api/import/abc/save", "/api/files/Pies/Pumpkin%20Pie.md"]);

        // favorites: added once each, removed, and following a renamed recipe or folder
        let mut favs = vec![];
        edit_favorites(&mut favs, &["Pies/Pumpkin Pie.md".into(), "Soups/Minestrone.md".into(), "Pies/Pumpkin Pie.md".into()], &[]).unwrap();
        edit_favorites(&mut favs, &["Pies/Apple Pie.md".into()], &["Soups/Minestrone.md".into()]).unwrap();
        assert_eq!(favs, ["Pies/Pumpkin Pie.md", "Pies/Apple Pie.md"]);
        assert!(edit_favorites(&mut favs, &["".into()], &[]).is_err());
        with(&s, |l| l[0].favorites = favs.clone());
        moved(&s, FIRST_BOOK, "Pies", "Desserts/Pies");
        moved(&s, FIRST_BOOK, "Desserts/Pies/Apple Pie.md", "Desserts/Pies/Apple Tart.md");
        moved(&s, FIRST_BOOK, "Pi", "Nothing"); // a prefix that isn't a folder moves nothing
        assert_eq!(with(&s, |l| l[0].favorites.clone()), ["Desserts/Pies/Pumpkin Pie.md", "Desserts/Pies/Apple Tart.md"]);

        // each cookbook has its own: the same path in another cookbook is another favorite, and a
        // move there leaves the first cookbook's alone
        with(&s, |l| l[0].favs_mut("b2").push("Desserts/Pies/Pumpkin Pie.md".into()));
        moved(&s, "b2", "Desserts", "Sweets");
        assert_eq!(with(&s, |l| (l[0].favs(FIRST_BOOK).to_vec(), l[0].favs("b2").to_vec())), (favs_after(), vec!["Sweets/Pies/Pumpkin Pie.md".to_string()]));
        *s.ext.users.0.lock().unwrap() = None; // and both are read back from disk
        assert_eq!(with(&s, |l| l[0].favs("b2").len()), 1);
        // a users.json from before cookbooks still reads, as the first cookbook's
        let old: User = serde_json::from_str(r#"{"id":"x","name":"Bo","created":1,"favorites":["Pie.md"]}"#).unwrap();
        assert_eq!((old.favs(FIRST_BOOK), old.favs("b2").len()), (&["Pie.md".to_string()][..], 0));

        // pictures: one emoji or a small raster data URL, never markup
        assert_eq!(clean_picture(" 🥧 ").unwrap(), "🥧");
        assert!(clean_picture("data:image/jpeg;base64,/9j/4AAQ").is_ok());
        for bad_one in ["", "Ann", "<b>", "data:image/svg+xml;base64,PHN2Zz4=", "data:image/png;base64,<script>", &format!("data:image/png;base64,{}", "A".repeat(140_001))] {
            assert!(clean_picture(bad_one).is_err(), "{bad_one:.40}");
        }

        // a new full name keeps the old one, so recipes credited to it still count
        let mut u = new_user("Ann".into(), "Ann Romo".into());
        set_names(&mut u, "Annie".into(), None);
        assert!(u.former.is_empty(), "a new short name changes no credits");
        set_names(&mut u, "Annie".into(), Some("Ann Smith".into()));
        set_names(&mut u, "Annie".into(), Some("ann smith".into()));
        assert_eq!(u.former, ["Ann Romo"]);
        set_names(&mut u, "Annie".into(), Some("Ann Romo".into()));
        assert_eq!(u.former, ["ann smith"], "back to an old name: it is current again, not former");

        remove(&s, &ann.id).unwrap();
        assert_eq!(name_of(&s, &ann.id), None);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
