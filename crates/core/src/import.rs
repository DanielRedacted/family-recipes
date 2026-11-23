//! Import ("downloader"): web pages and files become a draft the user reviews before saving.
//! Nothing is written to the vault until the user saves; originals (PDF/DOCX/...) are always kept.

use crate::auth::{Caller, Role};
use crate::extract::{self, Candidate};
use crate::fetch;
use crate::server::{AppState, err};
use crate::vault::{self, VaultPath};
use anyhow::{Result, anyhow};
use axum::Json;
use axum::extract::{Multipart, Path as AxPath, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

type St = State<Arc<AppState>>;
type Res = Result<Response, Response>;

#[derive(Clone, Serialize)]
struct LogStep {
    ms: u64,
    step: String,
    detail: String,
    ok: bool,
}

enum Status {
    Running,
    Ready(serde_json::Value),
    Failed(String),
}

pub struct Job {
    created: Instant,
    source_url: Option<String>,
    images: Vec<Candidate>,
    /// fetched or embedded image bytes by candidate index: (bytes, extension)
    bytes: HashMap<usize, (Vec<u8>, &'static str)>,
    /// original upload kept alongside the note: (file name, bytes)
    original: Option<(String, Vec<u8>)>,
    /// original already in the vault (PDF converted from the viewer)
    original_path: Option<String>,
    status: Status,
    log: Vec<LogStep>,
}

impl Job {
    fn new(source_url: Option<String>, original_path: Option<String>) -> Self {
        Job { created: Instant::now(), source_url, images: vec![], bytes: HashMap::new(), original: None, original_path, status: Status::Running, log: vec![] }
    }
}

pub struct Jobs {
    map: Mutex<HashMap<String, Job>>,
    fetches: tokio::sync::Semaphore,
}

impl Jobs {
    pub fn new() -> Self {
        Self { map: Default::default(), fetches: tokio::sync::Semaphore::new(6) }
    }
    fn insert(&self, job: Job) -> String {
        let id = crate::auth::random_token()[..16].to_string();
        let mut m = self.map.lock().unwrap();
        m.retain(|_, j| j.created.elapsed() < Duration::from_secs(1800));
        while m.len() >= 40 {
            let oldest = m.iter().min_by_key(|(_, j)| j.created).map(|(k, _)| k.clone()).unwrap();
            m.remove(&oldest);
        }
        m.insert(id.clone(), job);
        id
    }
}

/// Appends timed steps to a job's log (shown in the import screen, for debugging).
#[derive(Clone)]
struct Log {
    s: Arc<AppState>,
    id: String,
    t0: Instant,
}

impl Log {
    fn step(&self, step: &str, detail: impl Into<String>, ok: bool) {
        if let Some(j) = self.s.ext.jobs.map.lock().unwrap().get_mut(&self.id) {
            j.log.push(LogStep { ms: self.t0.elapsed().as_millis() as u64, step: step.into(), detail: detail.into(), ok });
        }
    }
}

/// What a finished import hands to the review screen.
struct Prepared {
    draft: extract::Draft,
    bytes: HashMap<usize, (Vec<u8>, &'static str)>,
    original: Option<(String, Vec<u8>)>,
    original_name: Option<String>,
}

impl From<extract::Draft> for Prepared {
    fn from(draft: extract::Draft) -> Self {
        Prepared { draft, bytes: HashMap::new(), original: None, original_name: None }
    }
}

/// Start an import in the background; the client polls `GET /api/import/{job}`.
fn spawn<F>(s: &Arc<AppState>, job: Job, work: impl FnOnce(Log) -> F) -> String
where
    F: std::future::Future<Output = Result<Prepared, String>> + Send + 'static,
{
    let id = s.ext.jobs.insert(job);
    let log = Log { s: s.clone(), id: id.clone(), t0: Instant::now() };
    let fut = work(log.clone());
    tokio::spawn(async move {
        let result = fut.await;
        let s = &log.s;
        match result {
            Ok(p) => {
                for (step, detail, ok) in &p.draft.trace {
                    log.step(step, detail.clone(), *ok);
                }
                let source = s.ext.jobs.map.lock().unwrap().get(&log.id).and_then(|j| j.source_url.clone());
                let body = response_json(s, &log.id, &p.draft, source.as_deref(), p.original_name.as_deref());
                let n = body["issues"].as_array().map_or(0, Vec::len);
                log.step("check", format!("{n} thing(s) to check"), n == 0);
                if let Some(j) = s.ext.jobs.map.lock().unwrap().get_mut(&log.id) {
                    j.images = p.draft.images;
                    j.bytes = p.bytes;
                    j.original = p.original;
                    j.status = Status::Ready(body);
                }
            }
            Err(e) => {
                log.step("failed", e.clone(), false);
                if let Some(j) = s.ext.jobs.map.lock().unwrap().get_mut(&log.id) {
                    j.status = Status::Failed(e);
                }
            }
        }
    });
    id
}

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/import/url", post(import_url))
        .route("/api/import/file", post(import_file))
        .route("/api/import/from-vault", post(import_from_vault))
        .route("/api/import/check", post(check))
        .route("/api/import/{job}/image/{i}", get(image))
        .route("/api/import/{job}/save", post(save))
        .route("/api/import/{job}", get(status).delete(cancel))
}

async fn status(State(s): St, caller: Caller, AxPath(job): AxPath<String>) -> Res {
    caller.need(Role::Writer)?;
    let m = s.ext.jobs.map.lock().unwrap();
    let j = m.get(&job).ok_or_else(|| err(StatusCode::NOT_FOUND, "this import expired — start it again"))?;
    let mut body = match &j.status {
        Status::Running => json!({ "status": "running" }),
        Status::Failed(e) => json!({ "status": "failed", "error": e }),
        Status::Ready(v) => {
            let mut v = v.clone();
            v["status"] = json!("ready");
            v
        }
    };
    body["job"] = json!(job);
    body["log"] = json!(j.log);
    Ok(Json(body).into_response())
}

// ---------- review checks ----------

#[derive(Serialize)]
pub struct Issue {
    level: &'static str,
    message: String,
}

static BOILER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(jump to recipe|print recipe|pin (it|recipe)|rate this recipe|subscribe|newsletter|advertisement|cookie (policy|settings)|leave a comment|watch the video|sign up)\b").unwrap()
});
static HTMLISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(</?(div|span|p|br|li|ul|script|style)\b[^>]*>|&(amp|nbsp|quot|#\d+);)").unwrap());
static QTY_SHORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d|[½¼¾⅓⅔⅛])[\d/ ½¼¾⅓⅔⅛.]*\s*(cups?|tbsp|tsp|teaspoons?|tablespoons?|oz|ounces?|lbs?|pounds?|g|kg|ml)?\b").unwrap());

pub fn issues(md: &str) -> Vec<Issue> {
    let p = crate::recipe::parse(md);
    let mut out = vec![];
    let w = |m: &str| Issue { level: "warn", message: m.to_string() };
    if p.title.is_none() {
        out.push(w("No title. Add a line starting with “# ” at the top."));
    }
    if let Some(e) = &p.frontmatter_error {
        out.push(w(&format!("The properties block at the top can’t be read: {e}")));
    }
    match &p.recipe {
        None => out.push(w("No ingredient list found. Put ingredients under a “## Ingredients” heading, one per line starting with “- ”.")),
        Some(r) => {
            let n = r.ingredient_count();
            if n < 2 {
                out.push(w("Only one ingredient was found — check that each ingredient is on its own line."));
            }
            let steps: Vec<_> = r.steps.iter().flat_map(|g| &g.items).collect();
            if steps.is_empty() {
                out.push(w("No steps found. Put them under a “## Directions” heading as a numbered list."));
            }
            for g in &r.ingredients {
                for it in &g.items {
                    if it.text.chars().count() > 200 {
                        out.push(w(&format!("Line {} looks too long for one ingredient — maybe several ingredients or a step got merged.", it.line + 1)));
                    }
                }
            }
            for s in &steps {
                if QTY_SHORT.is_match(&s.text) && s.text.chars().count() < 45 && !s.text.contains('.') {
                    out.push(w(&format!("Line {} looks like an ingredient inside the steps: “{}”.", s.line + 1, s.text)));
                }
            }
            if let Some(last) = steps.last() {
                let t = last.text.trim_end();
                if t.chars().count() > 40 && !t.ends_with(['.', '!', ')', '?', '"', '”']) {
                    out.push(Issue { level: "info", message: format!("The last step may be cut off: “…{}”.", t.chars().rev().take(30).collect::<Vec<_>>().into_iter().rev().collect::<String>()) });
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for (i, line) in md.lines().enumerate() {
        let t = line.trim();
        if HTMLISH.is_match(t) {
            out.push(w(&format!("Line {} still contains web-page code: “{}”.", i + 1, t.chars().take(60).collect::<String>())));
        }
        if BOILER.is_match(t) {
            out.push(Issue { level: "info", message: format!("Line {} looks like website clutter: “{}”.", i + 1, t.chars().take(60).collect::<String>()) });
        }
        if t.len() > 12 && !t.starts_with('#') && !t.starts_with("---") && !seen.insert(t.to_lowercase()) {
            out.push(Issue { level: "info", message: format!("Line {} repeats an earlier line.", i + 1) });
        }
    }
    out.truncate(30);
    out
}

#[derive(Deserialize)]
struct CheckBody {
    markdown: String,
}

async fn check(caller: Caller, Json(b): Json<CheckBody>) -> Res {
    caller.need(Role::Writer)?;
    Ok(Json(json!({ "issues": issues(&b.markdown) })).into_response())
}

// ---------- duplicates ----------

pub fn normalize_url(raw: &str) -> String {
    let Ok(mut u) = url::Url::parse(raw.trim()) else { return raw.trim().to_lowercase() };
    let keep: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| !(k.starts_with("utm_") || ["gclid", "fbclid", "mc_cid", "mc_eid", "ref", "_ga"].contains(&k.as_ref())))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    u.set_fragment(None);
    if keep.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear().extend_pairs(keep);
    }
    let host = u.host_str().unwrap_or("").trim_start_matches("www.").to_lowercase();
    format!("{host}{}{}", u.path().trim_end_matches('/'), u.query().map(|q| format!("?{q}")).unwrap_or_default())
}

fn duplicate_of(state: &AppState, source: &str, title: &str) -> Option<String> {
    let n = normalize_url(source);
    let idx = state.index.read().unwrap().clone();
    idx.files
        .values()
        .find(|e| e.frontmatter.as_ref().and_then(|f| f.get("source")).and_then(|s| s.as_str()).is_some_and(|s| normalize_url(s) == n))
        .or_else(|| (!title.is_empty()).then(|| idx.files.values().find(|e| e.kind == vault::FileKind::Note && e.title.eq_ignore_ascii_case(title))).flatten())
        .map(|e| e.path.as_str().to_string())
}

// ---------- URL ----------

#[derive(Deserialize)]
struct UrlBody {
    url: String,
}

fn response_json(state: &AppState, id: &str, d: &extract::Draft, source: Option<&str>, original_name: Option<&str>) -> serde_json::Value {
    let dup = source.and_then(|u| duplicate_of(state, u, &d.title)).or_else(|| duplicate_of(state, "", &d.title));
    let images: Vec<_> = d.images.iter().enumerate().map(|(i, c)| json!({ "i": i, "alt": c.alt, "kind": c.kind, "step": c.step, "url": c.url })).collect();
    json!({
        "job": id,
        "title": d.title,
        "markdown": d.markdown,
        "source_text": d.source_text,
        "method": d.method,
        "images": images,
        "issues": issues(&d.markdown),
        "duplicate": dup,
        "original": original_name,
    })
}

fn started(id: String) -> Res {
    Ok((StatusCode::ACCEPTED, Json(json!({ "job": id }))).into_response())
}

async fn import_url(State(s): St, caller: Caller, Json(b): Json<UrlBody>) -> Res {
    caller.need(Role::Writer)?;
    let u = fetch::check_url(&b.url).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let id = spawn(&s, Job::new(Some(u.to_string()), None), move |log| async move {
        let page = fetch::get(&u, None, 5 * 1024 * 1024, "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8").await.map_err(|e| format!("{e:#}"))?;
        let redirected = if page.url != u { format!(" → {}", page.url) } else { String::new() };
        log.step("fetch", format!("{} KB, {}{redirected}", page.bytes.len() / 1024, if page.content_type.is_empty() { "no content type" } else { &page.content_type }), true);
        if page.url != u {
            if let Some(j) = log.s.ext.jobs.map.lock().unwrap().get_mut(&log.id) {
                j.source_url = Some(page.url.to_string());
            }
        }
        if !page.content_type.is_empty() && !page.content_type.contains("html") && !page.content_type.contains("xml") {
            return Err("That address is not a web page (download the file and import it instead).".into());
        }
        let html = String::from_utf8_lossy(&page.bytes).into_owned();
        let rest = match extract::wprm_id(&html) {
            Some(rid) => wprm_rest(&page.url, rid, &log).await,
            None => None,
        };
        let final_url = page.url.clone();
        let mut d = tokio::task::spawn_blocking(move || extract::from_html(&html, Some(&final_url), rest.as_ref())).await.map_err(|e| e.to_string())?;
        // the page's own recipe data is kept as the original; a page without it keeps only its address (`source`)
        let original = d.json.take().map(|v| (format!("{}.json", if d.title.is_empty() { "Recipe" } else { &d.title }), serde_json::to_vec_pretty(&v).unwrap_or_default()));
        Ok(Prepared { original_name: original.as_ref().map(|(n, _)| n.clone()), original, ..d.into() })
    });
    started(id)
}

/// WP Recipe Maker sites expose the full recipe (equipment, nutrition, groups) as JSON.
async fn wprm_rest(page: &url::Url, rid: u64, log: &Log) -> Option<serde_json::Value> {
    let api = page.join(&format!("/wp-json/wp/v2/wprm_recipe/{rid}")).ok()?;
    match fetch::get(&api, Some(page.as_str()), 2 * 1024 * 1024, "application/json").await {
        Ok(f) => match serde_json::from_slice::<serde_json::Value>(&f.bytes) {
            Ok(v) if v["recipe"]["name"].as_str().is_some_and(|n| !n.trim().is_empty()) => {
                log.step("wprm-rest", format!("recipe {rid} from {}", api.path()), true);
                Some(v["recipe"].clone())
            }
            Ok(_) => {
                log.step("wprm-rest", format!("recipe {rid}: the site's answer has no recipe data"), false);
                None
            }
            Err(_) => {
                log.step("wprm-rest", format!("recipe {rid}: the site did not answer with JSON"), false);
                None
            }
        },
        Err(e) => {
            log.step("wprm-rest", format!("recipe {rid}: {e:#}"), false);
            None
        }
    }
}

/// Thumbnail proxy: only serves images the server itself listed for this job.
async fn image(State(s): St, caller: Caller, AxPath((job, i)): AxPath<(String, usize)>) -> Res {
    caller.need(Role::Writer)?;
    let (cand, cached, referer) = {
        let m = s.ext.jobs.map.lock().unwrap();
        let j = m.get(&job).ok_or_else(|| err(StatusCode::NOT_FOUND, "import expired"))?;
        (j.images.get(i).cloned(), j.bytes.get(&i).cloned(), j.source_url.clone())
    };
    let (bytes, ext) = match cached {
        Some(c) => c,
        None => {
            let c = cand.ok_or_else(|| err(StatusCode::NOT_FOUND, "no such image"))?;
            let u = fetch::check_url(&c.url).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
            let _permit = s.ext.jobs.fetches.acquire().await.unwrap();
            let f = fetch::get(&u, referer.as_deref(), 15 * 1024 * 1024, "image/avif,image/webp,image/*;q=0.8")
                .await
                .map_err(|e| err(StatusCode::BAD_GATEWAY, e))?;
            let (_, ext) = fetch::image_type(&f.bytes).ok_or_else(|| err(StatusCode::UNSUPPORTED_MEDIA_TYPE, "not an image"))?;
            let mut m = s.ext.jobs.map.lock().unwrap();
            if let Some(j) = m.get_mut(&job) {
                let total: usize = j.bytes.values().map(|b| b.0.len()).sum();
                if total + f.bytes.len() < 100 * 1024 * 1024 {
                    j.bytes.insert(i, (f.bytes.clone(), ext));
                }
            }
            (f.bytes, ext)
        }
    };
    let mime = fetch::image_type(&bytes).map(|t| t.0).unwrap_or("application/octet-stream");
    let _ = ext;
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=1800")], bytes).into_response())
}

async fn cancel(State(s): St, caller: Caller, AxPath(job): AxPath<String>) -> Res {
    caller.need(Role::Writer)?;
    s.ext.jobs.map.lock().unwrap().remove(&job);
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------- save ----------

#[derive(Deserialize)]
struct SaveBody {
    folder: String,
    name: String,
    markdown: String,
    #[serde(default)]
    images: Vec<usize>,
    cover: Option<usize>,
}

fn slug(s: &str) -> String {
    let s: String = s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "recipe".into() } else { s.chars().take(60).collect() }
}

fn md_path(p: &str) -> String {
    if p.contains([' ', '(', ')']) { format!("</{p}>") } else { format!("/{p}") }
}

/// Insert or replace a frontmatter key in markdown text.
fn set_frontmatter(md: &str, key: &str, value: &str) -> String {
    let line = format!("{key}: {}", serde_json::to_string(value).unwrap());
    if let Some((yaml, start, _)) = crate::recipe::split_frontmatter(md) {
        let mut lines: Vec<String> = yaml.lines().filter(|l| !l.starts_with(&format!("{key}:"))).map(str::to_string).collect();
        lines.push(line);
        format!("---\n{}\n---\n{}", lines.join("\n"), &md[start..])
    } else {
        format!("---\n{line}\n---\n\n{md}")
    }
}

fn clean_name(s: &str) -> String {
    let n: String = s.trim().chars().map(|c| if matches!(c, '/' | '\\' | ':') || c.is_control() { '-' } else { c }).collect();
    let n = n.trim_start_matches('.').trim().to_string();
    if n.is_empty() { "Untitled".into() } else { n.chars().take(120).collect() }
}

fn note_path(state: &AppState, folder: &str, name: &str) -> Result<VaultPath> {
    let name = clean_name(name);
    let rel = if folder.trim_matches('/').is_empty() { format!("{name}.md") } else { format!("{}/{name}.md", folder.trim_matches('/')) };
    let p = VaultPath::parse(&rel)?;
    if let Some(parent) = p.parent() {
        let dir = VaultPath::parse(parent)?;
        vault::resolve(&state.root, &dir, false)?; // parent chain must be safe
        std::fs::create_dir_all(state.root.join(parent))?;
    }
    Ok(vault::unique(&state.root, &p))
}

/// A file converted from inside the vault moves under attachments/originals, so it stops being its own entry.
fn move_to_originals(state: &AppState, p: &str) -> Result<VaultPath> {
    let src = VaultPath::parse(p)?;
    if p.starts_with(&format!("{}/", vault::ORIGINALS)) {
        return Ok(src);
    }
    std::fs::create_dir_all(state.root.join(vault::ORIGINALS))?;
    let dst = vault::unique(&state.root, &VaultPath::parse(&format!("{}/{}", vault::ORIGINALS, src.name()))?);
    std::fs::rename(vault::resolve(&state.root, &src, true)?, vault::resolve(&state.root, &dst, false)?)?;
    Ok(dst)
}

fn save_attachment(state: &AppState, name: &str, bytes: &[u8]) -> Result<VaultPath> {
    std::fs::create_dir_all(state.root.join(vault::ORIGINALS))?;
    let p = vault::unique(&state.root, &VaultPath::parse(&format!("{}/{}", vault::ATTACHMENTS, name))?);
    let abs = vault::resolve(&state.root, &p, false)?;
    vault::atomic_write(&abs, bytes)?;
    Ok(p)
}

async fn save(State(s): St, caller: Caller, AxPath(job): AxPath<String>, Json(b): Json<SaveBody>) -> Res {
    caller.need(Role::Writer)?;
    let bad = |e: anyhow::Error| err(StatusCode::BAD_REQUEST, format!("{e:#}"));
    let path = note_path(&s, &b.folder, &b.name).map_err(bad)?;
    let mut md = b.markdown.replace("\r\n", "\n");
    let title = crate::recipe::parse(&md).title.unwrap_or_else(|| b.name.clone());
    let slug = slug(&title);

    // images: fetch any not yet cached, save into attachments/
    let wanted: Vec<usize> = b.cover.into_iter().chain(b.images.iter().copied().filter(|i| Some(*i) != b.cover)).collect();
    let mut saved: Vec<(usize, VaultPath)> = vec![];
    for i in wanted {
        let (cand, cached, referer) = {
            let m = s.ext.jobs.map.lock().unwrap();
            let j = m.get(&job).ok_or_else(|| err(StatusCode::NOT_FOUND, "this import expired — start it again"))?;
            (j.images.get(i).cloned(), j.bytes.get(&i).cloned(), j.source_url.clone())
        };
        let (bytes, ext) = match cached {
            Some(c) => c,
            None => {
                let Some(c) = cand else { continue };
                let Ok(u) = fetch::check_url(&c.url) else { continue };
                match fetch::get(&u, referer.as_deref(), 15 * 1024 * 1024, "image/*").await {
                    Ok(f) => match fetch::image_type(&f.bytes) {
                        Some((_, ext)) => (f.bytes, ext),
                        None => continue,
                    },
                    Err(_) => continue,
                }
            }
        };
        let n = saved.len() + 1;
        let name = if n == 1 { format!("{slug}.{ext}") } else { format!("{slug}-{n}.{ext}") };
        let p = save_attachment(&s, &name, &bytes).map_err(bad)?;
        saved.push((i, p));
    }
    if let Some((_, cover)) = saved.iter().find(|(i, _)| Some(*i) == b.cover) {
        let line = format!("![{}]({})", title.replace([']', '['], ""), md_path(cover.as_str()));
        md = match md.lines().position(|l| l.starts_with("# ")) {
            Some(h) => {
                let mut lines: Vec<&str> = md.lines().collect();
                lines.insert(h + 1, "");
                lines.insert(h + 2, &line);
                lines.join("\n") + "\n"
            }
            None => format!("{line}\n\n{md}"),
        };
    }
    let others: Vec<String> = saved.iter().filter(|(i, _)| Some(*i) != b.cover).map(|(_, p)| format!("![]({})", md_path(p.as_str()))).collect();
    if !others.is_empty() {
        md = format!("{}\n\n## Photos\n\n{}\n", md.trim_end(), others.join("\n\n"));
    }

    // originals are always kept next to the recipe
    let (original, original_path) = {
        let mut m = s.ext.jobs.map.lock().unwrap();
        m.get_mut(&job).map(|j| (j.original.take(), j.original_path.clone())).unwrap_or((None, None))
    };
    if let Some((name, bytes)) = original {
        let p = save_attachment(&s, &format!("originals/{}", clean_name(&name)), &bytes).map_err(bad)?;
        md = set_frontmatter(&md, "original", &format!("/{}", p.as_str()));
    } else if let Some(p) = original_path {
        let p = move_to_originals(&s, &p).map_err(bad)?;
        md = set_frontmatter(&md, "original", &format!("/{}", p.as_str()));
    }

    let abs = vault::resolve(&s.root, &path, false).map_err(bad)?;
    vault::atomic_write(&abs, md.as_bytes()).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    s.ext.jobs.map.lock().unwrap().remove(&job);
    let s2 = s.clone();
    let _ = tokio::task::spawn_blocking(move || s2.rescan()).await;
    Ok(Json(json!({ "path": path })).into_response())
}

// ---------- files ----------

/// PDF text with a panic guard (pdf parsers can panic on odd files) and a size cap.
pub fn pdf_text(bytes: &[u8]) -> Option<String> {
    if bytes.len() > 25 * 1024 * 1024 {
        return None;
    }
    let b = bytes.to_vec();
    std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&b).ok())
        .ok()
        .flatten()
        .map(|t| t.replace('\u{0}', ""))
        .filter(|t| t.trim().len() > 20)
}

async fn pdf_text_async(bytes: Vec<u8>) -> Option<String> {
    let h = tokio::task::spawn_blocking(move || pdf_text(&bytes));
    tokio::time::timeout(Duration::from_secs(30), h).await.ok().and_then(Result::ok).flatten()
}

fn textutil_html(bytes: &[u8], ext: &str) -> Result<String> {
    let dir = std::env::temp_dir().join(format!("recipes-import-{}", crate::auth::random_token()[..8].to_string()));
    std::fs::create_dir_all(&dir)?;
    let f = dir.join(format!("in.{ext}"));
    std::fs::write(&f, bytes)?;
    let out = std::process::Command::new("/usr/bin/textutil").args(["-convert", "html", "-stdout"]).arg(&f).output();
    let _ = std::fs::remove_dir_all(&dir);
    let out = out?;
    if !out.status.success() {
        return Err(anyhow!("macOS could not read that document"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Default)]
struct Upload {
    name: String,
    bytes: Vec<u8>,
    /// the file is already in the vault (converted from its page): it isn't copied, it moves on save
    vault_path: String,
    mode: String,
    html: Option<String>,
    folder: String,
    title: String,
    tags: String,
}

async fn read_upload(mut mp: Multipart) -> Result<Upload> {
    let mut u = Upload::default();
    while let Some(f) = mp.next_field().await? {
        match f.name().unwrap_or("") {
            "file" => {
                u.name = f.file_name().unwrap_or("document").to_string();
                u.bytes = f.bytes().await?.to_vec();
            }
            "mode" => u.mode = f.text().await?,
            "html" => u.html = Some(f.text().await?),
            "folder" => u.folder = f.text().await?,
            "title" => u.title = f.text().await?,
            "tags" => u.tags = f.text().await?,
            "vault_path" => u.vault_path = f.text().await?,
            _ => {}
        }
    }
    if u.bytes.is_empty() {
        return Err(anyhow!("no file received"));
    }
    Ok(u)
}

fn ext_of(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

async fn import_file(State(s): St, caller: Caller, mp: Multipart) -> Res {
    caller.need(Role::Writer)?;
    let bad = |e: anyhow::Error| err(StatusCode::BAD_REQUEST, format!("{e:#}"));
    let u = read_upload(mp).await.map_err(bad)?;
    let ext = ext_of(&u.name);
    let stem = u.name.rsplit_once('.').map_or(u.name.as_str(), |(s, _)| s).to_string();

    if u.mode == "keep" {
        // view-only recipe: the original file plus a small note that points at it
        let is_img = fetch::image_type(&u.bytes).is_some();
        if ext != "pdf" && !is_img {
            return Err(err(StatusCode::BAD_REQUEST, "only PDFs and photos can be kept as view-only recipes"));
        }
        let title = if u.title.trim().is_empty() { stem.clone() } else { u.title.trim().to_string() };
        let path = note_path(&s, &u.folder, &title).map_err(bad)?;
        let orig = save_attachment(&s, &format!("originals/{}", clean_name(&u.name)), &u.bytes).map_err(bad)?;
        let tags: Vec<String> = u.tags.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        let mut md = String::from("---\n");
        if !tags.is_empty() {
            md.push_str(&format!("tags: [{}]\n", tags.iter().map(|t| serde_json::to_string(t).unwrap()).collect::<Vec<_>>().join(", ")));
        }
        md.push_str(&format!("original: {}\n---\n\n# {title}\n\n", serde_json::to_string(&format!("/{}", orig.as_str())).unwrap()));
        if is_img {
            md.push_str(&format!("![{title}]({})\n", md_path(orig.as_str())));
        } else {
            md.push_str(&format!("[{}]({})\n", orig.name(), md_path(orig.as_str())));
        }
        let abs = vault::resolve(&s.root, &path, false).map_err(bad)?;
        vault::atomic_write(&abs, md.as_bytes()).map_err(bad)?;
        let s2 = s.clone();
        let _ = tokio::task::spawn_blocking(move || s2.rescan()).await;
        return Ok(Json(json!({ "path": path })).into_response());
    }

    if !matches!(ext.as_str(), "pdf" | "docx" | "doc" | "rtf" | "odt" | "wordml" | "html" | "htm" | "txt" | "md" | "markdown" | "text") {
        return Err(err(StatusCode::UNSUPPORTED_MEDIA_TYPE, "supported files: PDF, Word (.docx/.doc), RTF, web page (.html), text and Markdown"));
    }
    if ext == "docx" && u.html.is_none() {
        return Err(err(StatusCode::BAD_REQUEST, "missing converted document"));
    }
    let in_vault = if u.vault_path.is_empty() {
        None
    } else {
        let p = VaultPath::parse(&u.vault_path).map_err(bad)?;
        vault::resolve(&s.root, &p, true).map_err(|e| err(StatusCode::NOT_FOUND, e))?;
        Some(p.as_str().to_string())
    };
    let id = spawn(&s, Job::new(None, in_vault.clone()), move |log| async move {
        log.step("file", format!("{} ({} KB)", u.name, u.bytes.len() / 1024), true);
        let mut embedded: Vec<(String, Vec<u8>)> = vec![];
        let draft = match ext.as_str() {
            "pdf" => {
                let text = pdf_text_async(u.bytes.clone())
                    .await
                    .ok_or("No text could be read from this PDF (it may be a scan). Keep it as a view-only PDF instead.")?;
                log.step("pdf-text", format!("{} characters", text.chars().count()), true);
                let md = extract::tidy_text(&text);
                draft(md, text)
            }
            "docx" => {
                let html = u.html.clone().unwrap_or_default();
                let (md, imgs) = extract::from_document_html(&html, &stem);
                log.step("docx", format!("{} characters of HTML from Word, {} embedded picture(s)", html.len(), imgs.len()), true);
                embedded = imgs;
                draft(md, extract::clean(&html))
            }
            "html" | "htm" => {
                let html = String::from_utf8_lossy(&u.bytes).into_owned();
                tokio::task::spawn_blocking(move || extract::from_html(&html, None, None)).await.map_err(|e| e.to_string())?
            }
            "txt" | "md" | "markdown" | "text" => {
                let t = String::from_utf8_lossy(&u.bytes).into_owned();
                let structured = t.contains("\n## ") || t.starts_with("# ");
                log.step("text", if structured { "already Markdown with headings; kept as is" } else { "plain text; headings and bullets added" }, true);
                let md = if structured { t.clone() } else { extract::tidy_text(&t) };
                draft(md, t)
            }
            _ => {
                let (b, e) = (u.bytes.clone(), ext.clone());
                let html = tokio::task::spawn_blocking(move || textutil_html(&b, &e)).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:#}"))?;
                let (md, imgs) = extract::from_document_html(&html, &stem);
                log.step("textutil", format!("macOS converted the document ({} characters, {} picture(s))", html.len(), imgs.len()), true);
                embedded = imgs;
                draft(md, extract::clean(&html))
            }
        };
        let mut d = draft;
        let mut bytes = HashMap::new();
        for (mime, b) in embedded {
            if let Some((_, ext)) = fetch::image_type(&b) {
                bytes.insert(d.images.len(), (b, ext));
                d.images.push(Candidate { url: String::new(), alt: mime, kind: "document".into(), step: None });
            }
        }
        // every converted file is kept; pasted text is named after its recipe
        let name = if u.name == "Pasted text.txt" && !d.title.is_empty() { format!("{}.txt", d.title) } else { u.name };
        let original = in_vault.is_none().then(|| (name.clone(), u.bytes));
        Ok(Prepared { draft: d, bytes, original_name: Some(name), original })
    });
    started(id)
}

fn draft(markdown: String, source_text: String) -> extract::Draft {
    let title = crate::recipe::parse(&markdown).title.unwrap_or_default();
    extract::Draft { title, markdown, source_text, images: vec![], method: "document".into(), trace: vec![], json: None }
}

#[derive(Deserialize)]
struct FromVault {
    path: String,
}

/// "Convert to editable recipe" for a PDF already in the vault: on save it moves under attachments/originals.
async fn import_from_vault(State(s): St, caller: Caller, Json(b): Json<FromVault>) -> Res {
    caller.need(Role::Writer)?;
    let p = VaultPath::parse(&b.path).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let abs = vault::resolve(&s.root, &p, true).map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    if p.ext() != "pdf" {
        return Err(err(StatusCode::BAD_REQUEST, "only PDFs can be converted"));
    }
    let bytes = tokio::fs::read(&abs).await.map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    let name = p.name().to_string();
    let id = spawn(&s, Job::new(None, Some(p.as_str().to_string())), move |log| async move {
        let text = pdf_text_async(bytes).await.ok_or("No text could be read from this PDF (it may be a scan).")?;
        log.step("pdf-text", format!("{} characters", text.chars().count()), true);
        Ok(Prepared { original_name: Some(name), ..draft(extract::tidy_text(&text), text).into() })
    });
    started(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_normalisation() {
        assert_eq!(normalize_url("https://www.Example.com/r/pie/?utm_source=x&gclid=1#c"), "example.com/r/pie");
        assert_eq!(normalize_url("https://example.com/r?id=4&utm_medium=y"), "example.com/r?id=4");
    }

    #[test]
    fn issue_checks() {
        let good = "# Pie\n\n## Ingredients\n\n- 1 cup flour\n- 2 eggs\n\n## Directions\n\n1. Mix.\n2. Bake.\n";
        assert!(issues(good).iter().all(|i| i.level != "warn"), "{:?}", issues(good).iter().map(|i| &i.message).collect::<Vec<_>>());
        let bad = "Some text\n\n<div>Jump to Recipe</div>\n";
        let msgs: Vec<String> = issues(bad).into_iter().map(|i| i.message).collect();
        assert!(msgs.iter().any(|m| m.contains("No title")));
        assert!(msgs.iter().any(|m| m.contains("No ingredient list")));
        assert!(msgs.iter().any(|m| m.contains("web-page code")));
        assert!(msgs.iter().any(|m| m.contains("clutter")));
    }

    #[test]
    fn frontmatter_insert() {
        assert_eq!(set_frontmatter("# T\n", "original", "/a b.pdf"), "---\noriginal: \"/a b.pdf\"\n---\n\n# T\n");
        assert_eq!(set_frontmatter("---\nyield: 2\n---\n# T\n", "original", "/x.pdf"), "---\nyield: 2\noriginal: \"/x.pdf\"\n---\n# T\n");
    }
}
