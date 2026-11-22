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
    /// In a table with alternative portions: this ingredient's full line for each portion
    /// ("" where a portion doesn't use it). `text` is the first non-empty one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alts: Vec<String>,
    /// What the line asks for, for nutrition: one per portion (`alts`), else one; `None` for a line
    /// that names no ingredient. Filled in when the vault is read (`ingredient::annotate`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub amounts: Vec<Option<crate::ingredient::Amount>>,
}

impl Line {
    pub fn new(line: usize, text: impl Into<String>) -> Self {
        Line { line, text: text.into(), alts: vec![], amounts: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Group {
    pub name: Option<String>,
    pub items: Vec<Line>,
    /// Portion labels of an ingredient table with alternative portions ("Single", "Double"; "" when
    /// the table has no labels). Empty for lists.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub portions: Vec<String>,
    /// First and last line of an ingredient table (the reader shows it as a scaled list).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<[usize; 2]>,
}

impl Group {
    fn new(name: Option<String>, items: Vec<Line>) -> Self {
        Group { name, items, portions: vec![], table: None }
    }
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
static WRAPPER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d+[.)])[ \t]+[-*+][ \t]").unwrap());
static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:(?:[-*+]|\d+[.)])\s+)+").unwrap());
/// a quantity followed by a unit or a counted thing ("2 cups", "1 egg", "3 medium zucchini"): stricter
/// than `QTY`, for paragraph lines and steps sections, where "2 minutes later" is not an ingredient
static QTY_UNIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\**(?:about\s+)?[\d½¼¾⅓⅔⅛⅜⅝⅞][\d\s/½¼¾⅓⅔⅛⅜⅝⅞.,⁄]*(?:\s*(?:-|–|to|or)\s*[\d½¼¾⅓⅔⅛⅜⅝⅞][\d\s/½¼¾⅓⅔⅛⅜⅝⅞.,⁄]*)?\s*(?:\([^)]*\)\s*)?(?:c|cups?|tbsps?\.?|tbs\.?|tbl\.?|tb\.?|t\.?|tsps?\.?|ts\.?|teaspoons?|tablespoons?|oz\.?|ounces?|lbs?\.?|pounds?|g|grams?|kg|ml|l|liters?|qts?\.?|quarts?|pints?|pt|gal|gallons?|drops?|pinch(?:es)?|dash(?:es)?|cloves?|sticks?|cans?|jars?|pkgs?\.?|packages?|packets?|envelopes?|slices?|pieces?|bunch(?:es)?|sprigs?|stalks?|heads?|box(?:es)?|bags?|handfuls?|scoops?|eggs?|yolks?|whites?|large|medium|small|whole|fillets?|filets?|bananas?|onions?|carrots?|potatoes?)\b").unwrap()
});

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
    // `qty | name`, or `name | amount with a unit` (a chart's "Pork | 135" is not an ingredient)
    let qty_name = rows.iter().filter(|b| { let f = filled(b); f.len() == 2 && ((NUM.is_match(f[0]) && !NUM.is_match(f[1])) || (!NUM.is_match(f[0]) && amount_cell(f[1]) == Some(true))) }).count();
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
    let t = text.replace("**", "").replace("__", "");
    let t = t.trim().trim_matches(['*', '_']).trim().trim_end_matches(':').trim().trim_matches(['*', '_']).trim();
    (t.chars().any(char::is_alphanumeric) && section_kind(t).is_none()).then(|| t.to_string())
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
    // "- + 3 cups…": a list item holding only a nested list; its nested items are the list's items
    let mut wrapper: Option<(usize, bool)> = None;
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
                if list_depth == 0 {
                    wrapper = None;
                }
            }
            Event::Start(Tag::Item) if list_depth == 1 || (list_depth == 2 && wrapper.is_some()) => {
                let (id, ordered) = if list_depth == 1 { *list_ids.last().unwrap() } else { wrapper.unwrap() };
                if list_depth == 1 && WRAPPER.is_match(&text[abs..range.end + body_start]) {
                    wrapper = Some((id, ordered));
                    continue;
                }
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
                    // a header of labels ("Single | Ingredients | Double"), unless it holds amounts ("8 | Eggs | 12")
                    let label = header && !cells.iter().any(|c| amount_cell(unbold(c)).is_some());
                    let kind = if label { BlockKind::TableHead { table: table_id } } else { BlockKind::Row { table: table_id } };
                    // a row's ingredient text is set from the table's layout once the table is known
                    let text = cells.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect::<Vec<_>>().join(if label { " | " } else { " " });
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

/// Remove a whole-cell `**bold**` / `__bold__` wrapper.
fn unbold(c: &str) -> &str {
    let t = c.trim();
    for m in ["**", "__"] {
        if let Some(x) = t.strip_prefix(m).and_then(|x| x.strip_suffix(m)) {
            return x.trim();
        }
    }
    t
}

/// A cell holding something: not empty and not a "-" placeholder.
fn cell_at(b: &Block, c: usize) -> Option<&str> {
    b.cells.get(c).map(|s| unbold(s)).filter(|s| !s.is_empty() && *s != "-" && *s != "–")
}

/// Columns of an ingredient table: the ingredient-name column (None when every cell is a whole
/// line, e.g. "2 cups flour | 4 cups flour"), and the portion columns in order.
fn table_layout(head: Option<&Block>, rows: &[&Block]) -> (Option<usize>, Vec<usize>) {
    let ncol = rows.iter().copied().chain(head).map(|b| b.cells.len()).max().unwrap_or(0);
    let named = head.and_then(|h| h.cells.iter().position(|c| unbold(c).to_lowercase().starts_with("ingredient")));
    let name = named.or_else(|| {
        // the column whose cells are mostly names: neither an amount nor starting with a number
        let score = |c: usize| {
            let filled = rows.iter().filter_map(|b| cell_at(b, c)).collect::<Vec<_>>();
            let names = filled.iter().filter(|s| amount_cell(s).is_none() && !NUM.is_match(s)).count();
            (names * 2 > filled.len()).then_some(names)
        };
        (0..ncol).filter_map(|c| score(c).map(|n| (n, c))).max_by_key(|&(n, c)| (n, std::cmp::Reverse(c))).map(|x| x.1)
    });
    (name, (0..ncol).filter(|&c| Some(c) != name).collect())
}

/// One table row as a full ingredient line per portion ("" where that portion doesn't use it).
fn row_lines(b: &Block, name_col: Option<usize>, portions: &[usize]) -> Vec<String> {
    let name = name_col.and_then(|c| cell_at(b, c));
    let cells: Vec<Option<&str>> = portions.iter().map(|&c| cell_at(b, c)).collect();
    if cells.iter().all(Option::is_none) {
        // "One crust pie shell pastry", "Optional": the same in every portion
        return vec![name.unwrap_or_default().to_string(); portions.len()];
    }
    cells
        .iter()
        .map(|c| match (c, name) {
            (None, _) => String::new(),
            (Some(a), None) => a.to_string(),
            // an amount ("1 ½ cups", "13 to 14 lbs.") goes before the name; a whole line
            // ("¾ cup packed brown sugar") stands alone; other notes go after it
            (Some(a), Some(n)) if amount_cell(a).is_some() || (NUM.is_match(a) && a.split_whitespace().count() <= 4) => format!("{a} {n}"),
            (Some(a), Some(_)) if NUM.is_match(a) => a.to_string(),
            (Some(a), Some(n)) => format!("{n} – {a}"),
        })
        .collect()
}

/// Rewrite the ingredient lines that come from tables: one clean line per row, and for tables with
/// alternative portions every portion's line plus the portion labels on the group.
fn table_groups(r: &mut Recipe, blocks: &[Block]) {
    let by_line: std::collections::HashMap<usize, &Block> = blocks.iter().map(|b| (b.line, b)).collect();
    for g in &mut r.ingredients {
        let Some(BlockKind::Row { table }) = g.items.first().and_then(|it| by_line.get(&it.line)).map(|b| b.kind) else { continue };
        let rows: Vec<&Block> = g.items.iter().filter_map(|it| by_line.get(&it.line).copied()).collect();
        if rows.len() != g.items.len() || rows.iter().any(|b| b.kind != BlockKind::Row { table }) {
            continue;
        }
        let head = blocks.iter().find(|b| b.kind == BlockKind::TableHead { table });
        let (name, portions) = table_layout(head, &rows);
        for (it, b) in g.items.iter_mut().zip(&rows) {
            let lines = row_lines(b, name, &portions);
            it.text = lines.iter().find(|l| !l.is_empty()).cloned().unwrap_or_default();
            if portions.len() >= 2 && rows.len() >= 2 {
                it.alts = lines;
            }
        }
        // alternative portions: two or more amount columns over two or more rows
        if portions.len() >= 2 && rows.len() >= 2 {
            g.portions = portions.iter().map(|&c| head.and_then(|h| h.cells.get(c)).map(|s| unbold(s).to_string()).unwrap_or_default()).collect();
        }
        g.table = Some([head.map_or(rows[0].line, |h| h.line), rows.last().unwrap().line]);
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Region {
    /// before any Ingredients / Directions heading, or under another heading ("## Sauce", "## Notes")
    Free,
    Ing,
    Steps,
}

/// A short line that names what follows ("**Topping**", "### Glaze", "Dumplings:", "Add:").
fn is_label(b: &Block) -> bool {
    match b.kind {
        BlockKind::Heading(l) => l >= 3,
        BlockKind::BoldPara => b.text.split_whitespace().count() <= 8,
        // an italic line ("*35/30/35 Recipe*"), unless it is an ingredient ("*¼ cup tahini*")
        BlockKind::Para if b.text.len() > 2 && ((b.text.starts_with('*') && b.text.ends_with('*')) || (b.text.starts_with('_') && b.text.ends_with('_'))) => {
            static COUNTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\d½¼¾⅓⅔⅛⅜⅝⅞][\d/½¼¾⅓⅔⅛⅜⅝⅞ .–-]*\s+[a-z]").unwrap());
            b.text.split_whitespace().count() <= 8 && !QTY_UNIT.is_match(lead(&b.text)) && !COUNTED.is_match(lead(&b.text))
        }
        BlockKind::Para => {
            b.text.split_whitespace().count() <= 4 && !b.text.ends_with(['.', ',', ';', '!', '?']) && !QTY.is_match(lead(&b.text))
        }
        _ => false,
    }
}

/// A quantity line ("1 cup flour"), or a short bare name ("Ice", "Salt") — not a sentence.
fn ingredient_like(t: &str) -> bool {
    let words = t.split_whitespace().count();
    QTY_UNIT.is_match(lead(t)) || (QTY.is_match(lead(t)) && words <= 10) || (words <= 5 && !t.trim_end().ends_with(['.', '!', '?', ':']))
}

/// The text after leading markup and brackets ("*¼ cup tahini*", "(1 tsp. rice syrup)").
fn lead(t: &str) -> &str {
    t.trim_start_matches(['*', '_', '(', ' '])
}

/// "Add:", "Then add:", "Blend into vegetables:" — an instruction, which stays a step; other labels
/// ("**Pizza Sauce**", "Dumplings:", "Optional:") name the ingredients that follow.
fn is_instruction(b: &Block) -> bool {
    const VERBS: &[&str] = &[
        "add", "mix", "combine", "blend", "stir", "feed", "cook", "toss", "whisk", "beat", "sort", "sauté", "saute", "then",
        "after", "melt", "heat", "boil", "bring", "cut", "chop", "pour", "place", "put", "fold", "sift", "dissolve", "whip",
        "layer", "sprinkle", "spread", "until", "meanwhile", "in", "on", "with", "and", "or",
    ];
    matches!(b.kind, BlockKind::Para)
        && b.text.trim_matches(['*', '_', ' ']).split(|c: char| !c.is_alphanumeric()).next().is_some_and(|w| VERBS.contains(&w.to_lowercase().as_str()))
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Src {
    List(usize, bool),
    Table(usize),
    Paras,
}

fn detect(blocks: &[Block]) -> Option<Recipe> {
    // heading sections: "## Ingredients" (group name from "## CAKE INGREDIENTS"), "## Directions", others
    let mut region = Vec::with_capacity(blocks.len());
    let mut sec_name: Vec<Option<String>> = Vec::with_capacity(blocks.len());
    let (mut cur, mut cur_name) = (Region::Free, None);
    for b in blocks {
        if let BlockKind::Heading(l) = b.kind {
            if l <= 2 {
                match section_kind(&b.text) {
                    Some((k, name)) => {
                        cur = if k == Kind::Ing { Region::Ing } else { Region::Steps };
                        cur_name = name;
                    }
                    None => (cur, cur_name) = (Region::Free, None),
                }
            }
        }
        region.push(cur);
        sec_name.push(cur_name.clone());
    }
    let heading = |i: usize| matches!(blocks[i].kind, BlockKind::Heading(l) if l <= 2);
    let has_ing_section = region.contains(&Region::Ing);

    // groups as (first block, group), sorted into document order at the end
    let mut ing: Vec<(usize, Group)> = Vec::new();
    let mut used = vec![false; blocks.len()]; // ingredient lines and the labels that name them

    // 1. Ingredients sections: every list item / table row (paragraph lines when there is no list)
    {
        let mut group: Option<String> = None;
        let mut last_src: Option<Option<usize>> = None;
        let mut i = 0;
        while i < blocks.len() {
            let b = &blocks[i];
            if region[i] != Region::Ing {
                i += 1;
                continue;
            }
            if heading(i) {
                group = sec_name[i].clone();
                last_src = None;
                i += 1;
                continue;
            }
            if is_group_label(b, blocks.get(i + 1)) {
                group = label_name(&b.text);
                last_src = None;
                i += 1;
                continue;
            }
            let end = (i..blocks.len()).find(|&k| heading(k)).unwrap_or(blocks.len());
            let has_items = blocks[i..end].iter().any(|x| matches!(x.kind, BlockKind::Item { .. } | BlockKind::Row { .. }));
            let take = match b.kind {
                BlockKind::Item { .. } | BlockKind::Row { .. } => true,
                BlockKind::Para => !has_items,
                _ => false,
            };
            if take {
                // a table's rows are a group of their own (it may carry portions)
                let src = match b.kind { BlockKind::Row { table } => Some(table), _ => None };
                match ing.last_mut() {
                    Some((_, g)) if g.name == group && last_src == Some(src) => g.items.push(Line::new(b.line, b.text.clone())),
                    _ => ing.push((i, Group::new(group.clone(), vec![Line::new(b.line, b.text.clone())]))),
                }
                last_src = Some(src);
                used[i] = true;
            }
            i += 1;
        }
    }

    // 2. Ingredient sets anywhere else: lists, tables and runs of paragraph lines that read as
    //    ingredients ("**Pizza Sauce**" + list, "Add:" + list, "### Turmeric Rice" under Directions…)
    let mut sets: Vec<(Src, Vec<usize>)> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        if region[i] == Region::Ing {
            continue;
        }
        let src = match b.kind {
            BlockKind::Item { list, ordered } => Src::List(list, ordered),
            BlockKind::Row { table } => Src::Table(table),
            // a paragraph line joins a run when it reads like an ingredient; sentences end the run
            BlockKind::Para if !is_label(b) && ingredient_like(&b.text) => Src::Paras,
            _ => continue,
        };
        match sets.last_mut() {
            Some((s, idx)) if *s == src && *idx.last().unwrap() + 1 == i && region[i] == region[idx[0]] => idx.push(i),
            _ => sets.push((src, vec![i])),
        }
    }
    let head_says_ingredients = |t: usize| blocks.iter().any(|b| b.kind == BlockKind::TableHead { table: t } && b.text.to_lowercase().contains("ingredient"));
    let mut first_found = !ing.is_empty();
    for (src, idx) in &sets {
        let items: Vec<&Block> = idx.iter().map(|&i| &blocks[i]).collect();
        // the line before names or introduces the set ("**Sauce**", "Add:", "…but substitute:")
        let label = idx[0].checked_sub(1).filter(|&k| {
            let b = &blocks[k];
            (is_label(b) || (b.kind == BlockKind::Para && b.text.trim_end().ends_with(':'))) && !used[k] && region[k] == region[idx[0]]
        });
        let n = items.len();
        let strict = items.iter().filter(|b| QTY_UNIT.is_match(lead(&b.text))).count();
        let loose = items.iter().filter(|b| QTY.is_match(lead(&b.text))).count();
        let steps = region[idx[0]] == Region::Steps;
        let ok = match src {
            // numbered lists are steps, unless the recipe has no other ingredient list (as before)
            Src::List(_, true) => !steps && !first_found && !has_ing_section && n >= 2 && loose * 2 >= n,
            Src::List(..) if steps => label.is_some() && strict * 2 >= n && strict >= 1,
            Src::List(..) => (n >= 2 && loose * 2 >= n) || (label.is_some() && strict == n),
            Src::Table(t) => !steps && ingredient_table(&items, head_says_ingredients(*t)),
            Src::Paras if steps => label.is_some() && strict * 2 >= n && strict >= 1,
            Src::Paras => (n >= 2 && strict * 2 >= n && strict >= 2) || (label.is_some() && strict == n),
        };
        if !ok {
            continue;
        }
        first_found = true;
        let mut name = None;
        if let Some(k) = label {
            if is_label(&blocks[k]) && !is_instruction(&blocks[k]) {
                name = label_name(&blocks[k].text);
                used[k] = true;
            }
        }
        for &i in idx {
            used[i] = true;
        }
        ing.push((idx[0], Group::new(name, items.iter().map(|b| Line::new(b.line, b.text.clone())).collect())));
    }
    if ing.is_empty() {
        return None;
    }
    ing.sort_by_key(|x| x.0);
    let mut r = Recipe { ingredients: ing.into_iter().map(|x| x.1).collect(), steps: vec![] };
    table_groups(&mut r, blocks);

    // 3. Steps: Directions sections (list items, or paragraph lines when there is no list)
    let mut group: Option<String> = None;
    for i in 0..blocks.len() {
        if region[i] != Region::Steps || used[i] {
            continue;
        }
        let b = &blocks[i];
        if heading(i) {
            group = None;
            continue;
        }
        if is_group_label(b, blocks.get(i + 1)) {
            group = label_name(&b.text);
            continue;
        }
        let end = (i..blocks.len()).find(|&k| heading(k)).unwrap_or(blocks.len());
        let has_items = (i..end).any(|k| !used[k] && matches!(blocks[k].kind, BlockKind::Item { .. }));
        let take = match b.kind {
            BlockKind::Item { .. } => true,
            BlockKind::Para => !has_items,
            _ => false,
        };
        if take {
            push(&mut r.steps, group.clone(), Line::new(b.line, b.text.clone()));
        }
    }
    // no Directions heading: what follows the first ingredients, up to the next main heading
    if r.steps.is_empty() {
        let mut first = blocks.iter().position(|b| b.line == r.ingredients[0].items[0].line).unwrap_or(0) + 1;
        // the first set's own instruction ("Sauté and cool:") is the first step
        if first >= 2 && !used[first - 2] && is_label(&blocks[first - 2]) && is_instruction(&blocks[first - 2]) {
            first -= 2;
        }
        let after: Vec<usize> = (first..blocks.len())
            // a main heading ends the steps once no ingredients follow it ("## Notes")
            .take_while(|&k| !(heading(k) && (k..blocks.len()).all(|j| !used[j])))
            .filter(|&k| !used[k] && region[k] != Region::Ing && matches!(blocks[k].kind, BlockKind::Item { .. } | BlockKind::Para | BlockKind::BoldPara))
            .collect();
        let has_items = after.iter().any(|&k| matches!(blocks[k].kind, BlockKind::Item { .. }));
        let mut g = None;
        for (j, &k) in after.iter().enumerate() {
            let b = &blocks[k];
            if is_group_label(b, after.get(j + 1).map(|&x| &blocks[x])) {
                g = label_name(&b.text);
                continue;
            }
            if has_items && b.kind == BlockKind::Para {
                continue;
            }
            push(&mut r.steps, g.clone(), Line::new(b.line, b.text.clone()));
        }
    }
    Some(r)
}

fn push(groups: &mut Vec<Group>, name: Option<String>, line: Line) {
    match groups.last_mut() {
        Some(g) if g.name == name => g.items.push(line),
        _ => groups.push(Group::new(name, vec![line])),
    }
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
        assert_eq!(r.ingredients[0].items[0], Line::new(8, "1 cup flour"));
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
        let g = &r.ingredients[0];
        assert_eq!(g.portions, ["Single", "Double"]);
        assert_eq!(g.table, Some([2, 5]));
        assert_eq!(g.items[0].text, "1 ½ cups AP flour");
        assert_eq!(g.items[1].alts, ["1 tsp Salt", "1 1/2 tsp Salt"]);
        assert_eq!(r.steps[0].items[0].text, "Mix flour and salt.");
        // batch sizes side by side, no headings at all
        let md = "# Eggnog\n\n| 8 | Eggs | 12 |\n| --- | --- | --- |\n| 2 cups | Cream | 3 cups |\n| ¾ cup | Sugar | 1 1/8 cup |\n| pinch | salt | 1/16 tsp |\n\nBlend.\n";
        let r = parse(md).recipe.unwrap();
        assert_eq!(r.ingredient_count(), 4);
        assert_eq!(r.ingredients[0].portions, ["", ""], "a header holding amounts is the first row");
        assert_eq!(r.ingredients[0].items[0].alts, ["8 Eggs", "12 Eggs"]);
        assert_eq!(r.ingredients[0].items[3].alts, ["pinch salt", "1/16 tsp salt"]);
        assert_eq!(r.steps[0].items[0].text, "Blend.");
        // amounts first, name last (the standard layout), a "-" for a portion that skips it
        let md = "# Ice Cream\n\n| VANILLA | CHOCOLATE |  |\n| --- | --- | --- |\n| 3 | 3 | egg yolks |\n| ½ cup | 1 cup | Sugar |\n| - | 2 oz. | Unsweetened baking chocolate |\n\n1. Mix.\n";
        let g = &parse(md).recipe.unwrap().ingredients[0];
        assert_eq!(g.portions, ["VANILLA", "CHOCOLATE"]);
        assert_eq!(g.items[2].alts, ["", "2 oz. Unsweetened baking chocolate"]);
        assert_eq!(g.items[2].text, "2 oz. Unsweetened baking chocolate");
        // name first; bold labels; a label header ("9” Quiche" is not an amount)
        let md = "# Q\n\n## Ingredients\n\n| 9” Quiche |  | 10” Quiche |\n| --- | --- | --- |\n|  | One crust pie shell |  |\n| 4 | Large eggs | 6 |\n";
        let g = &parse(md).recipe.unwrap().ingredients[0];
        assert_eq!(g.portions, ["9” Quiche", "10” Quiche"]);
        assert_eq!(g.items.len(), 2);
        assert_eq!(g.items[0].alts, ["One crust pie shell", "One crust pie shell"]);
        assert_eq!(g.items[1].alts, ["4 Large eggs", "6 Large eggs"]);
        // whole lines side by side (no name column)
        let md = "# Scones\n\n## Ingredients\n\n| SINGLE BATCH | DOUBLE BATCH |\n| --- | --- |\n| 2 cups flour | 4 cups flour |\n| 1 large egg | 2 large egg |\n";
        let g = &parse(md).recipe.unwrap().ingredients[0];
        assert_eq!(g.items[1].alts, ["1 large egg", "2 large egg"]);
        // one portion, name first: a plain ingredient line
        let md = "# M\n\n## Topping Ingredients\n\n| cane sugar | ½ cup |\n| --- | --- |\n| salt | pinch |\n";
        let g = &parse(md).recipe.unwrap().ingredients[0];
        assert_eq!((g.items[0].text.as_str(), g.items[1].text.as_str()), ("½ cup cane sugar", "pinch salt"));
        assert!(g.portions.is_empty() && g.items[0].alts.is_empty());
        assert_eq!(g.table, Some([4, 6]));
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
        assert_eq!(r.steps, vec![Group::new(None, vec![Line::new(14, "Mix the topping."), Line::new(16, "Bake.")])]);
    }

    #[test]
    fn every_ingredient_set() {
        let texts = |g: &Group| g.items.iter().map(|l| l.text.clone()).collect::<Vec<_>>();
        // no headings: every list after the first is found too, named by a bold label; "Add:" stays a step
        let md = "# Ribs\n\n- 1 cup onion\n- 2 Tbsp butter\n\nCook until soft.\n\nAdd:\n\n- 2 Tbsp vinegar\n- 1 Tbsp salt\n\nSimmer.\n\n**Sauce**\n\n- ½ cup ketchup\n- 1 tsp mustard\n";
        let r = parse(md).recipe.unwrap();
        assert_eq!(r.ingredients.len(), 3);
        assert_eq!(r.ingredients[1].name, None);
        assert_eq!(r.ingredients[2].name.as_deref(), Some("Sauce"));
        assert_eq!(texts(&r.ingredients[2]), ["½ cup ketchup", "1 tsp mustard"]);
        let steps: Vec<_> = r.steps.iter().flat_map(|g| g.items.iter().map(|l| l.text.as_str())).collect();
        assert_eq!(steps, ["Cook until soft.", "Add:", "Simmer."]);
        // a labelled set before the Ingredients heading, and one under Directions
        let md = "# Stew\n\n**Dumplings**:\n\n- 1 ½ cups flour\n- ¾ cup milk\n\n## Ingredients\n\n- 1 chicken\n- 5 cups broth\n\n## Directions\n\n1. Stew it.\n\n### Turmeric Rice\n\n1 ¼ cup rice\n\n2 ½ cups water\n\nCook the rice.\n";
        let r = parse(md).recipe.unwrap();
        let names: Vec<_> = r.ingredients.iter().map(|g| g.name.as_deref()).collect();
        assert_eq!(names, [Some("Dumplings"), None, Some("Turmeric Rice")]);
        assert_eq!(texts(&r.ingredients[2]), ["1 ¼ cup rice", "2 ½ cups water"]);
        assert!(r.steps.iter().flat_map(|g| &g.items).any(|l| l.text == "Cook the rice."));
        // "- + item" wrapper lists: the nested items are the ingredients
        let md = "# Biscuits\n\n## Ingredients\n\n- + 3 cups flour\n  + 1 tsp salt\n  + 8 Tbsp butter\n\n## Directions\n\n1. Mix.\n";
        assert_eq!(texts(&parse(md).recipe.unwrap().ingredients[0]), ["3 cups flour", "1 tsp salt", "8 Tbsp butter"]);
        // paragraph lines with quantities; times are not ingredients
        let md = "# Fish\n\n4 fillets\n1 cup flour\n1 egg\n8 oz. beer\n\nFry at 350.\n\n15 MINS\n\n20 MINS\n";
        let r = parse(md).recipe.unwrap();
        assert_eq!(r.ingredient_count(), 4);
        assert!(!r.ingredients.iter().flat_map(|g| &g.items).any(|l| l.text.contains("MINS")));
    }

    #[test]
    fn bad_frontmatter_does_not_block() {
        let p = parse("---\n: : :\n---\n# T\n");
        assert!(p.frontmatter_error.is_some());
        assert_eq!(p.title.as_deref(), Some("T"));
    }

    /// The recipes folder for the tests below (they run with `--ignored`; the recipes are not in this repo).
    fn vault() -> String {
        std::env::var("VAULT").expect("set VAULT to a recipes folder, e.g. VAULT=~/Recipes")
    }

    /// `VAULT=~/Recipes DUMP=/path/out.json cargo test -p recipes-core dump_vault -- --ignored`: every
    /// note's ingredients and steps, for reviewing parser changes across a whole recipes folder.
    #[test]
    #[ignore = "needs VAULT and DUMP"]
    fn dump_vault() {
        let root = &vault();
        let mut out = std::collections::BTreeMap::new();
        for e in walkdir::WalkDir::new(root).into_iter().filter_entry(|e| e.depth() == 0 || !crate::vault::hidden(e)).filter_map(Result::ok) {
            if e.path().extension().is_some_and(|x| x == "md") {
                let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
                let recipe = parse(&std::fs::read_to_string(e.path()).unwrap()).recipe.map(|mut r| {
                    crate::ingredient::annotate(&mut r);
                    r
                });
                out.insert(rel, recipe);
            }
        }
        std::fs::write(std::env::var("DUMP").unwrap(), serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }

    /// `VAULT=~/Recipes TECHNIQUES="$PWD/../recipe-tools/.tools/techniques.txt" cargo test -p recipes-core -- --ignored`:
    /// every note in a real recipes folder parses and every recipe is detected (pages listed in the
    /// TECHNIQUES file, kept on the recipes repository's tools branch, are exempt).
    #[test]
    #[ignore = "needs VAULT"]
    fn whole_vault_parses() {
        let root = &vault();
        let (mut n, mut recipes) = (0, 0);
        let techniques: Vec<String> = std::env::var("TECHNIQUES")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        let (mut missed, mut charts, mut steps_qty) = (vec![], vec![], vec![]);
        for e in walkdir::WalkDir::new(root).into_iter().filter_entry(|e| e.depth() == 0 || !crate::vault::hidden(e)).filter_map(Result::ok) {
            if e.path().extension().is_some_and(|x| x == "md") {
                let text = std::fs::read_to_string(e.path()).unwrap();
                let p = parse(&text);
                assert!(p.frontmatter_error.is_none(), "{}: {:?}", e.path().display(), p.frontmatter_error);
                n += 1;
                let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned();
                let reference = rel.starts_with("Reference/");
                match (&p.recipe, reference || techniques.contains(&rel)) {
                    (None, false) => missed.push(rel.clone()),
                    // charts are not recipes; these two pages are collections of small ingredient sets
                    (Some(_), true) if reference && !["Reference/Fermented Foods.md", "Reference/Balanced Meals.md"].contains(&rel.as_str()) => charts.push(rel.clone()),
                    _ => {}
                }
                if let Some(r) = &p.recipe {
                    // a short quantity line ("1 tsp vanilla") among the steps is a missed ingredient
                    for it in r.steps.iter().flat_map(|g| &g.items) {
                        if QTY_UNIT.is_match(lead(&it.text)) && it.text.split_whitespace().count() <= 14 && !it.text.starts_with('(') {
                            steps_qty.push(format!("{rel}: {}", it.text));
                        }
                    }
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
        eprintln!("{recipes}/{n} notes detected as recipes");
        assert!(missed.is_empty(), "not detected as recipes: {missed:?}");
        assert!(charts.is_empty(), "reference pages read as recipes: {charts:?}");
        assert!(steps_qty.is_empty(), "ingredient lines read as steps: {steps_qty:?}");
    }
}
