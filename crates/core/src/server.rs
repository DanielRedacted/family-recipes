//! HTTP API + embedded UI. One router serves the desktop window (loopback) and LAN devices.

use crate::auth::{self, Caller, Role};
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

#[derive(RustEmbed)]
#[folder = "../../vault-seed"]
pub struct Seed;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Listener {
    Loopback,
    Lan,
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
    #[serde(default)]
    pub seeded: bool,
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
    fn keychain_get(&self, _key: &str) -> Option<String> {
        None
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
            Listener::Lan => {
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
        let body = json!({ "files": files, "folders": idx.folders });
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

/// Build state: load settings, seed the vault on first run, scan, start the watcher.
pub fn init(cfg: Config) -> Result<Arc<AppState>> {
    std::fs::create_dir_all(&cfg.data_dir)?;
    let settings: Settings = std::fs::read(cfg.data_dir.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
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
    if !state.settings.read().unwrap().seeded {
        let n = vault::seed(
            &state.root,
            Seed::iter().filter_map(|f| Seed::get(&f).map(|d| (f.to_string(), d.data.into_owned()))),
        )?;
        tracing::info!("seeded {n} recipes into {}", state.root.display());
        state.settings.write().unwrap().seeded = true;
        state.save_settings()?;
    }
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
                p.strip_prefix(&root).is_ok_and(|r| !r.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')))
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
        }
    });
    Ok(())
}

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/ping", get(|| async { "ok" }))
        .route("/api/session", get(session).delete(logout))
        .route("/api/session/pin", post(login))
        .route("/api/vault", get(vault_payload))
        .route("/api/events", get(events))
        .route("/api/files/{*path}", get(get_file).put(put_file).delete(delete_file))
        .route("/api/fs/move", post(move_path))
        .route("/api/fs/mkdir", post(mkdir))
        .route("/api/trash", get(trash_list).delete(trash_empty))
        .route("/api/trash/restore", post(trash_restore))
        .route("/api/download", get(download))
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

async fn session(State(s): St, Caller(role): Caller) -> Response {
    let st = s.settings.read().unwrap();
    Json(json!({
        "role": role,
        "pin_set": st.pin_hash.is_some(),
        "ai": st.ai.enabled,
        "shell": s.shell.is_some() && role == Role::Admin,
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

#[derive(Deserialize)]
struct PinBody {
    pin: String,
}

async fn login(State(s): St, ConnectInfo(addr): ConnectInfo<SocketAddr>, Json(b): Json<PinBody>) -> Res {
    let ip = addr.ip();
    if let Err(wait) = s.limiter.check(ip) {
        return Err(err(StatusCode::TOO_MANY_REQUESTS, format!("too many attempts, wait {wait} s")));
    }
    let (hash, pin_gen) = {
        let st = s.settings.read().unwrap();
        (st.pin_hash.clone(), st.pin_gen)
    };
    let Some(hash) = hash else { return Err(err(StatusCode::CONFLICT, "no PIN has been set in the desktop app")) };
    let ok = tokio::task::spawn_blocking(move || auth::verify_pin(&b.pin, &hash)).await.unwrap_or(false);
    if !ok {
        s.limiter.fail(ip);
        return Err(err(StatusCode::UNAUTHORIZED, "wrong PIN"));
    }
    s.limiter.success(ip);
    let v = auth::make_session(&s.cookie_key, pin_gen);
    let cookie = format!("{}={v}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={}", auth::SESSION_COOKIE, 90 * 86400);
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"role": "writer"}))).into_response())
}

async fn logout() -> Response {
    let c = format!("{}=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0", auth::SESSION_COOKIE);
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
    let stream = rx.filter_map(|m| m.ok()).map(|etag| Ok(Event::default().event("vault").data(etag)));
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

#[derive(Deserialize)]
struct DownloadQ {
    path: Option<String>,
}

async fn download(State(s): St, Query(q): Query<DownloadQ>) -> Res {
    let (abs, name) = match q.path.as_deref().filter(|p| !p.is_empty()) {
        Some(raw) => {
            let p = vp(raw)?;
            (s.abs(&p, true)?, p.name().to_string())
        }
        None => (s.root.clone(), "Recipes".to_string()),
    };
    if abs.is_file() {
        let bytes = tokio::fs::read(&abs).await.map_err(bad)?;
        let disp = format!("attachment; filename*=UTF-8''{}", urlencode(&name));
        return Ok(([(header::CONTENT_DISPOSITION, disp)], bytes).into_response());
    }
    let root = s.root.clone();
    let bytes = tokio::task::spawn_blocking(move || zip_dir(&root, &abs))
        .await
        .unwrap()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let disp = format!("attachment; filename*=UTF-8''{}.zip", urlencode(&name));
    Ok(([(header::CONTENT_TYPE, "application/zip".to_string()), (header::CONTENT_DISPOSITION, disp)], bytes).into_response())
}

fn zip_dir(root: &Path, dir: &Path) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let base = dir.parent().unwrap_or(root);
        let base = if dir == root { root } else { base };
        for e in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(Result::ok)
        {
            if e.file_type().is_file() {
                let rel = e.path().strip_prefix(base)?.to_string_lossy().to_string();
                z.start_file(rel, opts)?;
                z.write_all(&std::fs::read(e.path())?)?;
            }
        }
        z.finish()?;
    }
    Ok(buf.into_inner())
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
    }))
    .into_response())
}

#[derive(Deserialize)]
struct SettingsPut {
    ai: Option<AiSettings>,
    lan: Option<bool>,
    keep_awake: Option<bool>,
    vault: Option<PathBuf>,
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
