//! `README.md` at the top of the recipes folder: every recipe, by folder, as links. Written by the app
//! after each rescan so the folder reads well on GitHub, in Finder or any Markdown viewer; hidden in the
//! app (`vault::hidden`). It depends only on the recipes (no dates, no counts), so two Macs syncing the
//! same folder write the same bytes and never commit it back and forth.

use crate::server::urlencode;
use crate::vault::{self, FileKind, Index};
use std::path::Path;

pub const FILE: &str = "README.md";
/// First line of a README the app wrote; one without it is someone's own and is left alone.
const MARKER: &str = "<!-- Written by the Recipes app: every recipe, by folder. Changes made here are replaced. -->";

/// `Breads/Soft Pretzels.md` → `Breads/Soft%20Pretzels.md` (each segment encoded, the slashes kept).
fn href(p: &str) -> String {
    p.split('/').map(urlencode).collect::<Vec<_>>().join("/")
}

/// A title as link text: Markdown punctuation that would change it is escaped.
fn text(t: &str) -> String {
    t.chars().fold(String::new(), |mut s, c| {
        if matches!(c, '\\' | '[' | ']' | '*' | '_' | '`' | '<' | '>') {
            s.push('\\');
        }
        s.push(c);
        s
    })
}

/// The contents, or None when there are no recipes.
pub fn render(idx: &Index) -> Option<String> {
    let attachments = format!("{}/", vault::ATTACHMENTS);
    let mut by_folder: std::collections::BTreeMap<&str, Vec<(String, &str, &str)>> = Default::default();
    for e in idx.files.values().filter(|e| e.kind == FileKind::Note && !e.path.as_str().starts_with(&attachments)) {
        by_folder.entry(e.path.parent().unwrap_or("")).or_default().push((e.title.to_lowercase(), e.path.as_str(), e.title.as_str()));
    }
    if by_folder.is_empty() {
        return None;
    }
    let mut out = format!("{MARKER}\n\n# Recipes\n");
    let list = |out: &mut String, notes: &mut Vec<(String, &str, &str)>| {
        notes.sort();
        out.push('\n');
        for (_, path, title) in notes.iter() {
            out.push_str(&format!("- [{}]({})\n", text(title), href(path)));
        }
    };
    if let Some(mut top) = by_folder.remove("") {
        list(&mut out, &mut top);
    }
    // folders in the folder list's order (sorted by name, each followed by its subfolders)
    for f in &idx.folders {
        if let Some(mut notes) = by_folder.remove(f.as_str()) {
            out.push_str(&format!("\n## {}\n", text(&f.as_str().replace('/', " / "))));
            list(&mut out, &mut notes);
        }
    }
    Some(out)
}

/// Write the contents when they changed. Never fails the rescan: a problem is only logged.
pub fn update(root: &Path, idx: &Index) {
    let Some(new) = render(idx) else { return };
    let abs = root.join(FILE);
    match std::fs::read_to_string(&abs) {
        Ok(cur) if cur == new || !cur.starts_with(MARKER) => return,
        _ => {}
    }
    if let Err(e) = vault::atomic_write(&abs, new.as_bytes()) {
        tracing::warn!("could not write {FILE}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_recipe_by_folder_and_writes_only_on_change() {
        let dir = std::env::temp_dir().join(format!("rv-toc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (p, s) in [
            ("Soups & Chili/White Chili.md", "# White Chili\n"),
            ("Pies/pumpkin.md", "# Pumpkin Pie\n"),
            ("Pies/Apple.md", "no heading\n"),
            ("Breads/Sourdough/Starter.md", "# Starter [basic]\n"),
            ("Notes.md", "# Kitchen notes\n"),
            ("attachments/originals/Pie.md", "# Pie\n"),
            ("AGENTS.md", "# Agents\n"),
        ] {
            std::fs::create_dir_all(dir.join(p).parent().unwrap()).unwrap();
            std::fs::write(dir.join(p), s).unwrap();
        }
        let idx = vault::scan(&dir, None, None);
        let out = render(&idx).unwrap();
        assert!(out.starts_with(MARKER));
        let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s:?} missing from\n{out}"));
        assert!(at("- [Kitchen notes](Notes.md)") < at("## Breads / Sourdough"));
        assert!(at("## Breads / Sourdough") < at("## Pies") && at("## Pies") < at("## Soups & Chili"));
        assert!(at("- [Starter \\[basic\\]](Breads/Sourdough/Starter.md)") > 0);
        assert!(at("- [Apple](Pies/Apple.md)") < at("- [Pumpkin Pie](Pies/pumpkin.md)"));
        assert!(at("(Soups%20%26%20Chili/White%20Chili.md)") > 0);
        assert!(!out.contains("originals") && !out.contains("Agents"));

        // written once, not rewritten when nothing changed, and never shown as a recipe
        update(&dir, &idx);
        let abs = dir.join(FILE);
        let mtime = || std::fs::metadata(&abs).unwrap().modified().unwrap();
        let first = mtime();
        std::thread::sleep(std::time::Duration::from_millis(20));
        update(&dir, &vault::scan(&dir, Some(&idx), None));
        assert_eq!(first, mtime());
        assert!(vault::scan(&dir, None, None).files.keys().all(|p| p.as_str() != FILE));

        // someone's own README is left alone
        std::fs::write(&abs, "# Our recipes\n").unwrap();
        update(&dir, &idx);
        assert_eq!(std::fs::read_to_string(&abs).unwrap(), "# Our recipes\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_is_written_for_an_empty_folder() {
        let dir = std::env::temp_dir().join(format!("rv-toc-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        update(&dir, &vault::scan(&dir, None, None));
        assert!(!dir.join(FILE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
