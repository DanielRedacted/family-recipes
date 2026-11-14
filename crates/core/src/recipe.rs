//! Recipe structure detection over a markdown note. Read-only: the file is never re-emitted.
//!
//! Line numbers are 0-based within the full file text (frontmatter included), so the client
//! can map them onto markdown-it's `token.map` after adding the frontmatter line count.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Line {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Group {
    pub name: Option<String>,
    pub items: Vec<Line>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Recipe {
    pub ingredients: Vec<Group>,
    pub steps: Vec<Group>,
}

impl Recipe {
    pub fn ingredient_count(&self) -> usize {
        self.ingredients.iter().map(|g| g.items.len()).sum()
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Parsed {
    /// First H1 before the first H2.
    pub title: Option<String>,
    /// First image before the first H2 (raw src as written).
    pub cover: Option<String>,
    /// Frontmatter as JSON (empty object when absent or invalid).
    pub frontmatter: serde_json::Value,
    pub frontmatter_error: Option<String>,
    /// Number of lines taken by the frontmatter block (0 when absent).
    pub body_line: usize,
    pub recipe: Option<Recipe>,
    /// `[[wiki links]]` targets in order of appearance.
    pub links: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Ing,
    Steps,
}

static QTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\d|[½¼¾⅓⅔⅛⅜⅝⅞]|(a|an|one|two|three|four|six|half|pinch|dash|few)\b)").unwrap()
});
static WIKI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!?\[\[([^\]|#]+)(?:[#|][^\]]*)?\]\]").unwrap());
static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d+[.)])\s+").unwrap());

const ING: &[&str] = &["ingredients", "ingredient", "you will need", "what you need", "you'll need"];
const STEPS: &[&str] = &[
    "directions", "direction", "instructions", "instruction", "method", "preparation", "procedure",
    "procedures", "steps",
];

fn section_kind(heading: &str) -> Option<(Kind, Option<String>)> {
    let t = heading.trim().trim_end_matches(':').trim().to_lowercase();
    let t = Regex::new(r"\s*\(.*\)$").unwrap().replace(&t, "").to_string();
    let original = heading.trim().trim_end_matches(':').trim();
    for (words, kind) in [(ING, Kind::Ing), (STEPS, Kind::Steps)] {
        for w in words {
            if t == *w {
                return Some((kind, None));
            }
            if let Some(prefix) = t.strip_suffix(&format!(" {w}")) {
                // "CAKE INGREDIENTS" -> group "CAKE"
                let name = original[..prefix.len()].trim().to_string();
                return Some((kind, Some(name)));
            }
        }
    }
    if t.starts_with("ingredients") {
        return Some((Kind::Ing, None));
    }
    None
}

/// Split `---\n...\n---\n` frontmatter off the front. Returns (yaml, body_start_byte, body_line).
pub fn split_frontmatter(text: &str) -> Option<(&str, usize, usize)> {
    let rest = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = text.len() - rest.len();
    let mut lines = 1;
    for line in rest.split_inclusive('\n') {
        lines += 1;
        if line.trim_end() == "---" {
            let yaml = &text[text.len() - rest.len()..offset];
            return Some((yaml, offset + line.len(), lines));
        }
        offset += line.len();
    }
    None
}

fn line_of(starts: &[usize], byte: usize) -> usize {
    match starts.binary_search(&byte) {
        Ok(i) => i,
        Err(i) => i - 1,
    }
}

struct Block {
    line: usize,
    text: String,
    kind: BlockKind,
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum BlockKind {
    Heading(u8),
    Para,
    BoldPara,
    Item { ordered: bool, list: usize },
}

pub fn parse(text: &str) -> Parsed {
    let mut out = Parsed { frontmatter: serde_json::json!({}), ..Default::default() };
    let mut body_start = 0;
    if let Some((yaml, start, lines)) = split_frontmatter(text) {
        body_start = start;
        out.body_line = lines;
        if !yaml.trim().is_empty() {
            match serde_saphyr::from_str::<serde_json::Value>(yaml) {
                Ok(v @ serde_json::Value::Object(_)) => out.frontmatter = v,
                Ok(_) => out.frontmatter_error = Some("frontmatter is not a key/value map".into()),
                Err(e) => out.frontmatter_error = Some(e.to_string()),
            }
        }
    }
    let line_starts: Vec<usize> =
        std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
    let body = &text[body_start..];
    out.links = WIKI.captures_iter(body).map(|c| c[1].trim().to_string()).collect();

    // Flatten the markdown into a list of blocks we care about.
    let mut blocks: Vec<Block> = Vec::new();
    let mut list_depth = 0usize;
    let mut list_ids: Vec<(usize, bool)> = Vec::new();
    let mut next_list = 0usize;
    let mut cur: Option<(usize, String, BlockKind, bool)> = None; // (line, text, kind, all_bold)
    let mut in_strong = 0;
    let mut seen_h2 = false;
    let mut para_depth = 0;
    for (ev, range) in Parser::new_ext(body, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH)
        .into_offset_iter()
    {
        let abs = range.start + body_start;
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                let lv = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                if lv == 2 {
                    seen_h2 = true;
                }
                cur = Some((line_of(&line_starts, abs), String::new(), BlockKind::Heading(lv), false));
            }
            Event::Start(Tag::List(start)) => {
                list_depth += 1;
                list_ids.push((next_list, start.is_some()));
                next_list += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth -= 1;
                list_ids.pop();
            }
            Event::Start(Tag::Item) if list_depth == 1 => {
                let (id, ordered) = *list_ids.last().unwrap();
                // Item text = its source lines with the marker stripped (keeps inline markdown as written).
                let src = &text[abs..range.end + body_start];
                let first_block: String = src
                    .lines()
                    .take_while(|l| !l.trim().is_empty())
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ");
                let item = MARKER.replace(&first_block, "").trim().to_string();
                blocks.push(Block {
                    line: line_of(&line_starts, abs),
                    text: item,
                    kind: BlockKind::Item { ordered, list: id },
                });
            }
            Event::Start(Tag::Paragraph) => {
                para_depth += 1;
                if list_depth == 0 {
                    cur = Some((line_of(&line_starts, abs), String::new(), BlockKind::Para, true));
                }
            }
            Event::Start(Tag::Strong) => in_strong += 1,
            Event::End(TagEnd::Strong) => in_strong -= 1,
            Event::Start(Tag::Image { dest_url, .. }) => {
                if !seen_h2 && out.cover.is_none() {
                    out.cover = Some(dest_url.to_string());
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, s, _, all_bold)) = cur.as_mut() {
                    s.push_str(&t);
                    if in_strong == 0 && !t.trim().is_empty() {
                        *all_bold = false;
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, s, _, _)) = cur.as_mut() {
                    s.push('\n');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((line, s, kind, _)) = cur.take() {
                    if kind == BlockKind::Heading(1) && !seen_h2 && out.title.is_none() {
                        out.title = Some(s.trim().to_string());
                    }
                    blocks.push(Block { line, text: s.trim().to_string(), kind });
                }
            }
            Event::End(TagEnd::Paragraph) => {
                para_depth -= 1;
                if list_depth == 0 {
                    if let Some((line, s, _, all_bold)) = cur.take() {
                        let kind = if all_bold && !s.trim().is_empty() { BlockKind::BoldPara } else { BlockKind::Para };
                        // one block per source line: family recipes put one ingredient per line
                        let n = s.lines().count().max(1);
                        for k in 0..n {
                            // raw source line (keeps inline markdown as written)
                            let raw = text.lines().nth(line + k).unwrap_or("").trim();
                            if !raw.is_empty() {
                                let plain = s.lines().nth(k).unwrap_or(raw).trim();
                                let text = if matches!(kind, BlockKind::BoldPara) { plain } else { raw };
                                blocks.push(Block { line: line + k, text: text.to_string(), kind });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let _ = para_depth;
    out.recipe = detect(&blocks);
    out
}

fn push(groups: &mut Vec<Group>, name: Option<String>, line: Line) {
    match groups.last_mut() {
        Some(g) if g.name == name => g.items.push(line),
        _ => groups.push(Group { name, items: vec![line] }),
    }
}

fn is_group_label(b: &Block, next: Option<&Block>) -> bool {
    match b.kind {
        BlockKind::Heading(l) if l >= 3 => true,
        BlockKind::BoldPara => b.text.split_whitespace().count() <= 8,
        // short plain line right before a list ("Custard" above numbered steps)
        BlockKind::Para => {
            b.text.split_whitespace().count() <= 4
                && !b.text.ends_with(['.', ',', ';', '!', '?'])
                && matches!(next, Some(Block { kind: BlockKind::Item { .. }, .. }))
        }
        _ => false,
    }
}

fn detect(blocks: &[Block]) -> Option<Recipe> {
    let mut r = Recipe::default();
    let mut section: Option<Kind> = None;
    let mut group: Option<String> = None;
    let mut saw_heading_section = false;
    // per section: use list items if the section has any, else paragraph lines
    let mut i = 0;
    while i < blocks.len() {
        let b = &blocks[i];
        if let BlockKind::Heading(l) = b.kind {
            if l <= 2 {
                if let Some((k, name)) = section_kind(&b.text) {
                    section = Some(k);
                    group = name;
                    saw_heading_section = true;
                } else {
                    section = None;
                }
                i += 1;
                continue;
            }
        }
        let Some(kind) = section else {
            i += 1;
            continue;
        };
        if is_group_label(b, blocks.get(i + 1)) {
            group = Some(b.text.trim_end_matches(':').trim().to_string());
            i += 1;
            continue;
        }
        // section extent
        let end = blocks[i..]
            .iter()
            .position(|x| matches!(x.kind, BlockKind::Heading(l) if l <= 2))
            .map_or(blocks.len(), |p| i + p);
        let has_items = blocks[i..end].iter().any(|x| matches!(x.kind, BlockKind::Item { .. }));
        let target = if kind == Kind::Ing { &mut r.ingredients } else { &mut r.steps };
        let take = match b.kind {
            BlockKind::Item { .. } => true,
            BlockKind::Para => !has_items,
            _ => false,
        };
        if take {
            push(target, group.clone(), Line { line: b.line, text: b.text.clone() });
        }
        i += 1;
    }
    if !saw_heading_section || r.ingredients.is_empty() {
        // Fallback: first list where at least half the items start with a quantity.
        let mut lists: Vec<(usize, Vec<&Block>)> = Vec::new();
        for b in blocks {
            if let BlockKind::Item { list, .. } = b.kind {
                match lists.last_mut() {
                    Some((id, items)) if *id == list => items.push(b),
                    _ => lists.push((list, vec![b])),
                }
            }
        }
        if let Some((id, items)) = lists.iter().find(|(_, items)| {
            let q = items.iter().filter(|b| QTY.is_match(&b.text)).count();
            items.len() >= 2 && q * 2 >= items.len()
        }) {
            r.ingredients = vec![Group {
                name: None,
                items: items.iter().map(|b| Line { line: b.line, text: b.text.clone() }).collect(),
            }];
            if r.steps.is_empty() {
                let last = items.last().unwrap().line;
                let after: Vec<&Block> = blocks
                    .iter()
                    .filter(|b| b.line > last)
                    .take_while(|b| !matches!(b.kind, BlockKind::Heading(l) if l <= 2))
                    .filter(|b| match b.kind {
                        BlockKind::Item { list, .. } => list != *id,
                        BlockKind::Para => true,
                        _ => false,
                    })
                    .collect();
                let has_items = after.iter().any(|b| matches!(b.kind, BlockKind::Item { .. }));
                let mut g = None;
                for (k, b) in after.iter().enumerate() {
                    if is_group_label(b, after.get(k + 1).copied()) {
                        g = Some(b.text.clone());
                        continue;
                    }
                    if has_items && b.kind == BlockKind::Para {
                        continue;
                    }
                    push(&mut r.steps, g.clone(), Line { line: b.line, text: b.text.clone() });
                }
            }
        }
    }
    (!r.ingredients.is_empty()).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_groups_and_lines() {
        let md = "---\nyield: 8\n---\n\n# Pie\n\n## CAKE INGREDIENTS\n\n- 1 cup flour\n- 2 eggs\n\n### Glaze\n\n- ½ cup sugar\n\n## Directions\n\nCake\n\n1. Mix.\n2. Bake.\n\n## Notes\n\nGood.\n";
        let p = parse(md);
        assert_eq!(p.title.as_deref(), Some("Pie"));
        assert_eq!(p.body_line, 3);
        assert_eq!(p.frontmatter["yield"], 8);
        let r = p.recipe.unwrap();
        assert_eq!(r.ingredients[0].name.as_deref(), Some("CAKE"));
        assert_eq!(r.ingredients[0].items[0], Line { line: 8, text: "1 cup flour".into() });
        assert_eq!(r.ingredients[1].name.as_deref(), Some("Glaze"));
        assert_eq!(r.steps[0].name.as_deref(), Some("Cake"));
        assert_eq!(r.steps[0].items.len(), 2);
        assert_eq!(md.lines().nth(r.steps[0].items[1].line), Some("2. Bake."));
    }

    #[test]
    fn paragraph_lines_and_fallback() {
        let p = parse("# Tri Tip\n\nIngredients\n\n## Ingredients\n\n1 roast\n\nSalt\n\n## Instructions\n\nSear it.\n\nRest it.\n");
        let r = p.recipe.unwrap();
        assert_eq!(r.ingredient_count(), 2);
        assert_eq!(r.steps[0].items.len(), 2);

        let p = parse("# Margaritas\n\n- 2 parts tequila\n- 2 parts lime juice\n\nShake.\n");
        let r = p.recipe.unwrap();
        assert_eq!(r.ingredient_count(), 2);
        assert_eq!(r.steps[0].items[0].text, "Shake.");

        assert!(parse("# Chart\n\n| a | b |\n| - | - |\n| 1 | 2 |\n").recipe.is_none());
    }

    #[test]
    fn bad_frontmatter_does_not_block() {
        let p = parse("---\n: : :\n---\n# T\n");
        assert!(p.frontmatter_error.is_some());
        assert_eq!(p.title.as_deref(), Some("T"));
    }

    #[test]
    fn whole_seed_vault_parses() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../vault-seed");
        let (mut n, mut recipes) = (0, 0);
        for e in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if e.path().extension().is_some_and(|x| x == "md") {
                let text = std::fs::read_to_string(e.path()).unwrap();
                let p = parse(&text);
                assert!(p.frontmatter_error.is_none(), "{}: {:?}", e.path().display(), p.frontmatter_error);
                n += 1;
                if let Some(r) = p.recipe {
                    recipes += 1;
                    for g in r.ingredients.iter().chain(&r.steps) {
                        for it in &g.items {
                            assert!(text.lines().nth(it.line).is_some_and(|l| !l.trim().is_empty()));
                        }
                    }
                }
            }
        }
        assert_eq!(n, 259);
        eprintln!("{recipes}/{n} seed notes detected as recipes");
        assert!(recipes >= 200, "only {recipes} recipes detected");
    }
}
