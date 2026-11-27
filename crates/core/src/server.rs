//! HTTP API + embedded UI. One router serves the desktop window (loopback) and LAN devices.

use crate::auth::{self, Caller, Role, Who};
use crate::vault::{self, Index, PdfText, VaultPath};
use anyhow::{Context, Result};
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path as AxPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router, middleware};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

#[derive(RustEmbed)]
#[folder = "../../ui/dist"]
struct Ui;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Listener {
    Loopback,
    Lan,
    /// Plain HTTP on the home network (port 80): no certificate needed on the device.
    LanHttp,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub base_url: String,
}
fn default_provider() -> String {
    "anthropic".into()
}
impl Default for AiSettings {
    fn default() -> Self {
        Self { enabled: false, provider: default_provider(), model: String::new(), base_url: String::new() }
    }
}

/// Show the Mac's calendar events (calendar.rs). Read-only; off by default.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CalendarSettings {
    #[serde(default)]
    pub enabled: bool,
    /// calendar ids to show; none until the owner ticks one, so a new calendar stays private
    #[serde(default)]
    pub calendars: Vec<String>,
}

/// One cookbook: a recipes folder the app serves (Settings → Cookbooks).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cookbook {
    pub id: String,
    pub path: PathBuf,
    /// Sync this folder with its GitHub repository (see git.rs).
    #[serde(default)]
    pub git_sync: bool,
    /// Phones and tablets don't see it; only the desktop window does.
    #[serde(default)]
    pub mac_only: bool,
}

/// The id of the cookbook a settings file from before cookbooks describes (its `vault` and
/// `git_sync`); meals and favorites saved then belong to it. Otherwise an id like any other.
pub const FIRST_BOOK: &str = "1";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    /// In the order they were added; a device that hasn't chosen opens the first it may see.
    #[serde(default)]
    pub cookbooks: Vec<Cookbook>,
    /// Before cookbooks: the one recipes folder. Read once into `cookbooks`, never written.
    #[serde(default, skip_serializing)]
    pub vault: Option<PathBuf>,
    #[serde(default)]
    pub pin_hash: Option<String>,
    #[serde(default)]
    pub pin_gen: u64,
    #[serde(default)]
    pub ai: AiSettings,
    #[serde(default = "yes")]
    pub lan: bool,
    #[serde(default)]
    pub keep_awake: bool,
    /// The first-start setup was finished (a settings file from before it existed counts as finished).
    #[serde(default = "yes")]
    pub setup_done: bool,
    /// Before cookbooks: sync for the one recipes folder. Read once into `cookbooks`, never written.
    #[serde(default, skip_serializing)]
    pub git_sync: bool,
    /// The person the desktop window edits as (users.rs); None = the built-in "Admin".
    #[serde(default)]
    pub editing_as: Option<String>,
    #[serde(default)]
    pub calendar: CalendarSettings,
    /// What the desktop app does when it finds a newer release.
    #[serde(default)]
    pub updates: Updates,
    /// The app's colour for every device (`#rrggbb`); None = the built-in purple. A device may pick its own.
    #[serde(default)]
    pub accent: Option<String>,
    /// Who uses the app, and what they call themselves.
    #[serde(default)]
    pub group: Group,
}
/// Settings → General → Group. The name is only ever what the owner typed: empty until then.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub kind: GroupKind,
    #[serde(default)]
    pub name: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupKind {
    #[default]
    Family,
    Friends,
    Team,
    Other,
}
/// `#rrggbb` in lower case, "" for none; anything else is refused.
fn clean_accent(s: &str) -> Result<Option<String>, &'static str> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let hex = s.strip_prefix('#').filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    hex.map(|h| Some(format!("#{}", h.to_ascii_lowercase()))).ok_or("the colour must look like #7a52cc")
}
/// The group's name, trimmed, at most 60 characters.
fn clean_group(mut g: Group) -> Group {
    g.name = g.name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect();
    g
}
/// Settings → App → Updates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Updates {
    /// Say a new version is available; download it when asked.
    #[default]
    Notify,
    /// Download it in the background, so "Update and restart" is quick.
    Download,
    /// Download and install it in the background, then restart while the window is closed.
    Install,
}
fn yes() -> bool {
    true
}
impl Default for Settings {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}

/// Things only the desktop shell can do (implemented by the Tauri app).
pub trait Shell: Send + Sync {
    fn reveal(&self, abs: &Path);
    fn choose_folder(&self) -> Option<PathBuf>;
    fn print(&self);
    /// Open a web page in the default browser.
    fn open_url(&self, _url: &str) {}
    /// Query (None) or set "open at login"; returns the current state.
    fn autostart(&self, _set: Option<bool>) -> bool {
        false
    }
    /// Quit and start again (into an installed update).
    fn restart(&self) {}
    /// Look for a newer release: Some((version, release notes, ready)), or None when up to date.
    /// With `Updates::Download` or `Install` it is also downloaded (and installed); `ready` says a
    /// restart is all that's left.
    fn update_check(&self, _mode: Updates) -> Result<Option<(String, String, bool)>, String> {
        Err("updates come with the desktop app".into())
    }
    /// Install the newer release (unless it already is), then restart the app. `quiet`: only while
    /// the window is closed; Ok(false) when it is open.
    fn update_install(&self, _quiet: bool) -> Result<bool, String> {
        Err("updates come with the desktop app".into())
    }
    /// The Downloads folder, where the recipe book is saved.
    fn downloads(&self) -> Option<PathBuf> {
        None
    }
    /// Print the window as it looks on paper to a PDF file at `to`, without the print dialog; returns once
    /// the file is written.
    fn save_pdf(&self, _to: &Path) -> Result<(), String> {
        Err("PDF files are made by the desktop app".into())
    }
}

/// `dir/name`, or `dir/stem (2).ext`, `(3)`… when that is taken.
pub fn unique_file(dir: &Path, name: &str) -> PathBuf {
    let p = Path::new(name);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut target = dir.join(name);
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    target
}

pub struct Payload {
    pub etag: String,
    pub json: Bytes,
}

pub struct AppState {
    pub data_dir: PathBuf,
    /// The open cookbooks, in the order of `settings.cookbooks` (one whose folder is missing isn't open).
    pub books: RwLock<Vec<Arc<Book>>>,
    /// Changes everyone hears about: the names of what changed (whiteboard, meals, people, update).
    /// A cookbook's own changes go on its `Book::events`.
    pub events: broadcast::Sender<String>,
    pub settings: RwLock<Settings>,
    pub launch_token: String,
    pub cookie_key: Vec<u8>,
    pub limiter: auth::PinLimiter,
    pub lan_hosts: RwLock<Vec<String>>,
    pub loopback_port: AtomicU16,
    pub shell: Option<Arc<dyn Shell>>,
    pub pdf_text: Option<PdfText>,
    pub ext: crate::Extensions,
}

/// An open cookbook: its folder, the index of it, and what watches and syncs it.
pub struct Book {
    pub id: String,
    /// the folder's name
    pub name: String,
    pub root: PathBuf,
    pub mac_only: AtomicBool,
    pub index: RwLock<Arc<Index>>,
    pub payload: RwLock<Arc<Payload>>,
    /// a new payload's etag after each change in the folder
    pub events: broadcast::Sender<String>,
    pub pdf_text: Option<PdfText>,
    pub rescan_lock: Mutex<()>,
    pub git: crate::git::Sync,
    pub ingredients: crate::ingredient::Cache,
    /// set when the cookbook is removed or moved: its sync loop stops
    pub closed: AtomicBool,
    /// the file watcher; dropping it ends the rescan loop
    watcher: Mutex<Option<Box<dyn std::any::Any + Send>>>,
}

impl Book {
    /// Rescan the folder (unchanged files are reused) and notify listeners.
    pub fn rescan(&self) {
        let _g = self.rescan_lock.lock().unwrap();
        let prev = self.index.read().unwrap().clone();
        let idx = vault::scan(&self.root, Some(&prev), self.pdf_text);
        // README.md, the contents (its write wakes the watcher once; the next pass finds nothing to change)
        crate::toc::update(&self.root, &idx);
        let files: Vec<&vault::Entry> = idx.files.values().collect();
        // folder icons and colours (written by the app; plain YAML, see docs/FORMAT.md)
        let folder_meta = std::fs::read(self.root.join(FOLDER_META)).ok().filter(|b| b.len() <= 256 * 1024).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let body = json!({ "files": files, "folders": idx.folders, "folder_meta": folder_meta });
        let bytes = serde_json::to_vec(&body).unwrap();
        let etag = vault::etag_of(&bytes);
        let changed = etag != self.payload.read().unwrap().etag;
        *self.payload.write().unwrap() = Arc::new(Payload { etag: etag.clone(), json: bytes.into() });
        *self.index.write().unwrap() = Arc::new(idx);
        if changed {
            let _ = self.events.send(etag);
        }
    }

    pub fn abs(&self, p: &VaultPath, must_exist: bool) -> Result<PathBuf, Response> {
        vault::resolve(&self.root, p, must_exist).map_err(|e| err(StatusCode::NOT_FOUND, e))
    }
}

/// The cookbook a request is for: `?book=<id>` (the UI adds it to every request), or the first one
/// the device may see when there is none. An unknown id is a 404, never another cookbook: a tab
/// left open on a removed cookbook must not save into a different one.
pub struct Bk(pub Arc<Book>);

impl axum::extract::FromRequestParts<Arc<AppState>> for Bk {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut axum::http::request::Parts, s: &Arc<AppState>) -> Result<Self, Response> {
        let l = parts.extensions.get::<Listener>().copied().unwrap_or(Listener::Lan);
        s.requested(book_param(&parts.uri), l).map(Bk)
    }
}

/// The `book` query parameter (ids are hex, so nothing to decode).
pub fn book_param(uri: &axum::http::Uri) -> Option<&str> {
    uri.query()?.split('&').find_map(|kv| kv.strip_prefix("book="))
}

/// A cookbook's name: its folder's name.
pub fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

impl AppState {
    /// The open cookbooks a device on listener `l` may see: all of them on the Mac, not the Mac-only
    /// ones on the home network.
    pub fn visible(&self, l: Listener) -> Vec<Arc<Book>> {
        self.books.read().unwrap().iter().filter(|b| l == Listener::Loopback || !b.mac_only.load(Ordering::Relaxed)).cloned().collect()
    }

    pub fn book(&self, id: &str) -> Option<Arc<Book>> {
        self.books.read().unwrap().iter().find(|b| b.id == id).cloned()
    }

    /// See `Bk`.
    pub fn requested(&self, id: Option<&str>, l: Listener) -> Result<Arc<Book>, Response> {
        let v = self.visible(l);
        match id {
            Some(id) => v.into_iter().find(|b| b.id == id),
            None => v.into_iter().next(),
        }
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "that cookbook isn't open in the app"))
    }

    /// Open a cookbook: scan its folder, then watch and sync it. It joins `books` at the position its
    /// entry has in `settings.cookbooks`. Must run inside the async runtime.
    pub fn open(self: &Arc<Self>, c: &Cookbook) -> Result<Arc<Book>> {
        let root = c.path.canonicalize().with_context(|| format!("the folder {} isn't there", c.path.display()))?;
        if !root.is_dir() {
            anyhow::bail!("{} isn't a folder", root.display());
        }
        let book = Arc::new(Book {
            id: c.id.clone(),
            name: name_of(&root),
            root,
            mac_only: AtomicBool::new(c.mac_only),
            index: RwLock::new(Arc::new(Index::default())),
            payload: RwLock::new(Arc::new(Payload { etag: String::new(), json: Bytes::new() })),
            events: broadcast::channel(16).0,
            pdf_text: self.pdf_text,
            rescan_lock: Mutex::new(()),
            git: Default::default(),
            ingredients: Default::default(),
            closed: AtomicBool::new(false),
            watcher: Mutex::new(None),
        });
        book.rescan();
        spawn_watcher(self.clone(), book.clone())?;
        crate::git::spawn(self.clone(), book.clone());
        let order: Vec<String> = self.settings.read().unwrap().cookbooks.iter().map(|c| c.id.clone()).collect();
        let mut books = self.books.write().unwrap();
        books.retain(|b| b.id != c.id);
        books.push(book.clone());
        books.sort_by_key(|b| order.iter().position(|id| *id == b.id).unwrap_or(usize::MAX));
        Ok(book)
    }

    /// Stop serving a cookbook (removed, or moved to another folder). Its folder is left as it is.
    pub fn close(&self, id: &str) {
        let Some(b) = self.book(id) else { return };
        self.books.write().unwrap().retain(|x| x.id != id);
        b.closed.store(true, Ordering::Relaxed);
        b.watcher.lock().unwrap().take();
        b.git.wake();
    }

    /// Why `path` can't be a cookbook, or its full path. `except`: the cookbook being moved there.
    pub fn check_folder(&self, path: &Path, except: Option<&str>) -> Result<PathBuf, String> {
        let p = path.canonicalize().map_err(|_| "that folder isn't there".to_string())?;
        if !p.is_dir() {
            return Err("choose a folder, not a file".into());
        }
        let nested = |a: &Path, b: &Path| a.starts_with(b) || b.starts_with(a);
        if self.data_dir.canonicalize().is_ok_and(|d| nested(&p, &d)) {
            return Err("that folder holds the app's own settings".into());
        }
        let name = name_of(&p).to_lowercase();
        for c in self.settings.read().unwrap().cookbooks.iter().filter(|c| Some(c.id.as_str()) != except) {
            let other = c.path.canonicalize().unwrap_or_else(|_| c.path.clone());
            if other == p {
                return Err("that folder is already a cookbook".into());
            }
            if nested(&p, &other) {
                return Err(format!("that folder is inside the cookbook {} or holds it; choose one outside it", name_of(&other)));
            }
            if name_of(&other).to_lowercase() == name {
                return Err(format!("there is already a cookbook called {}; rename the folder first", name_of(&other)));
            }
        }
        Ok(p)
    }

    pub fn host_allowed(&self, host: &str, l: Listener) -> bool {
        match l {
            Listener::Loopback => {
                let port = self.loopback_port.load(Ordering::Relaxed);
                host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
            }
            Listener::Lan | Listener::LanHttp => {
                let h = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map_or(host, |(h, _)| h);
                self.lan_hosts.read().unwrap().iter().any(|x| x == h)
            }
        }
    }

    pub fn save_settings(&self) -> Result<()> {
        let s = self.settings.read().unwrap().clone();
        write_private(&self.data_dir.join("settings.json"), serde_json::to_vec_pretty(&s)?.as_slice())
    }

    /// Change one cookbook's settings entry (None when there is no such cookbook).
    pub fn cookbook_mut<R>(&self, id: &str, f: impl FnOnce(&mut Cookbook) -> R) -> Option<R> {
        self.settings.write().unwrap().cookbooks.iter_mut().find(|c| c.id == id).map(f)
    }
}

pub fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path.parent().unwrap())?;
    vault::atomic_write(path, bytes)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

pub fn err(code: StatusCode, e: impl std::fmt::Display) -> Response {
    (code, Json(json!({ "error": e.to_string() }))).into_response()
}

fn bad(e: impl std::fmt::Display) -> Response {
    err(StatusCode::BAD_REQUEST, e)
}

type Res = Result<Response, Response>;
type St = State<Arc<AppState>>;

pub struct Config {
    pub vault: PathBuf,
    pub data_dir: PathBuf,
    pub shell: Option<Arc<dyn Shell>>,
    pub pdf_text: Option<PdfText>,
}

/// Build state: load settings, scan the recipes folder, start the watcher.
pub fn init(cfg: Config) -> Result<Arc<AppState>> {
    // Choose the TLS backend explicitly: with two compiled in (the desktop app's updater brings `ring`),
    // rustls refuses to pick one and the sharing server panics, taking the app's own window down with it.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    std::fs::create_dir_all(&cfg.data_dir)?;
    let mut settings: Settings = match std::fs::read(cfg.data_dir.join("settings.json")) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => Settings { setup_done: false, ..Default::default() }, // first start: show the setup
    };
    let key_path = cfg.data_dir.join("cookie.key");
    let cookie_key = match std::fs::read(&key_path) {
        Ok(k) if k.len() >= 32 => k,
        _ => {
            let k: [u8; 32] = rand::random();
            write_private(&key_path, &k)?;
            k.to_vec()
        }
    };
    // first start, or a settings file from before cookbooks: the one recipes folder, made if needed
    if settings.cookbooks.is_empty() {
        let path = settings.vault.take().unwrap_or(cfg.vault);
        std::fs::create_dir_all(&path).with_context(|| format!("creating the recipes folder {}", path.display()))?;
        let git_sync = std::mem::take(&mut settings.git_sync);
        settings.cookbooks.push(Cookbook { id: FIRST_BOOK.into(), path: path.canonicalize()?, git_sync, mac_only: false });
    }
    let cookbooks = settings.cookbooks.clone();
    let (tx, _) = broadcast::channel(64);
    let state = Arc::new(AppState {
        data_dir: cfg.data_dir,
        books: RwLock::new(vec![]),
        events: tx,
        settings: RwLock::new(settings),
        launch_token: auth::random_token(),
        cookie_key,
        limiter: Default::default(),
        lan_hosts: RwLock::new(vec!["recipes.local".into()]),
        loopback_port: AtomicU16::new(0),
        shell: cfg.shell,
        pdf_text: cfg.pdf_text,
        ext: Default::default(),
    });
    for c in &cookbooks {
        // a folder that's gone (an unplugged drive) is listed as missing and never made again
        if let Err(e) = state.open(c) {
            tracing::warn!("cookbook {}: {e:#}", c.path.display());
        }
    }
    Ok(state)
}

/// Rescan a cookbook after changes in its folder, until it is closed (the watcher is dropped).
fn spawn_watcher(state: Arc<AppState>, book: Arc<Book>) -> Result<()> {
    use notify::RecursiveMode;
    use notify_debouncer_full::{DebounceEventResult, new_debouncer};
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let root = book.root.clone();
    let mut deb = new_debouncer(std::time::Duration::from_millis(300), None, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            let relevant = events.iter().flat_map(|e| e.paths.iter()).any(|p| {
                p.strip_prefix(&root).is_ok_and(|r| r == Path::new(FOLDER_META) || !r.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')))
            });
            if relevant {
                let _ = tx.send(());
            }
        }
    })?;
    deb.watch(&book.root, RecursiveMode::Recursive)?;
    *book.watcher.lock().unwrap() = Some(Box::new(deb));
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            while rx.try_recv().is_ok() {}
            let b = book.clone();
            let _ = tokio::task::spawn_blocking(move || b.rescan()).await;
            book.git.wake();
            crate::nutrition::changed(&state);
        }
    });
    Ok(())
}

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/ping", get(|| async { "ok" }))
        .route("/api/session", get(session).post(login).delete(logout))
        .route("/api/vault", get(vault_payload))
        .route("/api/events", get(events))
        .route("/api/files/{*path}", get(get_file).put(put_file).delete(delete_file))
        .route("/api/fs/move", post(move_path))
        .route("/api/fs/mkdir", post(mkdir))
        .route("/api/folders", put(folder_meta_put))
        .route("/api/trash", get(trash_list).delete(trash_empty))
        .route("/api/trash/restore", post(trash_restore))
        .route("/api/settings", get(settings_get).put(settings_put))
        .route("/api/settings/pin", put(settings_pin))
        .route("/api/cookbooks", post(cookbook_add))
        .route("/api/cookbooks/{id}", put(cookbook_put).delete(cookbook_remove))
        .route("/api/shell/{action}", post(shell_action))
        .merge(crate::extra_routes())
        .route("/auth", get(admin_auth))
        .fallback(static_ui)
        .layer(DefaultBodyLimit::max(30 * 1024 * 1024));
    api.layer(middleware::from_fn_with_state(state.clone(), auth::guard))
        .layer(tower_http::compression::CompressionLayer::new())
        .with_state(state)
}

/// Serve `router` on a TCP listener, tagging requests with which listener they came from.
pub async fn serve(app: Router, listener: tokio::net::TcpListener, kind: Listener) -> Result<()> {
    let app = app.layer(Extension(kind));
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

// ---------- session ----------

async fn session(State(s): St, Caller(role): Caller, Extension(Who(who)): Extension<Who>, Extension(l): Extension<Listener>) -> Response {
    let books: Vec<_> = s.visible(l).iter().map(|b| json!({ "id": b.id, "name": b.name })).collect();
    let st = s.settings.read().unwrap();
    Json(json!({
        "books": books,
        "role": role,
        "user": who,
        "pin_set": st.pin_hash.is_some(),
        "ai": st.ai.enabled,
        "calendar": st.calendar.enabled && !st.calendar.calendars.is_empty(),
        "shell": s.shell.is_some() && role == Role::Admin,
        "version": env!("CARGO_PKG_VERSION"),
        "setup": role == Role::Admin && !st.setup_done,
        "accent": st.accent,
        "group": st.group,
    }))
    .into_response()
}

#[derive(Deserialize)]
struct PinBody {
    pin: String,
}

#[derive(Deserialize)]
struct SignInBody {
    name: String,
    /// needed when the name is new
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    pin: String,
}

/// Sign this device in as a person (added if new), with the family PIN when one is set.
async fn login(State(s): St, Extension(l): Extension<Listener>, ConnectInfo(addr): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(b): Json<SignInBody>) -> Res {
    let ip = addr.ip();
    crate::users::clean_name(&b.name)?;
    let (hash, pin_gen) = {
        let st = s.settings.read().unwrap();
        (st.pin_hash.clone(), st.pin_gen)
    };
    if let Some(hash) = hash {
        if b.pin.is_empty() {
            return Err(err(StatusCode::UNAUTHORIZED, "enter the PIN"));
        }
        if let Err(wait) = s.limiter.check(ip) {
            return Err(err(StatusCode::TOO_MANY_REQUESTS, format!("too many attempts, wait {wait} s")));
        }
        let pin = b.pin.clone();
        let ok = tokio::task::spawn_blocking(move || auth::verify_pin(&pin, &hash)).await.unwrap_or(false);
        if !ok {
            s.limiter.fail(ip);
            return Err(err(StatusCode::UNAUTHORIZED, "wrong PIN"));
        }
        s.limiter.success(ip);
    }
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("");
    let user = crate::users::sign_in(&s, &b.name, &b.full_name, ua, &ip.to_string())?;
    let v = auth::make_session(&s.cookie_key, pin_gen, &user.id);
    let age = auth::SESSION_DAYS * 86400;
    // `__Host-` cookies must be Secure, which browsers refuse over plain HTTP
    let cookie = if l == Listener::LanHttp {
        format!("{}={v}; Path=/; HttpOnly; SameSite=Strict; Max-Age={age}", auth::HTTP_SESSION_COOKIE)
    } else {
        format!("{}={v}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={age}", auth::SESSION_COOKIE)
    };
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "user": { "id": user.id, "name": user.name, "full_name": user.full_name } }))).into_response())
}

async fn logout(Extension(l): Extension<Listener>) -> Response {
    let c = if l == Listener::LanHttp {
        format!("{}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0", auth::HTTP_SESSION_COOKIE)
    } else {
        format!("{}=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0", auth::SESSION_COOKIE)
    };
    ([(header::SET_COOKIE, c)], StatusCode::NO_CONTENT).into_response()
}

#[derive(Deserialize)]
struct AuthQ {
    k: String,
}

/// Desktop window opens `/auth?k=<launch token>`; exchange it for an HttpOnly cookie.
async fn admin_auth(State(s): St, Extension(l): Extension<Listener>, Query(q): Query<AuthQ>) -> Res {
    if l != Listener::Loopback || !auth::constant_eq(q.k.as_bytes(), s.launch_token.as_bytes()) {
        return Err(err(StatusCode::FORBIDDEN, "invalid link"));
    }
    let c = format!("{}={}; Path=/; HttpOnly; SameSite=Strict", auth::ADMIN_COOKIE, s.launch_token);
    Ok(([(header::SET_COOKIE, c)], Redirect::to("/")).into_response())
}

// ---------- vault ----------

async fn vault_payload(Bk(b): Bk, headers: HeaderMap) -> Response {
    let p = b.payload.read().unwrap().clone();
    let tag = format!("\"{}\"", p.etag);
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(tag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    (
        [(header::CONTENT_TYPE, "application/json"), (header::ETAG, tag.as_str()), (header::CACHE_CONTROL, "no-cache")],
        p.json.clone(),
    )
        .into_response()
}

/// Sent when the background check (lib.rs `spawn_update_checks`) has an update ready.
pub const UPDATE_EVENT: &str = "update";

/// The cookbook's new payload etags (event `vault`), and the names of what else changed for everyone
/// (the whiteboard, the meals made, people, an update).
async fn events(State(s): St, Bk(b): Bk) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    type Item = Result<Event, std::convert::Infallible>;
    let shared = BroadcastStream::new(s.events.subscribe()).filter_map(|m| m.ok()).map(|m| -> Item { Ok(Event::default().event(&m).data("1")) });
    let own = BroadcastStream::new(b.events.subscribe()).filter_map(|m| m.ok()).map(|m| -> Item { Ok(Event::default().event("vault").data(m)) });
    Sse::new(shared.merge(own)).keep_alive(KeepAlive::default())
}

fn vp(raw: &str) -> Result<VaultPath, Response> {
    VaultPath::parse(raw).map_err(bad)
}

fn content_headers(p: &VaultPath) -> Vec<(header::HeaderName, HeaderValue)> {
    let kind = vault::kind_of(p);
    let mime = match kind {
        vault::FileKind::Note => "text/markdown; charset=utf-8".to_string(),
        _ => mime_guess::from_path(p.name()).first_or_octet_stream().to_string(),
    };
    let mut h = vec![(header::CONTENT_TYPE, HeaderValue::from_str(&mime).unwrap())];
    if matches!(kind, vault::FileKind::Other) || p.ext() == "svg" {
        let name = p.name().replace(['"', '\\'], "_");
        let disp = format!("attachment; filename*=UTF-8''{}", urlencode(&name));
        h.push((header::CONTENT_DISPOSITION, HeaderValue::from_str(&disp).unwrap()));
        h.push((header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox")));
    }
    h
}

pub fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn get_file(Bk(b): Bk, AxPath(raw): AxPath<String>, headers: HeaderMap) -> Res {
    let p = vp(&raw)?;
    let abs = b.abs(&p, true)?;
    let bytes = tokio::fs::read(&abs).await.map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    let tag = format!("\"{}\"", vault::etag_of(&bytes));
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(tag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let mut res = Response::new(Body::from(bytes));
    for (k, v) in content_headers(&p) {
        res.headers_mut().insert(k, v);
    }
    res.headers_mut().insert(header::ETAG, HeaderValue::from_str(&tag).unwrap());
    res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(res)
}

#[derive(Deserialize, Default)]
struct PutQ {
    #[serde(default)]
    unique: bool,
}

async fn put_file(
    Bk(b): Bk,
    caller: Caller,
    AxPath(raw): AxPath<String>,
    Query(q): Query<PutQ>,
    headers: HeaderMap,
    body: Bytes,
) -> Res {
    caller.need(Role::Writer)?;
    let mut p = vp(&raw)?;
    if vault::kind_of(&p) == vault::FileKind::Note && body.len() > 2 * 1024 * 1024 {
        return Err(bad("notes are limited to 2 MB"));
    }
    if p.as_str().starts_with(&format!("{}/", vault::ATTACHMENTS)) {
        if let Some(parent) = p.parent() {
            let dir = b.root.join(parent);
            if !dir.starts_with(b.root.join(vault::ATTACHMENTS)) {
                return Err(bad("bad attachment path"));
            }
            std::fs::create_dir_all(&dir).map_err(bad)?;
        }
    }
    if q.unique {
        p = vault::unique(&b.root, &p);
    }
    let abs = b.abs(&p, false)?;
    let if_match = headers.get(header::IF_MATCH).and_then(|v| v.to_str().ok()).map(|v| v.trim_matches('"').to_string());
    let create_only = headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some("*");
    let exists = abs.exists();
    let current = if exists { std::fs::read(&abs).ok().map(|b| vault::etag_of(&b)) } else { None };
    if create_only && exists {
        return Err(conflict(current, "a file with that name already exists"));
    }
    if !create_only {
        match (&if_match, &current) {
            (Some(want), Some(have)) if want == have => {}
            (Some(_), _) => return Err(conflict(current, "the file changed since you opened it")),
            (None, _) => return Err(err(StatusCode::PRECONDITION_REQUIRED, "If-Match or If-None-Match required")),
        }
    }
    let b2 = b.clone();
    let p2 = p.clone();
    let etag = tokio::task::spawn_blocking(move || -> Result<String> {
        if exists {
            vault::backup_before_overwrite(&b2.root, &p2, &abs)?;
        }
        vault::atomic_write(&abs, &body)?;
        b2.rescan();
        Ok(vault::etag_of(&body))
    })
    .await
    .unwrap()
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "path": p, "etag": etag })).into_response())
}

fn conflict(current: Option<String>, msg: &str) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "error": msg, "etag": current }))).into_response()
}

async fn delete_file(Bk(b): Bk, caller: Caller, AxPath(raw): AxPath<String>) -> Res {
    caller.need(Role::Writer)?;
    let p = vp(&raw)?;
    let id = vault::trash(&b.root, &p).map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    blocking_rescan(&b).await;
    Ok(Json(json!({ "trash_id": id })).into_response())
}

async fn blocking_rescan(b: &Arc<Book>) {
    let b = b.clone();
    let _ = tokio::task::spawn_blocking(move || b.rescan()).await;
}

#[derive(Deserialize)]
struct MoveBody {
    from: String,
    to: String,
}

async fn move_path(State(s): St, Bk(book): Bk, caller: Caller, Json(b): Json<MoveBody>) -> Res {
    caller.need(Role::Writer)?;
    let (from, to) = (vp(&b.from)?, vp(&b.to)?);
    let src = book.abs(&from, true)?;
    let dst = book.abs(&to, false)?;
    if src.is_dir() && to.as_str().starts_with(&format!("{}/", from.as_str())) {
        return Err(bad("cannot move a folder into itself"));
    }
    let case_only = from.as_str().to_lowercase() == to.as_str().to_lowercase();
    if !case_only && vault::unique(&book.root, &to) != to {
        return Err(conflict(None, "something with that name already exists"));
    }
    std::fs::rename(&src, &dst).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::users::moved(&s, &book.id, from.as_str(), to.as_str());
    blocking_rescan(&book).await;
    Ok(Json(json!({ "path": to })).into_response())
}

#[derive(Deserialize)]
struct PathBody {
    path: String,
}

async fn mkdir(Bk(book): Bk, caller: Caller, Json(b): Json<PathBody>) -> Res {
    caller.need(Role::Writer)?;
    let p = vp(&b.path)?;
    if vault::unique(&book.root, &p) != p {
        return Err(conflict(None, "something with that name already exists"));
    }
    let abs = book.abs(&p, false)?;
    std::fs::create_dir(&abs).map_err(bad)?;
    blocking_rescan(&book).await;
    Ok(Json(json!({ "path": p })).into_response())
}

pub const FOLDER_META: &str = ".folders.yaml";

/// Folder icons/colours: the client sends the whole file; it is stored as written.
async fn folder_meta_put(Bk(b): Bk, caller: Caller, body: String) -> Res {
    caller.need(Role::Writer)?;
    if body.len() > 256 * 1024 {
        return Err(err(StatusCode::PAYLOAD_TOO_LARGE, "too large"));
    }
    vault::atomic_write(&b.root.join(FOLDER_META), body.as_bytes()).map_err(bad)?;
    blocking_rescan(&b).await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn trash_list(Bk(b): Bk, caller: Caller) -> Res {
    caller.need(Role::Writer)?;
    Ok(Json(vault::list_trash(&b.root)).into_response())
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn trash_restore(Bk(book): Bk, caller: Caller, Json(b): Json<IdBody>) -> Res {
    caller.need(Role::Writer)?;
    let path = vault::restore(&book.root, &b.id).map_err(bad)?;
    blocking_rescan(&book).await;
    Ok(Json(json!({ "path": path })).into_response())
}

async fn trash_empty(Bk(b): Bk, caller: Caller) -> Res {
    caller.need(Role::Admin)?;
    let dir = b.root.join(vault::TRASH);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(bad)?;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- settings (desktop only) ----------

async fn settings_get(State(s): St, caller: Caller) -> Res {
    caller.need(Role::Admin)?;
    let st = s.settings.read().unwrap().clone();
    let lan = crate::lan_status(&s);
    let cookbooks: Vec<_> = st
        .cookbooks
        .iter()
        .map(|c| json!({ "id": c.id, "name": name_of(&c.path), "path": c.path, "missing": s.book(&c.id).is_none(), "mac_only": c.mac_only, "git": crate::git::status(&s, &c.id) }))
        .collect();
    Ok(Json(json!({
        "cookbooks": cookbooks,
        "pin_set": st.pin_hash.is_some(),
        "ai": st.ai,
        "ai_key_set": crate::ai_key_set(&s),
        "usda_key_set": crate::nutrition::key_set(),
        "nutrition_status": crate::nutrition::status(&s),
        "lan": st.lan,
        "keep_awake": st.keep_awake,
        "network": lan,
        "autostart": s.shell.as_ref().map(|sh| sh.autostart(None)),
        "editing_as": st.editing_as,
        "calendar": st.calendar,
        "updates": st.updates,
        "accent": st.accent,
        "group": st.group,
    }))
    .into_response())
}

#[derive(Deserialize)]
struct SettingsPut {
    ai: Option<AiSettings>,
    calendar: Option<CalendarSettings>,
    lan: Option<bool>,
    keep_awake: Option<bool>,
    setup_done: Option<bool>,
    /// a person's id, or "" for nobody
    editing_as: Option<String>,
    updates: Option<Updates>,
    /// `#rrggbb`, or "" for the built-in colour
    accent: Option<String>,
    group: Option<Group>,
}

async fn settings_put(State(s): St, caller: Caller, Json(b): Json<SettingsPut>) -> Res {
    caller.need(Role::Admin)?;
    let accent = b.accent.as_deref().map(clean_accent).transpose().map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    {
        let mut st = s.settings.write().unwrap();
        if let Some(a) = accent {
            st.accent = a;
        }
        if let Some(g) = b.group {
            st.group = clean_group(g);
        }
        if let Some(ai) = b.ai {
            st.ai = ai;
        }
        if let Some(c) = b.calendar {
            st.calendar = c;
        }
        if let Some(l) = b.lan {
            st.lan = l;
        }
        if let Some(k) = b.keep_awake {
            st.keep_awake = k;
        }
        if let Some(u) = b.updates {
            st.updates = u;
        }
        if let Some(d) = b.setup_done {
            st.setup_done = d;
        }
        if let Some(id) = b.editing_as {
            st.editing_as = (!id.is_empty()).then_some(id);
        }
    }
    s.save_settings().map_err(bad)?;
    crate::settings_changed(&s);
    settings_get(State(s), caller).await
}

async fn settings_pin(State(s): St, caller: Caller, Json(b): Json<PinBody>) -> Res {
    caller.need(Role::Admin)?;
    let pin = b.pin.trim().to_string();
    {
        let mut st = s.settings.write().unwrap();
        if pin.is_empty() {
            st.pin_hash = None;
        } else {
            // ponytail: any length; PinLimiter is what slows guessing
            st.pin_hash = Some(auth::hash_pin(&pin));
        }
        st.pin_gen += 1; // signs every device out
    }
    s.save_settings().map_err(bad)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- cookbooks (desktop only) ----------

#[derive(Deserialize)]
struct CookbookBody {
    path: PathBuf,
}

/// Open a folder as a new cookbook (an empty one, made in the folder picker, starts empty).
async fn cookbook_add(State(s): St, caller: Caller, Json(b): Json<CookbookBody>) -> Res {
    caller.need(Role::Admin)?;
    let path = s.check_folder(&b.path, None).map_err(bad)?;
    let c = Cookbook { id: format!("{:08x}", rand::random::<u32>()), path, git_sync: false, mac_only: false };
    s.settings.write().unwrap().cookbooks.push(c.clone());
    let (s2, c2) = (s.clone(), c.clone());
    if let Err(e) = tokio::task::spawn_blocking(move || s2.open(&c2)).await.unwrap() {
        s.settings.write().unwrap().cookbooks.retain(|x| x.id != c.id);
        return Err(bad(format!("{e:#}")));
    }
    s.save_settings().map_err(bad)?;
    Ok(Json(json!({ "id": c.id })).into_response())
}

#[derive(Deserialize)]
struct CookbookPut {
    mac_only: Option<bool>,
    /// the folder moved, or was found again: serve this one instead (sync is turned off)
    path: Option<PathBuf>,
}

async fn cookbook_put(State(s): St, caller: Caller, AxPath(id): AxPath<String>, Json(b): Json<CookbookPut>) -> Res {
    caller.need(Role::Admin)?;
    let old = s.cookbook_mut(&id, |c| c.clone()).ok_or_else(|| err(StatusCode::NOT_FOUND, "no such cookbook"))?;
    if let Some(m) = b.mac_only {
        s.cookbook_mut(&id, |c| c.mac_only = m);
        if let Some(book) = s.book(&id) {
            book.mac_only.store(m, Ordering::Relaxed);
        }
    }
    if let Some(p) = b.path {
        let path = s.check_folder(&p, Some(&id)).map_err(bad)?;
        s.close(&id);
        // a different folder may be a different repository: never push it without being asked
        let c = s.cookbook_mut(&id, |c| {
            c.path = path;
            c.git_sync = false;
            c.clone()
        });
        let s2 = s.clone();
        let opened = tokio::task::spawn_blocking(move || c.map(|c| s2.open(&c))).await.unwrap();
        if let Some(Err(e)) = opened {
            s.cookbook_mut(&id, |c| *c = old.clone());
            let s2 = s.clone();
            let _ = tokio::task::spawn_blocking(move || s2.open(&old)).await;
            return Err(bad(format!("{e:#}")));
        }
    }
    s.save_settings().map_err(bad)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Stop serving a cookbook; its folder, and its git history, stay where they are.
async fn cookbook_remove(State(s): St, caller: Caller, AxPath(id): AxPath<String>) -> Res {
    caller.need(Role::Admin)?;
    {
        let st = s.settings.read().unwrap();
        if !st.cookbooks.iter().any(|c| c.id == id) {
            return Err(err(StatusCode::NOT_FOUND, "no such cookbook"));
        }
        if st.cookbooks.len() == 1 {
            return Err(bad("the app needs at least one cookbook"));
        }
    }
    s.close(&id);
    s.settings.write().unwrap().cookbooks.retain(|c| c.id != id);
    s.save_settings().map_err(bad)?;
    let _ = tokio::task::spawn_blocking(move || crate::git::forget_token(&id)).await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// A file name for the recipe book: the cookbook's name, without anything a file name can't hold.
fn file_name(raw: Option<&str>, ext: &str) -> String {
    let n: String = raw.unwrap_or("").chars().filter(|c| !matches!(c, '/' | '\\' | ':') && !c.is_control()).take(100).collect();
    let n = n.trim().trim_start_matches('.').trim();
    format!("{}.{ext}", if n.is_empty() { "Recipes" } else { n })
}

async fn shell_action(State(s): St, caller: Caller, Extension(l): Extension<Listener>, uri: axum::http::Uri, AxPath(action): AxPath<String>, body: Bytes) -> Res {
    caller.need(Role::Admin)?;
    let Some(shell) = s.shell.clone() else { return Err(err(StatusCode::NOT_IMPLEMENTED, "not running in the desktop app")) };
    let json = serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default();
    match action.as_str() {
        "reveal" => {
            let b: PathBody = serde_json::from_slice(&body).map_err(bad)?;
            let book = s.requested(book_param(&uri), l)?;
            let abs = if b.path.is_empty() { book.root.clone() } else { book.abs(&vp(&b.path)?, true)? };
            shell.reveal(&abs);
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        "choose-vault" => {
            let picked = tokio::task::spawn_blocking(move || shell.choose_folder()).await.unwrap();
            Ok(Json(json!({ "path": picked })).into_response())
        }
        "print" => {
            shell.print();
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        "open-url" => {
            // a link to a web page, opened in the browser instead of inside the app window
            let url = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|v| v["url"].as_str().map(str::to_string)).unwrap_or_default();
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(err(StatusCode::BAD_REQUEST, "only web addresses can be opened"));
            }
            shell.open_url(&url);
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        "restart" => {
            shell.restart();
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        "autostart" => {
            let set = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|v| v["enabled"].as_bool());
            let on = tokio::task::spawn_blocking(move || shell.autostart(set)).await.unwrap();
            Ok(Json(json!({ "enabled": on })).into_response())
        }
        "update-check" => {
            let mode = s.settings.read().unwrap().updates;
            let found = tokio::task::spawn_blocking(move || shell.update_check(mode)).await.unwrap().map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
            let (version, notes, ready) = match found {
                Some((v, n, r)) => (Some(v), Some(n), r),
                None => (None, None, false),
            };
            Ok(Json(json!({ "version": version, "notes": notes, "ready": ready, "mode": mode })).into_response())
        }
        "update-install" => {
            let quiet = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|v| v["quiet"].as_bool()).unwrap_or(false);
            let restarting = tokio::task::spawn_blocking(move || shell.update_install(quiet)).await.unwrap().map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
            Ok(Json(json!({ "restarting": restarting })).into_response())
        }
        // Settings → Recipe book: the whole book saved to Downloads and shown in Finder
        "save-html" | "save-pdf" => {
            let dir = shell.downloads().ok_or_else(|| err(StatusCode::INTERNAL_SERVER_ERROR, "the Downloads folder wasn't found"))?;
            let fail = |e: String| err(StatusCode::INTERNAL_SERVER_ERROR, format!("the recipe book could not be saved: {e}"));
            let name = json["name"].as_str();
            let to = if action == "save-html" {
                let html = json["html"].as_str().ok_or_else(|| bad("no book to save"))?;
                let to = unique_file(&dir, &file_name(name, "html"));
                std::fs::write(&to, html).map_err(|e| fail(e.to_string()))?;
                to
            } else {
                let to = unique_file(&dir, &file_name(name, "pdf"));
                let (sh, t) = (shell.clone(), to.clone());
                tokio::task::spawn_blocking(move || sh.save_pdf(&t)).await.unwrap().map_err(fail)?;
                to
            };
            shell.reveal(&to);
            Ok(Json(json!({ "name": to.file_name().map(|n| n.to_string_lossy().into_owned()) })).into_response())
        }
        _ => Err(err(StatusCode::NOT_FOUND, "unknown action")),
    }
}

// ---------- UI ----------

const CSP: &str = "default-src 'self'; img-src 'self' data: blob: https:; style-src 'self' 'unsafe-inline'; \
script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; connect-src 'self'; object-src 'none'; \
base-uri 'none'; frame-src 'self' blob:; frame-ancestors 'self'";

async fn static_ui(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") {
        return err(StatusCode::NOT_FOUND, "no such endpoint");
    }
    let (file, data) = match Ui::get(path).filter(|_| !path.is_empty()) {
        Some(d) => (path, d),
        None => match Ui::get("index.html") {
            Some(d) => ("index.html", d),
            None => return (StatusCode::NOT_FOUND, "UI not built (run npm run build in ui/)").into_response(),
        },
    };
    let mime = mime_guess::from_path(file).first_or_octet_stream();
    let cache = if file.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    let mut res = ([(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, cache)], data.data).into_response();
    if file.ends_with(".html") {
        res.headers_mut().insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let t = std::env::temp_dir().join(format!("recipes-books-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        t
    }

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    /// A state with two cookbooks, `1` (folder `Family`) and `b2` (folder `Mine`), both holding
    /// `Pies/Pie.md`; `b2` is Mac only.
    async fn two(t: &Path) -> Arc<AppState> {
        write(&t.join("Family/Pies/Pie.md"), "# Family pie\n");
        write(&t.join("Mine/Pies/Pie.md"), "# My pie\n");
        let s = init(Config { vault: t.join("Family"), data_dir: t.join("data"), shell: None, pdf_text: None }).unwrap();
        let c = Cookbook { id: "b2".into(), path: t.join("Mine"), git_sync: false, mac_only: true };
        s.settings.write().unwrap().cookbooks.push(c.clone());
        s.open(&c).unwrap();
        s
    }

    fn title(b: &Book) -> String {
        b.index.read().unwrap().files.get(&VaultPath::parse("Pies/Pie.md").unwrap()).unwrap().title.clone()
    }

    #[test]
    fn accent_and_group_settings() {
        assert_eq!(clean_accent(" #2E7D5B "), Ok(Some("#2e7d5b".into())));
        assert_eq!(clean_accent(""), Ok(None));
        for bad in ["2e7d5b", "#2e7d5", "#2e7d5bff", "#ggg000", "red", "#2e7d5b;}body{x"] {
            assert!(clean_accent(bad).is_err(), "{bad}");
        }
        let g = clean_group(Group { kind: GroupKind::Team, name: "  Test   Group ".into() });
        assert_eq!(g.name, "Test Group");
        assert_eq!(clean_group(Group { kind: GroupKind::Other, name: "x".repeat(99) }).name.len(), 60);
        // an older settings file: the built-in colour, a family with no name
        let st: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!((st.accent, st.group), (None, Group::default()));
        assert_eq!(serde_json::to_value(Group::default()).unwrap(), json!({ "kind": "family", "name": "" }));
    }

    #[tokio::test]
    async fn each_request_reaches_its_own_cookbook() {
        let t = tmp("reach");
        let s = two(&t).await;
        let uri = |q: &str| q.parse::<axum::http::Uri>().unwrap();
        assert_eq!(book_param(&uri("/api/vault?book=b2&x=1")), Some("b2"));
        assert_eq!(book_param(&uri("/api/files/Pie.md?unique=true")), None);

        // the same path is a different recipe in each cookbook
        assert_eq!(title(&s.requested(None, Listener::Loopback).unwrap()), "Family pie");
        assert_eq!(title(&s.requested(Some("b2"), Listener::Loopback).unwrap()), "My pie");
        // an unknown cookbook is refused, never answered from another one
        assert_eq!(s.requested(Some("gone"), Listener::Loopback).err().map(|r| r.status()), Some(StatusCode::NOT_FOUND));
        // a Mac-only cookbook is the desktop window's alone
        for l in [Listener::Lan, Listener::LanHttp] {
            assert!(s.requested(Some("b2"), l).is_err());
            assert_eq!(s.visible(l).len(), 1);
        }

        // a change in one cookbook is heard only by those reading it
        let (b1, b2) = (s.book(FIRST_BOOK).unwrap(), s.book("b2").unwrap());
        let (mut rx1, mut rx2) = (b1.events.subscribe(), b2.events.subscribe());
        write(&t.join("Mine/Soup.md"), "# Soup\n");
        b2.rescan();
        assert!(rx2.try_recv().is_ok());
        assert!(rx1.try_recv().is_err());

        // closed: gone from every device, its folder left alone
        s.close("b2");
        assert!(s.requested(Some("b2"), Listener::Loopback).is_err() && b2.closed.load(Ordering::Relaxed));
        assert!(t.join("Mine/Soup.md").exists());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[tokio::test]
    async fn folders_that_cant_be_cookbooks() {
        let t = tmp("check");
        let s = two(&t).await;
        for (p, why) in [
            (t.join("Family"), "already a cookbook"),
            (t.join("Family/Pies"), "inside"),
            (t.clone(), "settings"), // it holds both cookbooks too; the settings rule comes first
            (t.join("data"), "settings"),
            (t.join("nowhere"), "isn't there"),
            (t.join("Family/Pies/Pie.md"), "not a file"),
        ] {
            assert!(s.check_folder(&p, None).unwrap_err().contains(why), "{}", p.display());
        }
        std::fs::create_dir_all(t.join("elsewhere/mine")).unwrap();
        assert!(s.check_folder(&t.join("elsewhere/mine"), None).unwrap_err().contains("already a cookbook called Mine"), "names match in any letter case");
        std::fs::create_dir_all(t.join("Baking")).unwrap();
        assert!(s.check_folder(&t.join("Baking"), None).is_ok());
        // a cookbook may move to a folder of the same name
        std::fs::create_dir_all(t.join("moved/Mine")).unwrap();
        assert!(s.check_folder(&t.join("moved/Mine"), Some("b2")).is_ok());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[tokio::test]
    async fn moving_a_cookbook_turns_its_sync_off() {
        let t = tmp("move");
        let s = two(&t).await;
        s.cookbook_mut("b2", |c| c.git_sync = true);
        write(&t.join("moved/Mine/Pies/Pie.md"), "# My moved pie\n");
        let body = CookbookPut { mac_only: Some(false), path: Some(t.join("moved/Mine")) };
        cookbook_put(State(s.clone()), Caller(Role::Admin), AxPath("b2".into()), Json(body)).await.unwrap();
        let c = s.cookbook_mut("b2", |c| c.clone()).unwrap();
        assert!(!c.git_sync && !c.mac_only, "never pushes a folder it wasn't connected in");
        assert_eq!(title(&s.requested(Some("b2"), Listener::Lan).unwrap()), "My moved pie");
        assert_eq!(s.books.read().unwrap().iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), [FIRST_BOOK, "b2"], "keeps its place");

        // saved as cookbooks; the settings from before cookbooks are not written again
        let saved = std::fs::read_to_string(t.join("data/settings.json")).unwrap();
        assert!(saved.contains("\"cookbooks\"") && !saved.contains("\"vault\"") && !saved.contains("\"git_sync\": false,\n  \"editing_as\""));
        // the last cookbook can't be removed
        cookbook_remove(State(s.clone()), Caller(Role::Admin), AxPath("b2".into())).await.unwrap();
        assert!(cookbook_remove(State(s.clone()), Caller(Role::Admin), AxPath(FIRST_BOOK.into())).await.is_err());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn settings_from_before_cookbooks() {
        let st: Settings = serde_json::from_str(r#"{"vault":"/Users/x/Recipes","git_sync":true}"#).unwrap();
        assert_eq!((st.vault.as_deref(), st.git_sync, st.cookbooks.len()), (Some(Path::new("/Users/x/Recipes")), true, 0));
        assert_eq!(file_name(Some("Mine/../.x"), "pdf"), "Mine...x.pdf");
        assert_eq!(file_name(Some("  "), "html"), "Recipes.html");
    }
}
