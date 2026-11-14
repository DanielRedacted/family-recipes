//! The vault: a folder of markdown (and other) files. The folder is the database.

use crate::recipe::{self, Recipe};
use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_normalization::UnicodeNormalization;

pub const TRASH: &str = ".trash";
pub const ATTACHMENTS: &str = "attachments";

/// A validated path relative to the vault root: NFC, `/`-separated, no `..`, no dot-components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct VaultPath(String);

impl VaultPath {
    pub fn parse(raw: &str) -> Result<Self> {
        let s: String = raw.trim_matches('/').nfc().collect();
        if s.is_empty() {
            bail!("empty path");
        }
        if s.len() > 1024 || s.contains('\\') || s.chars().any(|c| c.is_control()) {
            bail!("invalid characters in path");
        }
        for part in s.split('/') {
            if part.is_empty() || part.starts_with('.') || part.len() > 255 || part.trim() != part {
                bail!("invalid path component {part:?}");
            }
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap()
    }
    pub fn parent(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(p, _)| p)
    }
    pub fn stem(&self) -> &str {
        let n = self.name();
        n.rsplit_once('.').map_or(n, |(s, _)| s)
    }
    pub fn ext(&self) -> String {
        self.name().rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
    }
    pub fn join(&self, child: &str) -> Result<Self> {
        Self::parse(&format!("{}/{}", self.0, child))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Note,
    Pdf,
    Image,
    Other,
}

pub fn kind_of(p: &VaultPath) -> FileKind {
    match p.ext().as_str() {
        "md" | "markdown" => FileKind::Note,
        "pdf" => FileKind::Pdf,
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif" => FileKind::Image,
        _ => FileKind::Other,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub path: VaultPath,
    pub kind: FileKind,
    pub size: u64,
    pub mtime: u64,
    pub etag: String,
    /// Display title: H1, else filename stem.
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter_error: Option<String>,
    pub body_line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe: Option<Recipe>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    /// Extracted text for PDFs (search only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Index {
    pub files: BTreeMap<VaultPath, Entry>,
    pub folders: Vec<VaultPath>,
}

pub fn etag_of(bytes: &[u8]) -> String {
    let h = Sha256::digest(bytes);
    h[..12].iter().map(|b| format!("{b:02x}")).collect()
}

fn mtime(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

/// Hook for PDF text extraction (filled in by the import module).
pub type PdfText = fn(&[u8]) -> Option<String>;

pub fn read_entry(root: &Path, p: VaultPath, prev: Option<&Entry>, pdf: Option<PdfText>) -> Result<Entry> {
    let abs = root.join(p.as_str());
    let meta = std::fs::symlink_metadata(&abs)?;
    if !meta.is_file() {
        bail!("not a regular file");
    }
    let (size, mt) = (meta.len(), mtime(&meta));
    if let Some(prev) = prev.filter(|e| e.size == size && e.mtime == mt) {
        return Ok(prev.clone());
    }
    let kind = kind_of(&p);
    let stem = p.stem().to_string();
    let mut e = Entry {
        title: stem,
        kind: kind.clone(),
        size,
        mtime: mt,
        etag: String::new(),
        raw: None,
        frontmatter: None,
        frontmatter_error: None,
        body_line: 0,
        cover: None,
        recipe: None,
        links: vec![],
        text: None,
        path: p,
    };
    match kind {
        FileKind::Note => {
            let bytes = std::fs::read(&abs)?;
            e.etag = etag_of(&bytes);
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let parsed = recipe::parse(&text);
            if let Some(t) = parsed.title.filter(|t| !t.is_empty()) {
                e.title = t;
            }
            e.frontmatter = Some(parsed.frontmatter);
            e.frontmatter_error = parsed.frontmatter_error;
            e.body_line = parsed.body_line;
            e.cover = parsed.cover;
            e.recipe = parsed.recipe;
            e.links = parsed.links;
            e.raw = Some(text);
        }
        FileKind::Pdf => {
            let bytes = std::fs::read(&abs)?;
            e.etag = etag_of(&bytes);
            e.text = pdf.and_then(|f| f(&bytes));
        }
        _ => e.etag = format!("{size:x}-{mt:x}"),
    }
    Ok(e)
}

/// Full scan; entries whose size+mtime are unchanged are reused from `prev`.
pub fn scan(root: &Path, prev: Option<&Index>, pdf: Option<PdfText>) -> Index {
    let mut idx = Index::default();
    let walker = walkdir::WalkDir::new(root).follow_links(false).sort_by_file_name().into_iter();
    for e in walker.filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.')) {
        let Ok(e) = e else { continue };
        if e.depth() == 0 {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(root) else { continue };
        let Ok(vp) = VaultPath::parse(&rel.to_string_lossy()) else { continue };
        if e.file_type().is_dir() {
            idx.folders.push(vp);
        } else if e.file_type().is_file() {
            let old = prev.and_then(|p| p.files.get(&vp));
            match read_entry(root, vp.clone(), old, pdf) {
                Ok(entry) => {
                    idx.files.insert(vp, entry);
                }
                Err(err) => tracing::warn!("skipping {}: {err}", vp.as_str()),
            }
        }
    }
    idx
}

/// Resolve a vault path to an absolute path, refusing anything that escapes the root
/// (including through symlinks). `must_exist=false` allows a new file in an existing folder.
pub fn resolve(root: &Path, p: &VaultPath, must_exist: bool) -> Result<PathBuf> {
    let croot = root.canonicalize().context("vault root missing")?;
    let abs = croot.join(p.as_str());
    let check = if must_exist { abs.clone() } else { abs.parent().map(Path::to_path_buf).unwrap_or(croot.clone()) };
    // walk every component with symlink_metadata so no symlink is followed
    let mut cur = croot.clone();
    for c in check.strip_prefix(&croot).map_err(|_| anyhow!("outside vault"))?.components() {
        let Component::Normal(c) = c else { bail!("bad component") };
        cur.push(c);
        let m = std::fs::symlink_metadata(&cur).with_context(|| format!("{} not found", p.as_str()))?;
        if m.file_type().is_symlink() {
            bail!("symlinks are not allowed");
        }
    }
    Ok(abs)
}

/// Atomic write: temp file in the same folder, fsync, rename.
pub fn atomic_write(abs: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let dir = abs.parent().ok_or_else(|| anyhow!("no parent"))?;
    let tmp = dir.join(format!(".{}.{}.tmp", abs.file_name().unwrap().to_string_lossy(), std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, abs).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

fn now_stamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    format!("{secs}")
}

/// Move a file or folder into `.trash/<stamp>/<original path>`; `.origin` records the path.
pub fn trash(root: &Path, p: &VaultPath) -> Result<String> {
    let abs = resolve(root, p, true)?;
    let stamp = now_stamp();
    let base = root.join(TRASH).join(&stamp);
    let dest = base.join(p.as_str());
    std::fs::create_dir_all(dest.parent().unwrap())?;
    std::fs::rename(abs, dest)?;
    std::fs::write(base.join(".origin"), p.as_str())?;
    Ok(stamp)
}

/// Keep a copy of the current version in the trash before overwriting (at most one per 10 minutes).
pub fn backup_before_overwrite(root: &Path, p: &VaultPath, abs: &Path) -> Result<()> {
    let dir = root.join(TRASH).join("versions").join(p.as_str());
    std::fs::create_dir_all(&dir)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let recent = std::fs::read_dir(&dir)?.filter_map(Result::ok).any(|e| {
        e.file_name().to_string_lossy().parse::<u64>().is_ok_and(|t| now.saturating_sub(t / 1000) < 600)
    });
    if !recent {
        std::fs::copy(abs, dir.join(now_stamp()))?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct TrashItem {
    pub id: String,
    pub path: String,
    pub deleted: u64,
}

pub fn list_trash(root: &Path) -> Vec<TrashItem> {
    let mut out: Vec<TrashItem> = std::fs::read_dir(root.join(TRASH))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|d| {
            let id = d.file_name().to_string_lossy().to_string();
            let ms = id.parse::<u64>().ok()?;
            let path = std::fs::read_to_string(d.path().join(".origin")).ok()?;
            Some(TrashItem { id, path, deleted: ms / 1000 })
        })
        .collect();
    out.sort_by(|a, b| b.deleted.cmp(&a.deleted));
    out
}

pub fn restore(root: &Path, id: &str) -> Result<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        bail!("bad trash id");
    }
    let item = list_trash(root).into_iter().find(|t| t.id == id).ok_or_else(|| anyhow!("not in trash"))?;
    let src = root.join(TRASH).join(id).join(item.path.as_str());
    let target = unique(root, &VaultPath::parse(&item.path)?);
    if let Some(parent) = root.join(target.as_str()).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(src, root.join(target.as_str()))?;
    let _ = std::fs::remove_dir_all(root.join(TRASH).join(id));
    Ok(target.as_str().to_string())
}

/// First free variant of `p` ("Name (2).md", ...), case-insensitively (APFS default).
pub fn unique(root: &Path, p: &VaultPath) -> VaultPath {
    let exists = |q: &VaultPath| {
        let parent = q.parent().map_or(root.to_path_buf(), |d| root.join(d));
        let name = q.name().to_lowercase();
        std::fs::read_dir(parent)
            .map(|r| r.filter_map(Result::ok).any(|e| e.file_name().to_string_lossy().to_lowercase() == name))
            .unwrap_or(false)
    };
    if !exists(p) {
        return p.clone();
    }
    let ext = p.ext();
    let stem = p.stem();
    for n in 2.. {
        let name = if ext.is_empty() { format!("{stem} ({n})") } else { format!("{stem} ({n}).{ext}") };
        let q = match p.parent() {
            Some(d) => VaultPath::parse(&format!("{d}/{name}")),
            None => VaultPath::parse(&name),
        };
        if let Ok(q) = q {
            if !exists(&q) {
                return q;
            }
        }
    }
    unreachable!()
}

/// Install or update the bundled recipes. `installed` maps each path to the fingerprint of the
/// version the app last put there. A file is written only when it is new to this folder, or
/// when it is still exactly what the app installed (the old copy goes to .trash/versions).
/// Anything edited, moved or deleted by the user is left alone. Returns (added, updated).
pub fn seed(root: &Path, files: impl Iterator<Item = (String, Vec<u8>)>, installed: &mut BTreeMap<String, String>) -> Result<(usize, usize)> {
    let (mut added, mut updated) = (0, 0);
    for (rel, bytes) in files {
        let Ok(p) = VaultPath::parse(&rel) else { continue };
        let abs = root.join(p.as_str());
        let new = etag_of(&bytes);
        match installed.get(p.as_str()) {
            None if !abs.exists() => {
                std::fs::create_dir_all(abs.parent().unwrap())?;
                atomic_write(&abs, &bytes)?;
                added += 1;
            }
            None => continue, // the folder already has its own file with this name
            Some(old) if *old == new => continue,
            Some(old) => match std::fs::read(&abs) {
                Ok(cur) if etag_of(&cur) == *old => {
                    backup_before_overwrite(root, &p, &abs)?;
                    atomic_write(&abs, &bytes)?;
                    updated += 1;
                }
                _ => continue, // changed or removed by the user
            },
        }
        installed.insert(p.as_str().to_string(), new);
    }
    Ok((added, updated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_updates_only_untouched_files() {
        let root = std::env::temp_dir().join(format!("seed-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let v1 = |n: &str| (format!("A/{n}.md"), format!("{n} v1").into_bytes());
        let mut installed = BTreeMap::new();
        assert_eq!(seed(&root, ["x", "y", "z"].map(v1).into_iter(), &mut installed).unwrap(), (3, 0));
        std::fs::write(root.join("A/y.md"), "my edit").unwrap(); // user edited y
        std::fs::remove_file(root.join("A/z.md")).unwrap(); // user deleted z
        let v2 = |n: &str| (format!("A/{n}.md"), format!("{n} v2").into_bytes());
        assert_eq!(seed(&root, ["x", "y", "z", "w"].map(v2).into_iter(), &mut installed).unwrap(), (1, 1));
        assert_eq!(std::fs::read_to_string(root.join("A/x.md")).unwrap(), "x v2");
        assert_eq!(std::fs::read_to_string(root.join("A/y.md")).unwrap(), "my edit");
        assert!(!root.join("A/z.md").exists());
        assert!(root.join(".trash/versions/A/x.md").is_dir(), "old x kept");
        // running again changes nothing
        assert_eq!(seed(&root, ["x", "y", "z", "w"].map(v2).into_iter(), &mut installed).unwrap(), (0, 0));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn vault_path_rejects_escapes() {
        for bad in ["", "..", "a/../b", "/../x", ".trash/x", "a/.hidden", "a\\b", "a//b", "a/\u{0}b", " a.md"] {
            assert!(VaultPath::parse(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(VaultPath::parse("/Pies/Pumpkin Pie.md").unwrap().as_str(), "Pies/Pumpkin Pie.md");
        // NFD input normalises to NFC
        assert_eq!(VaultPath::parse("Cre\u{301}pes.md").unwrap().as_str(), "Cr\u{e9}pes.md");
    }

    #[test]
    fn resolve_refuses_symlinks_and_trash_roundtrip() {
        let dir = std::env::temp_dir().join(format!("rv-{}", now_stamp()));
        std::fs::create_dir_all(dir.join("Pies")).unwrap();
        std::fs::write(dir.join("Pies/A.md"), "# A\n").unwrap();
        std::os::unix::fs::symlink("/etc", dir.join("Pies/link")).unwrap();
        assert!(resolve(&dir, &VaultPath::parse("Pies/link/passwd").unwrap(), true).is_err());
        assert!(resolve(&dir, &VaultPath::parse("Pies/link/new.md").unwrap(), false).is_err());
        assert!(resolve(&dir, &VaultPath::parse("Pies/A.md").unwrap(), true).is_ok());

        let p = VaultPath::parse("Pies/A.md").unwrap();
        let id = trash(&dir, &p).unwrap();
        assert!(!dir.join("Pies/A.md").exists());
        let items = list_trash(&dir);
        assert_eq!(items[0].path, "Pies/A.md");
        assert_eq!(restore(&dir, &id).unwrap(), "Pies/A.md");
        assert!(dir.join("Pies/A.md").exists());
        assert_eq!(unique(&dir, &p).as_str(), "Pies/A (2).md");
        assert_eq!(unique(&dir, &VaultPath::parse("Pies/a.MD").unwrap()).as_str(), "Pies/a (2).md");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
