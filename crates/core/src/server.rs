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
use std::sync::atomic::{AtomicU16, Ordering};
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    #[serde(default)]
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
    /// Sync the recipes folder with its GitHub repository (see git.rs).
    #[serde(default)]
    pub git_sync: bool,
    /// The person the desktop window edits as (users.rs); None = the built-in "Admin".
    #[serde(default)]
    pub editing_as: Option<String>,
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
    /// Quit and start again (after the recipes folder was changed).
    fn restart(&self) {}
    /// Look for a newer release: Some((version, release notes)), or None when up to date.
    fn update_check(&self) -> Result<Option<(String, String)>, String> {
        Err("updates come with the desktop app".into())
    }
    /// Download and install the newer release, then restart the app.
    fn update_install(&self) -> Result<(), String> {
        Err("updates come with the desktop app".into())
    }
}

pub struct Payload {
    pub etag: String,
    pub json: Bytes,
}

pub struct AppState {
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub index: RwLock<Arc<Index>>,
    pub payload: RwLock<Arc<Payload>>,
    pub events: broadcast::Sender<String>,
    pub settings: RwLock<Settings>,
    pub launch_token: String,
    pub cookie_key: Vec<u8>,
    pub limiter: auth::PinLimiter,
    pub lan_hosts: RwLock<Vec<String>>,
    pub loopback_port: AtomicU16,
    pub shell: Option<Arc<dyn Shell>>,
    pub pdf_text: Option<PdfText>,
    pub rescan_lock: Mutex<()>,
    pub ext: crate::Extensions,
}

impl AppState {
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

    /// Rescan the vault (unchanged files are reused) and notify listeners.
    pub fn rescan(&self) {
        let _g = self.rescan_lock.lock().unwrap();
        let prev = self.index.read().unwrap().clone();
        let idx = vault::scan(&self.root, Some(&prev), self.pdf_text);
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
    let settings: Settings = match std::fs::read(cfg.data_dir.join("settings.json")) {
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
    let root = settings.vault.clone().unwrap_or(cfg.vault);
    std::fs::create_dir_all(&root).with_context(|| format!("creating vault {}", root.display()))?;
    let root = root.canonicalize()?;
    let (tx, _) = broadcast::channel(64);
    let state = Arc::new(AppState {
        root,
        data_dir: cfg.data_dir,
        index: RwLock::new(Arc::new(Index::default())),
        payload: RwLock::new(Arc::new(Payload { etag: String::new(), json: Bytes::new() })),
        events: tx,
        settings: RwLock::new(settings),
        launch_token: auth::random_token(),
        cookie_key,
        limiter: Default::default(),
        lan_hosts: RwLock::new(vec!["recipes.local".into()]),
        loopback_port: AtomicU16::new(0),
        shell: cfg.shell,
        pdf_text: cfg.pdf_text,
        rescan_lock: Mutex::new(()),
        ext: Default::default(),
    });
    state.rescan();
    spawn_watcher(state.clone())?;
    Ok(state)
}

fn spawn_watcher(state: Arc<AppState>) -> Result<()> {
    use notify::RecursiveMode;
    use notify_debouncer_full::{DebounceEventResult, new_debouncer};
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let root = state.root.clone();
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
    deb.watch(&state.root, RecursiveMode::Recursive)?;
    tokio::spawn(async move {
        let _keep = deb;
        while rx.recv().await.is_some() {
            while rx.try_recv().is_ok() {}
            let s = state.clone();
            let _ = tokio::task::spawn_blocking(move || s.rescan()).await;
            crate::git::changed(&state);
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

async fn session(State(s): St, Caller(role): Caller, Extension(Who(who)): Extension<Who>) -> Response {
    let st = s.settings.read().unwrap();
    Json(json!({
        "role": role,
        "user": who,
        "pin_set": st.pin_hash.is_some(),
        "ai": st.ai.enabled,
        "shell": s.shell.is_some() && role == Role::Admin,
        "version": env!("CARGO_PKG_VERSION"),
        "setup": role == Role::Admin && !st.setup_done,
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
            return Err(err(StatusCode::UNAUTHORIZED, "enter the family PIN"));
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

async fn vault_payload(State(s): St, headers: HeaderMap) -> Response {
    let p = s.payload.read().unwrap().clone();
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

async fn events(State(s): St) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = BroadcastStream::new(s.events.subscribe());
    // a new vault payload's etag, or the name of what else changed (the whiteboard, the meals made)
    let stream = rx.filter_map(|m| m.ok()).map(|m| {
        Ok(if m == crate::board::EVENT || m == crate::made::EVENT { Event::default().event(&m).data("1") } else { Event::default().event("vault").data(m) })
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
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

async fn get_file(State(s): St, AxPath(raw): AxPath<String>, headers: HeaderMap) -> Res {
    let p = vp(&raw)?;
    let abs = s.abs(&p, true)?;
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
    State(s): St,
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
            let dir = s.root.join(parent);
            if !dir.starts_with(s.root.join(vault::ATTACHMENTS)) {
                return Err(bad("bad attachment path"));
            }
            std::fs::create_dir_all(&dir).map_err(bad)?;
        }
    }
    if q.unique {
        p = vault::unique(&s.root, &p);
    }
    let abs = s.abs(&p, false)?;
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
    let s2 = s.clone();
    let p2 = p.clone();
    let etag = tokio::task::spawn_blocking(move || -> Result<String> {
        if exists {
            vault::backup_before_overwrite(&s2.root, &p2, &abs)?;
        }
        vault::atomic_write(&abs, &body)?;
        s2.rescan();
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

async fn delete_file(State(s): St, caller: Caller, AxPath(raw): AxPath<String>) -> Res {
    caller.need(Role::Writer)?;
    let p = vp(&raw)?;
    let id = vault::trash(&s.root, &p).map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    blocking_rescan(&s).await;
    Ok(Json(json!({ "trash_id": id })).into_response())
}

async fn blocking_rescan(s: &Arc<AppState>) {
    let s = s.clone();
    let _ = tokio::task::spawn_blocking(move || s.rescan()).await;
}

#[derive(Deserialize)]
struct MoveBody {
    from: String,
    to: String,
}

async fn move_path(State(s): St, caller: Caller, Json(b): Json<MoveBody>) -> Res {
    caller.need(Role::Writer)?;
    let (from, to) = (vp(&b.from)?, vp(&b.to)?);
    let src = s.abs(&from, true)?;
    let dst = s.abs(&to, false)?;
    if src.is_dir() && to.as_str().starts_with(&format!("{}/", from.as_str())) {
        return Err(bad("cannot move a folder into itself"));
    }
    let case_only = from.as_str().to_lowercase() == to.as_str().to_lowercase();
    if !case_only && vault::unique(&s.root, &to) != to {
        return Err(conflict(None, "something with that name already exists"));
    }
    std::fs::rename(&src, &dst).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    blocking_rescan(&s).await;
    Ok(Json(json!({ "path": to })).into_response())
}

#[derive(Deserialize)]
struct PathBody {
    path: String,
}

async fn mkdir(State(s): St, caller: Caller, Json(b): Json<PathBody>) -> Res {
    caller.need(Role::Writer)?;
    let p = vp(&b.path)?;
    if vault::unique(&s.root, &p) != p {
        return Err(conflict(None, "something with that name already exists"));
    }
    let abs = s.abs(&p, false)?;
    std::fs::create_dir(&abs).map_err(bad)?;
    blocking_rescan(&s).await;
    Ok(Json(json!({ "path": p })).into_response())
}

pub const FOLDER_META: &str = ".folders.yaml";

/// Folder icons/colours: the client sends the whole file; it is stored as written.
async fn folder_meta_put(State(s): St, caller: Caller, body: String) -> Res {
    caller.need(Role::Writer)?;
    if body.len() > 256 * 1024 {
        return Err(err(StatusCode::PAYLOAD_TOO_LARGE, "too large"));
    }
    vault::atomic_write(&s.root.join(FOLDER_META), body.as_bytes()).map_err(bad)?;
    blocking_rescan(&s).await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn trash_list(State(s): St, caller: Caller) -> Res {
    caller.need(Role::Writer)?;
    Ok(Json(vault::list_trash(&s.root)).into_response())
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn trash_restore(State(s): St, caller: Caller, Json(b): Json<IdBody>) -> Res {
    caller.need(Role::Writer)?;
    let path = vault::restore(&s.root, &b.id).map_err(bad)?;
    blocking_rescan(&s).await;
    Ok(Json(json!({ "path": path })).into_response())
}

async fn trash_empty(State(s): St, caller: Caller) -> Res {
    caller.need(Role::Admin)?;
    let dir = s.root.join(vault::TRASH);
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
    Ok(Json(json!({
        "vault": s.root,
        "pin_set": st.pin_hash.is_some(),
        "ai": st.ai,
        "ai_key_set": crate::ai_key_set(&s),
        "lan": st.lan,
        "keep_awake": st.keep_awake,
        "network": lan,
        "autostart": s.shell.as_ref().map(|sh| sh.autostart(None)),
        "git": crate::git::status(&s),
        "editing_as": st.editing_as,
    }))
    .into_response())
}

#[derive(Deserialize)]
struct SettingsPut {
    ai: Option<AiSettings>,
    lan: Option<bool>,
    keep_awake: Option<bool>,
    vault: Option<PathBuf>,
    setup_done: Option<bool>,
    /// a person's id, or "" for nobody
    editing_as: Option<String>,
}

async fn settings_put(State(s): St, caller: Caller, Json(b): Json<SettingsPut>) -> Res {
    caller.need(Role::Admin)?;
    {
        let mut st = s.settings.write().unwrap();
        if let Some(ai) = b.ai {
            st.ai = ai;
        }
        if let Some(l) = b.lan {
            st.lan = l;
        }
        if let Some(k) = b.keep_awake {
            st.keep_awake = k;
        }
        if let Some(d) = b.setup_done {
            st.setup_done = d;
        }
        if let Some(id) = b.editing_as {
            st.editing_as = (!id.is_empty()).then_some(id);
        }
        if let Some(v) = b.vault {
            if !v.is_dir() {
                return Err(bad("that folder does not exist"));
            }
            st.vault = Some(v);
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
            if pin.chars().count() < 6 {
                return Err(bad("use at least 6 characters"));
            }
            st.pin_hash = Some(auth::hash_pin(&pin));
        }
        st.pin_gen += 1; // signs every device out
    }
    s.save_settings().map_err(bad)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn shell_action(State(s): St, caller: Caller, AxPath(action): AxPath<String>, body: Bytes) -> Res {
    caller.need(Role::Admin)?;
    let Some(shell) = s.shell.clone() else { return Err(err(StatusCode::NOT_IMPLEMENTED, "not running in the desktop app")) };
    match action.as_str() {
        "reveal" => {
            let b: PathBody = serde_json::from_slice(&body).map_err(bad)?;
            let abs = if b.path.is_empty() { s.root.clone() } else { s.abs(&vp(&b.path)?, true)? };
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
            let found = tokio::task::spawn_blocking(move || shell.update_check()).await.unwrap().map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
            let (version, notes) = found.unzip();
            Ok(Json(json!({ "version": version, "notes": notes })).into_response())
        }
        "update-install" => {
            tokio::task::spawn_blocking(move || shell.update_install()).await.unwrap().map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
            Ok(StatusCode::NO_CONTENT.into_response())
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
