//! Optional sync of the recipes folder with a (private) GitHub repository, through a bundled libgit2.
//! Connected once (first-start setup or Settings → Recipes folder). After that every change in the
//! folder is committed and pushed when online, and changes from other Macs are pulled at startup,
//! every few minutes and on "Sync now". Edits that collide never lose text: the other version is
//! saved beside ours as "<name> (from other Mac).md".

use crate::auth::{Caller, Role};
use crate::server::{AppState, err};
use anyhow::{Result, anyhow, bail};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use git2::build::CheckoutBuilder;
use git2::{Commit, Cred, Direction, ErrorClass, ErrorCode, FetchOptions, Index, IndexAddOption, IndexEntry, PushOptions, RemoteCallbacks, Repository, Signature, StatusOptions};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "family.recipes.desktop";
const KEYCHAIN_USER: &str = "github-token";
/// How often GitHub is checked for changes from other Macs (and failed uploads retried).
const EVERY: Duration = Duration::from_secs(300);
const AUTH_ERR: &str = "GitHub refused the token. Connect again with a new one.";

#[derive(Default)]
pub struct Sync {
    notify: tokio::sync::Notify,
    lock: Mutex<()>,
    status: RwLock<Status>,
}

#[derive(Serialize, Clone, Default)]
pub struct Status {
    pub connected: bool,
    /// "owner/name"
    pub remote: String,
    /// Commits made here that aren't on GitHub yet.
    pub waiting: usize,
    /// Unix seconds of the last successful sync.
    pub last_sync: Option<u64>,
    pub error: Option<String>,
    /// Files changed by the last pull.
    pub pulled: usize,
}

type St = State<Arc<AppState>>;
type Res = Result<Response, Response>;

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/git/connect", post(connect))
        .route("/api/git/sync", post(sync_now))
        .route("/api/git", delete(disconnect))
}

/// Something in the recipes folder changed (called by the file watcher).
pub fn changed(s: &AppState) {
    s.ext.git.notify.notify_one();
}

pub fn status(s: &AppState) -> Status {
    let mut st = s.ext.git.status.read().unwrap().clone();
    st.connected = s.settings.read().unwrap().git_sync;
    st
}

/// Background loop: a sync right away (pulls what changed while the app was closed), then after
/// every change in the folder and every few minutes.
pub fn spawn(s: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            let s2 = s.clone();
            let _ = tokio::task::spawn_blocking(move || run(&s2)).await;
            tokio::select! {
                _ = s.ext.git.notify.notified() => {}
                _ = tokio::time::sleep(EVERY) => {}
            }
        }
    });
}

/// One sync round when sync is on (blocking). Records the outcome in the status.
fn run(s: &AppState) -> Status {
    let g = &s.ext.git;
    let _one_at_a_time = g.lock.lock().unwrap();
    if !s.settings.read().unwrap().git_sync {
        return status(s);
    }
    let res = sync_once(&s.root, token().as_deref());
    if let Err(e) = &res {
        tracing::warn!("recipes sync: {e:#}");
    }
    {
        let mut st = g.status.write().unwrap();
        if let Ok(repo) = Repository::open(&s.root) {
            st.remote = repo.find_remote("origin").ok().and_then(|r| r.url().ok().map(display_url)).unwrap_or_default();
            st.waiting = waiting(&repo);
        }
        match res {
            Ok(n) => {
                st.error = None;
                st.pulled = n;
                st.last_sync = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs());
            }
            Err(e) => {
                st.error = Some(format!("{e:#}"));
                st.pulled = 0;
            }
        }
    }
    status(s)
}

// ---------- routes ----------

#[derive(Deserialize)]
struct ConnectBody {
    url: String,
    #[serde(default)]
    token: String,
}

async fn connect(State(s): St, caller: Caller, Json(b): Json<ConnectBody>) -> Res {
    caller.need(Role::Admin)?;
    let s2 = s.clone();
    let st = tokio::task::spawn_blocking(move || -> Result<Status> {
        let url = repo_url(&b.url)?;
        let given = b.token.trim().to_string();
        let token = if given.is_empty() { token() } else { Some(given.clone()) };
        {
            let _one_at_a_time = s2.ext.git.lock.lock().unwrap();
            prepare(&s2.root, &url, token.as_deref())?;
            sync_once(&s2.root, token.as_deref())?; // a bad token or repository name fails here, before anything is saved
        }
        if !given.is_empty() {
            entry()?.set_password(&given).map_err(|e| anyhow!("Keychain: {e}"))?;
        }
        s2.settings.write().unwrap().git_sync = true;
        s2.save_settings()?;
        Ok(run(&s2))
    })
    .await
    .unwrap()
    .map_err(|e| err(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    Ok(Json(st).into_response())
}

async fn sync_now(State(s): St, caller: Caller) -> Res {
    caller.need(Role::Admin)?;
    let s2 = s.clone();
    Ok(Json(tokio::task::spawn_blocking(move || run(&s2)).await.unwrap()).into_response())
}

/// Stop syncing and forget the token. The files and their git history stay in the folder.
async fn disconnect(State(s): St, caller: Caller) -> Res {
    caller.need(Role::Admin)?;
    s.settings.write().unwrap().git_sync = false;
    s.save_settings().map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let _ = tokio::task::spawn_blocking(|| entry().map(|e| e.delete_credential())).await;
    Ok(Json(status(&s)).into_response())
}

// ---------- token ----------

fn entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)?)
}

fn token() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|t| !t.is_empty())
}

fn callbacks(token: Option<&str>) -> RemoteCallbacks<'_> {
    let mut cb = RemoteCallbacks::new();
    let mut tries = 0;
    cb.credentials(move |_, _, _| {
        // libgit2 asks again after a refusal; answering once avoids an endless loop
        tries += 1;
        match token {
            Some(t) if tries == 1 => Cred::userpass_plaintext("x-access-token", t),
            _ => Err(git2::Error::from_str(AUTH_ERR)),
        }
    });
    cb
}

fn friendly(e: git2::Error) -> anyhow::Error {
    let m = e.message();
    if e.code() == ErrorCode::Auth || m.contains(AUTH_ERR) || m.contains("401") || m.contains("403") || m.contains("authentication") {
        anyhow!(AUTH_ERR)
    } else if m.contains("404") {
        anyhow!("GitHub can't find that repository, or the token doesn't give access to it.")
    } else if matches!(e.class(), ErrorClass::Net | ErrorClass::Http | ErrorClass::Ssl | ErrorClass::Os) {
        anyhow!("Couldn't reach GitHub ({m}). Changes are kept and uploaded once the Mac is online.")
    } else {
        anyhow!("{m}")
    }
}

// ---------- git ----------

/// `owner/name`, a github.com address (https or ssh form) → the https clone URL. An absolute path
/// (a local repository) is passed through for tests.
fn repo_url(raw: &str) -> Result<String> {
    let u = raw.trim().trim_end_matches('/');
    let u = u.strip_prefix("git@github.com:").or_else(|| u.strip_prefix("github.com/")).map_or(u.to_string(), |r| format!("https://github.com/{r}"));
    if u.starts_with("https://") || u.starts_with('/') {
        Ok(u)
    } else if u.split('/').count() == 2 && !u.contains(':') && !u.contains(' ') {
        Ok(format!("https://github.com/{u}.git"))
    } else {
        bail!("Enter the repository as owner/name or its https://github.com/… address.")
    }
}

fn display_url(u: &str) -> String {
    u.trim_start_matches("https://github.com/").trim_end_matches(".git").to_string()
}

/// Make the recipes folder a copy of `url`: an existing clone of it is reused, an empty folder
/// (dot files such as .DS_Store don't count) is set up to receive it on the first sync.
fn prepare(root: &Path, url: &str, token: Option<&str>) -> Result<()> {
    let repo = match Repository::open(root) {
        Ok(r) => r,
        Err(_) => {
            if std::fs::read_dir(root)?.flatten().any(|e| !e.file_name().to_string_lossy().starts_with('.')) {
                bail!("This folder already has files in it. Choose an empty folder first (Use a different folder…), then connect.");
            }
            Repository::init(root)?
        }
    };
    let fresh = repo.head().is_err(); // nothing checked out yet
    match repo.find_remote("origin") {
        Ok(r) if r.url().ok().map(display_url) == Some(display_url(url)) => {}
        Ok(r) if !fresh => bail!("This folder is already a copy of {}.", r.url().ok().map(display_url).unwrap_or_default()),
        Ok(_) => repo.remote_set_url("origin", url)?,
        Err(_) => {
            repo.remote("origin", url)?;
        }
    }
    if fresh {
        let mut remote = repo.find_remote("origin")?;
        let conn = remote.connect_auth(Direction::Fetch, Some(callbacks(token)), None).map_err(friendly)?;
        // an empty repository has no default branch yet
        let branch = conn.default_branch().ok().and_then(|b| b.as_str().ok().map(str::to_string)).unwrap_or_else(|| "refs/heads/main".into());
        drop(conn);
        repo.set_head(&branch)?;
    }
    // never upload the app's trash or Finder's files, whatever the repository's .gitignore says
    let ex = repo.path().join("info/exclude");
    let mut cur = std::fs::read_to_string(&ex).unwrap_or_default();
    for line in [".trash/", ".DS_Store"] {
        if !cur.lines().any(|l| l == line) {
            if !cur.is_empty() && !cur.ends_with('\n') {
                cur.push('\n');
            }
            cur += line;
            cur.push('\n');
        }
    }
    std::fs::create_dir_all(ex.parent().unwrap())?;
    std::fs::write(&ex, cur)?;
    Ok(())
}

/// Commit everything, pull, push. Returns the number of files the pull changed.
fn sync_once(root: &Path, token: Option<&str>) -> Result<usize> {
    let repo = Repository::open(root)?;
    if repo.state() != git2::RepositoryState::Clean {
        bail!("The recipes folder's git repository is in the middle of a merge or rebase. Finish it in Terminal first.");
    }
    let branch_ref = repo.find_reference("HEAD")?.symbolic_target()?.map(str::to_string).ok_or_else(|| anyhow!("The recipes folder's git repository isn't on a branch."))?;
    let branch = branch_ref.trim_start_matches("refs/heads/").to_string();
    let tracking = format!("refs/remotes/origin/{branch}");
    commit_all(&repo)?;
    let mut pulled = 0;
    // a push refused because GitHub got a newer commit in between: pull that and try once more
    for _ in 0..2 {
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(callbacks(token));
        repo.find_remote("origin")?.fetch(&[format!("+{branch_ref}:{tracking}")], Some(&mut fo), None).map_err(friendly)?;
        pulled += pull(&repo, &branch_ref, &tracking)?;
        let local = repo.refname_to_id(&branch_ref).ok();
        if local.is_none() || local == repo.refname_to_id(&tracking).ok() {
            return Ok(pulled);
        }
        if push(&repo, &branch_ref, token)? {
            repo.reference(&tracking, local.unwrap(), true, "pushed")?;
            return Ok(pulled);
        }
    }
    bail!("GitHub kept changing during the upload; trying again in a few minutes.")
}

/// `git add -A && git commit`, with a message naming what changed.
fn commit_all(repo: &Repository) -> Result<()> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let changed: Vec<String> = repo.statuses(Some(&mut opts))?.iter().filter_map(|e| e.path().ok().map(str::to_string)).collect();
    if changed.is_empty() {
        return Ok(());
    }
    let mut idx = repo.index()?;
    idx.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    idx.update_all(["*"], None)?;
    idx.write()?;
    let tree = repo.find_tree(idx.write_tree()?)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if parent.as_ref().is_some_and(|p| p.tree_id() == tree.id()) {
        return Ok(());
    }
    let msg = match changed.as_slice() {
        [one] => format!("Update {one}"),
        many => format!("Update {} files", many.len()),
    };
    let sig = signature(repo);
    let parents: Vec<&Commit> = parent.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &parents)?;
    Ok(())
}

fn signature(repo: &Repository) -> Signature<'static> {
    repo.signature().unwrap_or_else(|_| Signature::now("Recipes", "recipes@localhost").unwrap())
}

/// Bring in GitHub's commits (fast-forward, or a merge). Returns the number of files that changed.
fn pull(repo: &Repository, branch_ref: &str, tracking: &str) -> Result<usize> {
    let Ok(theirs_id) = repo.refname_to_id(tracking) else { return Ok(0) }; // nothing on GitHub yet
    let theirs = repo.find_commit(theirs_id)?;
    let ours = repo.refname_to_id(branch_ref).ok().map(|id| repo.find_commit(id)).transpose()?;
    let target = match &ours {
        None => theirs, // first sync of a new copy
        Some(o) if o.id() == theirs_id || repo.graph_descendant_of(o.id(), theirs_id)? => return Ok(0),
        Some(o) if repo.graph_descendant_of(theirs_id, o.id())? => theirs,
        Some(o) => merge(repo, o, &theirs)?,
    };
    let old_tree = ours.as_ref().map(|o| o.tree()).transpose()?;
    let changed = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&target.tree()?), None)?.deltas().len();
    // safe: a file edited since the commit above is left alone (and this round fails, to retry later)
    repo.checkout_tree(target.as_object(), Some(CheckoutBuilder::new().safe()))?;
    repo.reference(branch_ref, target.id(), true, "sync")?;
    Ok(changed)
}

/// Merge their commit into ours. A file both sides changed keeps our version under its name and
/// gets theirs beside it; a file deleted on one side and changed on the other is kept.
fn merge<'r>(repo: &'r Repository, ours: &Commit, theirs: &Commit) -> Result<Commit<'r>> {
    let mut idx = repo.merge_commits(ours, theirs, None)?;
    if idx.has_conflicts() {
        let conflicts = idx.conflicts()?.collect::<Result<Vec<_>, _>>()?;
        for c in conflicts {
            let Some(path) = c.our.as_ref().or(c.their.as_ref()).or(c.ancestor.as_ref()).map(|e| e.path.clone()) else { continue };
            idx.conflict_remove(Path::new(OsStr::from_bytes(&path)))?;
            let both = c.our.is_some() && c.their.is_some();
            if let Some(e) = c.our {
                add_resolved(&mut idx, e, None)?;
            }
            // the contents are written again from the merged recipes: no copy, which would show as a recipe
            if both && path == crate::toc::FILE.as_bytes() {
                continue;
            }
            if let Some(e) = c.their {
                let copy = both.then(|| conflict_copy(&idx, &path));
                add_resolved(&mut idx, e, copy)?;
            }
        }
    }
    let tree = repo.find_tree(idx.write_tree_to(repo)?)?;
    let sig = signature(repo);
    let id = repo.commit(None, &sig, &sig, "Merge recipe changes from GitHub", &tree, &[ours, theirs])?;
    Ok(repo.find_commit(id)?)
}

fn add_resolved(idx: &mut Index, mut e: IndexEntry, path: Option<Vec<u8>>) -> Result<()> {
    e.flags &= !0x3000; // GIT_INDEX_ENTRY_STAGEMASK: stage 0, resolved
    if let Some(p) = path {
        e.path = p;
    }
    idx.add(&e)?;
    Ok(())
}

/// "Cookies/Macaroons.md" → "Cookies/Macaroons (from other Mac).md", numbered when that's taken.
fn conflict_copy(idx: &Index, path: &[u8]) -> Vec<u8> {
    let p = String::from_utf8_lossy(path);
    let name_at = p.rfind('/').map_or(0, |i| i + 1);
    let (stem, ext) = match p.rfind('.') {
        Some(i) if i > name_at => (&p[..i], &p[i..]),
        _ => (&p[..], ""),
    };
    (1..)
        .map(|n| if n == 1 { format!("{stem} (from other Mac){ext}") } else { format!("{stem} (from other Mac {n}){ext}") })
        .find(|c| idx.get_path(Path::new(c), 0).is_none())
        .unwrap()
        .into_bytes()
}

/// Returns false when GitHub refused because it has commits we don't (pull, then try again).
fn push(repo: &Repository, branch_ref: &str, token: Option<&str>) -> Result<bool> {
    let mut accepted = true;
    let res = {
        let mut cb = callbacks(token);
        cb.push_update_reference(|_, refused| {
            if refused.is_some() {
                accepted = false;
            }
            Ok(())
        });
        let mut po = PushOptions::new();
        po.remote_callbacks(cb);
        repo.find_remote("origin")?.push(&[format!("{branch_ref}:{branch_ref}")], Some(&mut po))
    };
    match res {
        Ok(()) => Ok(accepted),
        Err(e) if e.code() == ErrorCode::NotFastForward || e.message().contains("fastforward") || e.message().contains("fast-forward") => Ok(false),
        Err(e) => Err(friendly(e)),
    }
}

/// Commits on this Mac that GitHub doesn't have yet.
fn waiting(repo: &Repository) -> usize {
    (|| -> Result<usize> {
        let branch_ref = repo.find_reference("HEAD")?.symbolic_target()?.unwrap_or_default().to_string();
        let mut walk = repo.revwalk()?;
        walk.push(repo.refname_to_id(&branch_ref)?)?;
        if let Ok(id) = repo.refname_to_id(&branch_ref.replace("refs/heads/", "refs/remotes/origin/")) {
            walk.hide(id)?;
        }
        Ok(walk.count())
    })()
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }
    fn read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn two_macs_sync_without_losing_text() {
        let t = std::env::temp_dir().join(format!("recipes-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let remote = t.join("remote.git");
        Repository::init_bare(&remote).unwrap();
        let url = remote.to_str().unwrap();
        let (a, b) = (t.join("a"), t.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        let mac = a.join("Cookies/Macaroons.md");
        let mac_b = b.join("Cookies/Macaroons.md");
        let copy = |d: &Path| d.join("Cookies/Macaroons (from other Mac).md");

        // A starts the (empty) repository
        prepare(&a, url, None).unwrap();
        write(&mac, "# Macaroons\n\n- 1 cup coconut\n");
        sync_once(&a, None).unwrap();
        assert_eq!(waiting(&Repository::open(&a).unwrap()), 0);

        // B is a new Mac: its empty folder (only Finder's .DS_Store) receives the recipes
        write(&b.join(".DS_Store"), "x");
        prepare(&b, url, None).unwrap();
        assert_eq!(sync_once(&b, None).unwrap(), 1);
        assert!(read(&mac_b).contains("1 cup coconut"));

        // both change the same line before syncing: B keeps its text and gets A's beside it
        write(&mac, "# Macaroons\n\n- 2 cups coconut\n");
        write(&mac_b, "# Macaroons\n\n- 1 cup shredded coconut\n");
        sync_once(&a, None).unwrap();
        sync_once(&b, None).unwrap();
        assert!(read(&mac_b).contains("shredded"));
        assert!(read(&copy(&b)).contains("2 cups"));

        // A gets B's merge
        assert_eq!(sync_once(&a, None).unwrap(), 2);
        assert!(read(&mac).contains("shredded"));
        assert!(read(&copy(&a)).contains("2 cups"));

        // deletions travel too; .DS_Store never does
        std::fs::remove_file(copy(&a)).unwrap();
        sync_once(&a, None).unwrap();
        sync_once(&b, None).unwrap();
        assert!(!copy(&b).exists());
        assert!(!a.join(".DS_Store").exists());

        // the contents changed on both Macs: each keeps its own (rewritten on the next rescan), no copy
        write(&a.join("README.md"), "A\n");
        write(&b.join("README.md"), "B\n");
        sync_once(&a, None).unwrap();
        sync_once(&b, None).unwrap();
        assert_eq!(read(&b.join("README.md")), "B\n");
        assert!(!b.join("README (from other Mac).md").exists());

        // a folder that already has recipes but no repository is refused
        let c = t.join("c");
        write(&c.join("Pie.md"), "# Pie\n");
        assert!(prepare(&c, url, None).is_err());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn repo_urls() {
        assert_eq!(repo_url("me/recipes").unwrap(), "https://github.com/me/recipes.git");
        assert_eq!(repo_url("git@github.com:me/recipes.git").unwrap(), "https://github.com/me/recipes.git");
        assert_eq!(display_url(&repo_url("https://github.com/me/recipes/").unwrap()), "me/recipes");
        assert!(repo_url("recipes").is_err());
    }
}
