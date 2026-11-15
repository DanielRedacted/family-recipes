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
/// table cells: only digits and fraction characters count as a quantity
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d|[½¼¾⅓⅔⅛⅜⅝⅞])").unwrap());
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
    /// table rows: the cells as written (empty cells kept)
    cells: Vec<String>,
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum BlockKind {
    Heading(u8),
    Para,
    BoldPara,
    Item { ordered: bool, list: usize },
    /// a table body row (or a header row holding data, e.g. `| ¾ cup | Warm milk |`)
    Row { table: usize },
    /// a table header of labels, e.g. `| Single | Ingredients | Double |`
    TableHead { table: usize },
}

/// A cell that is only an amount ("1 ½ cups", "48oz", "1 tsp scant", "3 cups less 3 Tbsp").
/// Returns whether it names a kitchen unit.
fn amount_cell(c: &str) -> Option<bool> {
    static RX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:about\s+)?[\d½¼¾⅓⅔⅛⅜⅝⅞][\d\s/½¼¾⅓⅔⅛⅜⅝⅞.,–-]*?\s*(cups?|c|tbsps?|tbs|tsps?|t|teaspoons?|tablespoons?|oz|ounces?|lbs?|pounds?|g|grams?|kg|ml|l|qts?|quarts?|pints?|pt|gal|gallons?|drops?|pinch(?:es)?|dash(?:es)?|cloves?|sticks?|cans?|pkgs?|packages?|slices?)?\.?\s*(?:\(.*\)|scant|slight|heaping|less\b.*|,.*|\+.*)?$").unwrap()
    });
    let c = c.trim();
    if c.eq_ignore_ascii_case("pinch") || c.eq_ignore_ascii_case("dash") {
        return Some(true);
    }
    RX.captures(c).map(|m| m.get(1).is_some())
}

/// Is this table an ingredient table? (rows: its body rows, in order)
fn ingredient_table(rows: &[&Block], head_says_ingredients: bool) -> bool {
    if rows.is_empty() {
        return false;
    }
    if head_says_ingredients {
        return true;
    }
    let half = |n: usize| n >= 2 && n * 2 >= rows.len();
    fn filled(b: &Block) -> Vec<&str> {
        b.cells.iter().map(|c| c.as_str()).filter(|c| !c.is_empty() && *c != "-" && *c != "–").collect()
    }
    // `qty | name`
    let qty_name = rows.iter().filter(|b| { let f = filled(b); f.len() == 2 && NUM.is_match(f[0]) && !NUM.is_match(f[1]) }).count();
    if half(qty_name) {
        return true;
    }
    // batch sizes side by side: one name cell, the rest amounts (with kitchen units)
    if rows[0].cells.len() >= 3 {
        let mut ing = 0;
        let mut with_unit = 0;
        for b in rows {
            let f = filled(b);
            let amounts: Vec<bool> = f.iter().filter_map(|c| amount_cell(c)).collect();
            // the one other cell is the name, and names don't start with a number
            let names = f.iter().filter(|c| amount_cell(c).is_none()).count();
            let numeric_name = f.iter().any(|c| amount_cell(c).is_none() && NUM.is_match(c));
            if names == 1 && !numeric_name && !amounts.is_empty() {
                ing += 1;
                with_unit += amounts.iter().any(|u| *u) as usize;
            }
        }
        if half(ing) && with_unit * 2 >= ing {
            return true;
        }
    }
    // variants side by side: every cell is a whole ingredient line ("¾ cup water | ¼ cup water")
    let variants = rows.iter().filter(|b| { let f = filled(b); f.len() >= 2 && f.iter().all(|c| NUM.is_match(c) && amount_cell(c).is_none()) }).count();
    half(variants)
}

/// A group label's name, unless it is just a section word ("Ingredients:", "Directions").
fn label_name(text: &str) -> Option<String> {
    let t = text.trim().trim_matches(['*', '_']).trim().trim_end_matches(':').trim().trim_matches(['*', '_']).trim();
    (!t.is_empty() && section_kind(t).is_none()).then(|| t.to_string())
}

/// One table row as an ingredient line: `qty | name` reads "qty name"; other layouts keep
/// every cell, in order, separated by " · " (multi-batch tables are shown, not scaled).
fn row_text(cells: &[String]) -> String {
    let cells: Vec<&str> = cells.iter().map(|c| c.trim()).filter(|c| !c.is_empty() && *c != "-" && *c != "–").collect();
    if cells.len() == 2 && NUM.is_match(cells[0]) && !NUM.is_match(cells[1]) {
        format!("{} {}", cells[0], cells[1])
    } else {
        cells.join(" · ")
    }
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
    // tables: (first line, cells, header?)
    let mut row: Option<(usize, Vec<String>, bool)> = None;
    let mut table_id = 0usize;
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
                    cells: vec![],
                });
            }
            Event::Start(Tag::Paragraph) => {
                para_depth += 1;
                if list_depth == 0 {
                    cur = Some((line_of(&line_starts, abs), String::new(), BlockKind::Para, true));
                }
            }
            Event::Start(Tag::Table(_)) => table_id += 1,
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) if list_depth == 0 => {
                let header = matches!(ev, Event::Start(Tag::TableHead));
                row = Some((line_of(&line_starts, abs), vec![], header));
            }
            Event::Start(Tag::TableCell) => {
                if let Some((_, cells, _)) = row.as_mut() {
                    cells.push(String::new());
                }
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                if let Some((line, cells, header)) = row.take() {
                    let label = header && !cells.iter().any(|c| NUM.is_match(c.trim()));
                    let kind = if label { BlockKind::TableHead { table: table_id } } else { BlockKind::Row { table: table_id } };
                    let text = if label { cells.iter().map(|c| c.trim()).collect::<Vec<_>>().join(" | ") } else { row_text(&cells) };
                    if !text.is_empty() {
                        blocks.push(Block { line, text, kind, cells: cells.iter().map(|c| c.trim().to_string()).collect() });
                    }
                }
            }
            Event::Start(Tag::Strong) => in_strong += 1,
            Event::End(TagEnd::Strong) => in_strong -= 1,
            Event::Start(Tag::Image { dest_url, .. }) => {
                if !seen_h2 && out.cover.is_none() {
                    out.cover = Some(dest_url.to_string());
                }
            }
            Event::Text(t) | Event::Code(t) if row.is_some() => {
                if let Some(c) = row.as_mut().and_then(|r| r.1.last_mut()) {
                    c.push_str(&t);
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
                    blocks.push(Block { line, text: s.trim().to_string(), kind, cells: vec![] });
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
                                blocks.push(Block { line: line + k, text: text.to_string(), kind, cells: vec![] });
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
                && matches!(next, Some(Block { kind: BlockKind::Item { .. } | BlockKind::Row { .. }, .. }))
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
            group = label_name(&b.text);
            i += 1;
            continue;
        }
        // section extent
        let end = blocks[i..]
            .iter()
            .position(|x| matches!(x.kind, BlockKind::Heading(l) if l <= 2))
            .map_or(blocks.len(), |p| i + p);
        let has_items = blocks[i..end].iter().any(|x| matches!(x.kind, BlockKind::Item { .. } | BlockKind::Row { .. }) && (kind == Kind::Ing || matches!(x.kind, BlockKind::Item { .. })));
        let target = if kind == Kind::Ing { &mut r.ingredients } else { &mut r.steps };
        let take = match b.kind {
            BlockKind::Item { .. } => true,
            BlockKind::Row { .. } => kind == Kind::Ing,
            BlockKind::Para => !has_items,
            _ => false,
        };
        if take {
            push(target, group.clone(), Line { line: b.line, text: b.text.clone() });
        }
        i += 1;
    }
    if !saw_heading_section || r.ingredients.is_empty() {
        // Fallback: the first list where at least half the items start with a quantity, or the
        // first ingredient table (see `ingredient_table`); more "label + list" pairs right after
        // it are further ingredient groups ("**Topping**" list, "**Cake**" list).
        #[derive(Clone, Copy, PartialEq)]
        enum Src {
            List(usize),
            Table(usize),
        }
        let mut cands: Vec<(Src, Vec<&Block>)> = Vec::new();
        for b in blocks {
            let src = match b.kind {
                BlockKind::Item { list, .. } => Src::List(list),
                BlockKind::Row { table } => Src::Table(table),
                _ => continue,
            };
            match cands.last_mut() {
                Some((s, items)) if *s == src => items.push(b),
                _ => cands.push((src, vec![b])),
            }
        }
        let head_says_ingredients = |t: usize| blocks.iter().any(|b| b.kind == BlockKind::TableHead { table: t } && b.text.to_lowercase().contains("ingredient"));
        let qualifies = |src: &Src, items: &Vec<&Block>| match src {
            Src::List(_) => {
                let q = items.iter().filter(|b| QTY.is_match(&b.text)).count();
                items.len() >= 2 && q * 2 >= items.len()
            }
            Src::Table(t) => ingredient_table(items, head_says_ingredients(*t)),
        };
        let index_of = |b: &Block| blocks.iter().position(|x| std::ptr::eq(x, b)).unwrap();
        if let Some(first) = cands.iter().position(|(s, it)| qualifies(s, it)) {
            let label_before = |b: &Block| {
                let k = index_of(b);
                (k > 0 && is_group_label(&blocks[k - 1], Some(b))).then(|| label_name(&blocks[k - 1].text)).flatten()
            };
            let (src, items) = &cands[first];
            let mut groups = vec![Group { name: label_before(items[0]), items: items.iter().map(|b| Line { line: b.line, text: b.text.clone() }).collect() }];
            let mut used = vec![*src];
            let mut last = items.last().unwrap().line;
            if matches!(src, Src::List(_)) {
                for (s2, it2) in &cands[first + 1..] {
                    let k = index_of(it2[0]);
                    let prev_is_label = k > 0 && blocks[k - 1].line > last && is_group_label(&blocks[k - 1], Some(it2[0]));
                    let nothing_between = k >= 2 && blocks[k - 2].line <= last;
                    if !(matches!(s2, Src::List(_)) && prev_is_label && nothing_between && qualifies(s2, it2)) {
                        break;
                    }
                    groups.push(Group { name: label_name(&blocks[k - 1].text), items: it2.iter().map(|b| Line { line: b.line, text: b.text.clone() }).collect() });
                    used.push(*s2);
                    last = it2.last().unwrap().line;
                }
            }
            r.ingredients = groups;
            if r.steps.is_empty() {
                let after: Vec<&Block> = blocks
                    .iter()
                    .filter(|b| b.line > last)
                    .take_while(|b| !matches!(b.kind, BlockKind::Heading(l) if l <= 2))
                    .filter(|b| match b.kind {
                        BlockKind::Item { list, .. } => !used.contains(&Src::List(list)),
                        BlockKind::Para | BlockKind::BoldPara => true,
                        _ => false,
                    })
                    .collect();
                let has_items = after.iter().any(|b| matches!(b.kind, BlockKind::Item { .. }));
                let mut g = None;
                for (k, b) in after.iter().enumerate() {
                    if is_group_label(b, after.get(k + 1).copied()) {
                        g = label_name(&b.text);
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
    fn ingredient_tables() {
        // a qty | name table under a heading; the header row holds data
        let md = "# Bread\n\n## Ingredients\n\n| ¾ cup | Warm milk |\n| --- | --- |\n| 2 tsp | Yeast |\n| | Salt |\n\n## Directions\n\nMix.\n";
        let r = parse(md).recipe.unwrap();
        let items: Vec<&str> = r.ingredients[0].items.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(items, ["¾ cup Warm milk", "2 tsp Yeast", "Salt"]);
        assert_eq!(md.lines().nth(r.ingredients[0].items[1].line), Some("| 2 tsp | Yeast |"));
        assert_eq!(r.steps[0].items[0].text, "Mix.");
        // a multi-batch table with no headings: found by its "Ingredients" column
        let md = "# 9” Pie Crust\n\n| Single | Ingredients | Double |\n| --- | --- | --- |\n| 1 ½ cups | AP flour | 3 cups |\n| 1 tsp | Salt | 1 1/2 tsp |\n\nMix flour and salt.\n";
        let r = parse(md).recipe.unwrap();
        assert_eq!(r.ingredients[0].items[0].text, "1 ½ cups · AP flour · 3 cups");
        assert_eq!(r.steps[0].items[0].text, "Mix flour and salt.");
        // batch sizes side by side, no headings at all
        let md = "# Eggnog\n\n| 8 | Eggs | 12 |\n| --- | --- | --- |\n| 2 cups | Cream | 3 cups |\n| ¾ cup | Sugar | 1 1/8 cup |\n| pinch | salt | 1/16 tsp |\n\nBlend.\n";
        let r = parse(md).recipe.unwrap();
        assert_eq!(r.ingredient_count(), 4);
        assert_eq!(r.steps[0].items[0].text, "Blend.");
        // charts are not recipes
        assert!(parse("# Sous Vide\n\n|  | Temperature | Time |\n| --- | --- | --- |\n| Pork | 135 | |\n| Fish | 134 | |\n| Tri tip | 132 | 24 hours |\n").recipe.is_none());
        assert!(parse("# BBQ\n\n| Cut | Time | Temp |\n| --- | --- | --- |\n| Pork Butt | 1 - 1.5 hours per lb | 195 deg F |\n| Ribs | 5 - 6 hours | 190 deg F |\n").recipe.is_none());
    }

    #[test]
    fn labelled_lists_without_headings() {
        let md = "# Coffee Cake\n\n**Topping**\n\n- 2 T butter\n- ½ tsp cinnamon\n\n**Cake**\n\n- 1 cup flour\n- 1 cup water\n\n**Directions**\n\nMix the topping.\n\nBake.\n";
        let r = parse(md).recipe.unwrap();
        let groups: Vec<_> = r.ingredients.iter().map(|g| (g.name.as_deref(), g.items.len())).collect();
        assert_eq!(groups, [(Some("Topping"), 2), (Some("Cake"), 2)]);
        assert_eq!(r.steps, vec![Group { name: None, items: vec![Line { line: 14, text: "Mix the topping.".into() }, Line { line: 16, text: "Bake.".into() }] }]);
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
        let techniques: Vec<String> = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/techniques.txt"))
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let (mut missed, mut charts) = (vec![], vec![]);
        for e in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if e.path().extension().is_some_and(|x| x == "md") {
                let text = std::fs::read_to_string(e.path()).unwrap();
                let p = parse(&text);
                assert!(p.frontmatter_error.is_none(), "{}: {:?}", e.path().display(), p.frontmatter_error);
                n += 1;
                let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
                let reference = rel.starts_with("Reference/");
                match (&p.recipe, reference || techniques.contains(&rel)) {
                    (None, false) => missed.push(rel.clone()),
                    (Some(_), true) if reference => charts.push(rel.clone()),
                    _ => {}
                }
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
        assert_eq!(n, 257); // 259 originals, two exact duplicates left out (tools/names.tsv)
        eprintln!("{recipes}/{n} seed notes detected as recipes");
        assert!(missed.is_empty(), "not detected as recipes: {missed:?}");
        assert!(charts.is_empty(), "reference pages read as recipes: {charts:?}");
    }
}
