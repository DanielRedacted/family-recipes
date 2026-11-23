//! Turn a web page (or document) into a draft markdown recipe, without AI.
//!
//! Chain: schema.org JSON-LD → WP Recipe Maker HTML (adds ingredient/step groups) →
//! microdata → readability fallback. Parsing heuristics ported from the old WordPress
//! importer (recipe-webapp/.../class-jsonld-parser.php, class-wprm-parser.php).

use crate::schema::{self, find_recipe, schema_to_rec};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

pub use crate::schema::iso_duration;

#[derive(Default, Debug, Clone)]
pub struct Rec {
    pub name: String,
    pub description: String,
    pub author: String,
    pub yield_: String,
    pub prep: String,
    pub cook: String,
    pub total: String,
    pub tags: Vec<String>,
    pub ingredients: Vec<(Option<String>, Vec<String>)>,
    pub equipment: Vec<String>,
    pub steps: Vec<(Option<String>, Vec<String>)>,
    pub notes: Vec<String>,
    /// (key, value as written), e.g. ("calories", "320 kcal"); keys listed in docs/FORMAT.md
    pub nutrition: Vec<(String, String)>,
}

impl Rec {
    fn count(g: &[(Option<String>, Vec<String>)]) -> usize {
        g.iter().map(|x| x.1.len()).sum()
    }
    /// Fill whatever this record lacks from `other`.
    fn fill_from(&mut self, other: &Rec) {
        for (mine, theirs) in [
            (&mut self.name, &other.name),
            (&mut self.description, &other.description),
            (&mut self.author, &other.author),
            (&mut self.yield_, &other.yield_),
            (&mut self.prep, &other.prep),
            (&mut self.cook, &other.cook),
            (&mut self.total, &other.total),
        ] {
            if mine.is_empty() {
                *mine = theirs.clone();
            }
        }
        for (mine, theirs) in [(&mut self.ingredients, &other.ingredients), (&mut self.steps, &other.steps)] {
            if mine.is_empty() {
                *mine = theirs.clone();
            }
        }
        for (mine, theirs) in [(&mut self.equipment, &other.equipment), (&mut self.notes, &other.notes)] {
            if mine.is_empty() {
                *mine = theirs.clone();
            }
        }
        if self.nutrition.is_empty() {
            self.nutrition = other.nutrition.clone();
        }
        schema::add_tags(&mut self.tags, other.tags.clone());
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Candidate {
    pub url: String,
    pub alt: String,
    /// recipe | step | page | content | document
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
}

pub struct Draft {
    pub title: String,
    pub markdown: String,
    pub source_text: String,
    pub images: Vec<Candidate>,
    /// wprm-rest | json-ld | wprm | microdata | page | document
    pub method: String,
    /// what was looked for and found, for the import log: (step, detail, found)
    pub trace: Vec<(&'static str, String, bool)>,
    /// the page's own recipe data (WP Recipe Maker REST or JSON-LD), archived as the recipe's original
    pub json: Option<Value>,
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).unwrap()
}

static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Strip tags, decode entities, collapse whitespace.
pub fn clean(s: &str) -> String {
    let text = if s.contains('<') || s.contains('&') {
        let frag = Html::parse_fragment(&s.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n"));
        frag.root_element().text().collect::<String>()
    } else {
        s.to_string()
    };
    WS.replace_all(text.replace('\u{a0}', " ").trim(), " ").into_owned()
}

fn el_text(e: ElementRef) -> String {
    clean(&e.text().collect::<Vec<_>>().join(" "))
}

// ---------- schema.org (JSON-LD and microdata share schema.rs) ----------

fn jsonld(doc: &Html, images: &mut Vec<Candidate>, trace: &mut Vec<(&'static str, String, bool)>) -> Option<(Rec, Value)> {
    let scripts: Vec<_> = doc.select(&sel(r#"script[type="application/ld+json"]"#)).collect();
    let mut bad = 0;
    for s in &scripts {
        let raw: String = s.text().collect();
        let raw = raw.trim().trim_start_matches("<!--").trim_end_matches("-->").trim();
        // entities are decoded per value later (decoding first would break the JSON)
        let v: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => match serde_json::from_str(&raw.replace(['\n', '\r', '\t'], " ")) {
                Ok(v) => v,
                Err(_) => {
                    bad += 1;
                    continue;
                }
            },
        };
        if let Some(r) = find_recipe(&v, 0) {
            let rec = schema_to_rec(r, images);
            trace.push(("json-ld", format!("Recipe found ({} ingredients, {} steps) in {} script(s)", Rec::count(&rec.ingredients), Rec::count(&rec.steps), scripts.len()), true));
            return Some((rec, r.clone()));
        }
    }
    let why = if scripts.is_empty() { "no JSON-LD on the page".to_string() } else { format!("{} JSON-LD script(s), no Recipe{}", scripts.len(), if bad > 0 { format!(", {bad} unreadable") } else { String::new() }) };
    trace.push(("json-ld", why, false));
    None
}

/// Microdata (itemscope itemtype=".../Recipe") folded into the JSON-LD shape.
fn microdata(doc: &Html, images: &mut Vec<Candidate>) -> Option<Rec> {
    let scope = doc.select(&sel("[itemscope][itemtype]")).find(|e| e.value().attr("itemtype").is_some_and(|t| t.ends_with("/Recipe")))?;
    let mut o = Map::new();
    o.insert("@type".into(), json!("Recipe"));
    for e in scope.select(&sel("[itemprop]")) {
        let prop = e.value().attr("itemprop").unwrap_or("");
        let v = e.value();
        let val = v
            .attr("content")
            .or(v.attr("datetime"))
            .map(str::to_string)
            .or_else(|| if matches!(v.name(), "img" | "source") { v.attr("src").map(str::to_string) } else { None })
            .or_else(|| if matches!(v.name(), "link" | "a") && prop == "image" { v.attr("href").map(str::to_string) } else { None })
            .unwrap_or_else(|| el_text(e));
        if val.is_empty() {
            continue;
        }
        for p in prop.split_whitespace() {
            match o.get_mut(p) {
                Some(Value::Array(a)) => a.push(json!(val)),
                Some(x) => *x = json!([x.clone(), val]),
                None => {
                    o.insert(p.into(), if matches!(p, "recipeIngredient" | "ingredients" | "recipeInstructions") { json!([val]) } else { json!(val) });
                }
            }
        }
    }
    let v = Value::Object(o);
    let rec = schema_to_rec(&v, images);
    (!rec.ingredients.is_empty()).then_some(rec)
}

/// WP Recipe Maker markup: keeps ingredient and instruction groups that JSON-LD flattens.
fn wprm(doc: &Html) -> Option<Rec> {
    let c = doc.select(&sel(".wprm-recipe-container, .wprm-recipe")).next()?;
    let mut rec = Rec::default();
    let first = |s: &str| c.select(&sel(s)).next().map(el_text).unwrap_or_default();
    rec.name = first(".wprm-recipe-name");
    rec.description = first(".wprm-recipe-summary");
    rec.author = first(".wprm-recipe-author");
    let servings = first(".wprm-recipe-servings");
    let unit = first(".wprm-recipe-servings-unit");
    rec.yield_ = format!("{servings} {unit}").trim().to_string();
    for g in c.select(&sel(".wprm-recipe-ingredient-group")) {
        let name = g.select(&sel(".wprm-recipe-group-name")).next().map(el_text).filter(|s| !s.is_empty());
        let items: Vec<String> = g
            .select(&sel(".wprm-recipe-ingredient"))
            .map(|i| {
                let parts: Vec<String> = [".wprm-recipe-ingredient-amount", ".wprm-recipe-ingredient-unit", ".wprm-recipe-ingredient-name", ".wprm-recipe-ingredient-notes"]
                    .iter()
                    .filter_map(|s| i.select(&sel(s)).next().map(el_text))
                    .filter(|s| !s.is_empty())
                    .collect();
                if parts.is_empty() { el_text(i) } else { parts.join(" ") }
            })
            .map(|s| s.trim_start_matches(['▢', '☐', '□']).trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !items.is_empty() {
            rec.ingredients.push((name, items));
        }
    }
    for g in c.select(&sel(".wprm-recipe-instruction-group")) {
        let name = g.select(&sel(".wprm-recipe-group-name")).next().map(el_text).filter(|s| !s.is_empty());
        let items: Vec<String> = g.select(&sel(".wprm-recipe-instruction-text")).map(el_text).filter(|s| !s.is_empty()).collect();
        if !items.is_empty() {
            rec.steps.push((name, items));
        }
    }
    let notes = c.select(&sel(".wprm-recipe-notes")).next().map(|n| n.text().collect::<String>()).unwrap_or_default();
    rec.notes = notes.lines().map(|l| WS.replace_all(l.trim(), " ").into_owned()).filter(|l| !l.is_empty()).collect();
    // times: <span class="wprm-recipe-prep_time-hours">1</span> <span class="…-minutes">20</span>
    let num = |s: &str| first(s).chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<i64>().unwrap_or(0);
    for (t, slot) in [("prep", &mut rec.prep), ("cook", &mut rec.cook), ("total", &mut rec.total)] {
        *slot = schema::minutes_text(num(&format!(".wprm-recipe-{t}_time-hours")) * 60 + num(&format!(".wprm-recipe-{t}_time-minutes")));
    }
    for e in c.select(&sel(".wprm-recipe-equipment-name")) {
        let t = el_text(e);
        if !t.is_empty() && !rec.equipment.contains(&t) {
            rec.equipment.push(t);
        }
    }
    // nutrition label: .wprm-nutrition-label-text-nutrition-container-<key> > -value + -unit
    for n in c.select(&sel(".wprm-nutrition-label-text-nutrition-container")) {
        let Some(key) = n.value().classes().find_map(|k| k.strip_prefix("wprm-nutrition-label-text-nutrition-container-")) else { continue };
        let part = |s: &str| n.select(&sel(s)).next().map(el_text).unwrap_or_default();
        let v = part(".wprm-nutrition-label-text-nutrition-value");
        if !v.is_empty() {
            let unit = part(".wprm-nutrition-label-text-nutrition-unit");
            rec.nutrition.push((key.replace('-', "_"), format!("{v} {unit}").trim().to_string()));
        }
    }
    for t in ["course", "cuisine", "keyword"] {
        schema::add_tags(&mut rec.tags, first(&format!(".wprm-recipe-{t}")).split(',').map(str::to_string));
    }
    (!rec.ingredients.is_empty()).then_some(rec)
}

/// WP Recipe Maker recipe id on the page, for the REST lookup.
pub fn wprm_id(html: &str) -> Option<u64> {
    static RX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"wprm-recipe-container-(\d+)|data-recipe-id="(\d+)""#).unwrap());
    let c = RX.captures(html)?;
    c.get(1).or(c.get(2))?.as_str().parse().ok()
}

// ---------- images ----------

static JUNK_IMG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(logo|icon|avatar|sprite|pixel|/ads?/|doubleclick|gravatar|emoji|badge|placeholder|spinner|loading|blank\.gif|data:image/svg|\.svg(\?|$)|\.ico(\?|$)|facebook\.com/tr|pinterest|share)").unwrap()
});

fn best_srcset(s: &str) -> Option<String> {
    s.split(',')
        .filter_map(|c| {
            let mut p = c.split_whitespace();
            let u = p.next()?;
            let w = p.next().and_then(|d| d.trim_end_matches(['w', 'x']).parse::<f64>().ok()).unwrap_or(1.0);
            Some((w, u.to_string()))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|x| x.1)
}

fn page_images(doc: &Html, out: &mut Vec<Candidate>) {
    for m in doc.select(&sel(r#"meta[property="og:image"], meta[name="twitter:image"], meta[property="og:image:url"]"#)) {
        if let Some(c) = m.value().attr("content") {
            out.push(Candidate { url: c.into(), alt: String::new(), kind: "page".into(), step: None });
        }
    }
    let root = doc
        .select(&sel(".wprm-recipe-container, .tasty-recipes, article, main, [role=main]"))
        .next()
        .unwrap_or_else(|| doc.root_element());
    for img in root.select(&sel("img, picture source")) {
        let v = img.value();
        let small = |a: &str| v.attr(a).and_then(|x| x.parse::<u32>().ok()).is_some_and(|n| n < 150);
        if small("width") || small("height") {
            continue;
        }
        let url = v
            .attr("data-lazy-srcset")
            .or(v.attr("data-srcset"))
            .or(v.attr("srcset"))
            .and_then(best_srcset)
            .or_else(|| v.attr("data-lazy-src").or(v.attr("data-src")).or(v.attr("data-original")).or(v.attr("src")).map(str::to_string));
        if let Some(u) = url.filter(|u| !u.starts_with("data:")) {
            out.push(Candidate { url: u, alt: v.attr("alt").unwrap_or("").chars().take(120).collect(), kind: "content".into(), step: None });
        }
    }
}

fn finish_images(mut imgs: Vec<Candidate>, base: Option<&url::Url>) -> Vec<Candidate> {
    let mut seen = std::collections::HashSet::new();
    imgs.retain_mut(|c| {
        let abs = match base {
            Some(b) => b.join(c.url.trim()).map(|u| u.to_string()).unwrap_or_default(),
            None => c.url.clone(),
        };
        c.url = abs;
        (c.url.starts_with("http://") || c.url.starts_with("https://"))
            && !JUNK_IMG.is_match(&c.url)
            && seen.insert(c.url.split('?').next().unwrap_or("").to_string())
    });
    imgs.truncate(40);
    imgs
}

// ---------- markdown ----------

fn yaml(v: &str) -> String {
    static SAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9 ,./()½¼¾⅓⅔+-]*$").unwrap());
    if SAFE.is_match(v) && !v.ends_with(':') { v.to_string() } else { serde_json::to_string(v).unwrap() }
}

pub fn rec_to_markdown(r: &Rec, source: Option<&str>) -> String {
    let mut fm = vec![];
    if let Some(s) = source {
        fm.push(format!("source: {}", yaml(s)));
    }
    for (k, v) in [("author", &r.author), ("yield", &r.yield_), ("prep_time", &r.prep), ("cook_time", &r.cook), ("total_time", &r.total)] {
        if !v.is_empty() {
            fm.push(format!("{k}: {}", yaml(v)));
        }
    }
    if !r.tags.is_empty() {
        fm.push(format!("tags: [{}]", r.tags.iter().map(|t| yaml(t)).collect::<Vec<_>>().join(", ")));
    }
    if !r.nutrition.is_empty() {
        fm.push("nutrition:".into());
        for (k, v) in &r.nutrition {
            fm.push(format!("  {k}: {}", yaml(v)));
        }
    }
    let mut md = String::new();
    if !fm.is_empty() {
        md.push_str(&format!("---\n{}\n---\n\n", fm.join("\n")));
    }
    md.push_str(&format!("# {}\n\n", if r.name.is_empty() { "Untitled recipe" } else { &r.name }));
    if !r.description.is_empty() {
        md.push_str(&format!("{}\n\n", r.description));
    }
    md.push_str("## Ingredients\n\n");
    for (g, items) in &r.ingredients {
        if let Some(g) = g {
            md.push_str(&format!("### {g}\n\n"));
        }
        for i in items {
            md.push_str(&format!("- {i}\n"));
        }
        md.push('\n');
    }
    if !r.equipment.is_empty() {
        md.push_str("## Equipment\n\n");
        for e in &r.equipment {
            md.push_str(&format!("- {e}\n"));
        }
        md.push('\n');
    }
    md.push_str("## Directions\n\n");
    for (g, items) in &r.steps {
        if let Some(g) = g {
            md.push_str(&format!("### {g}\n\n"));
        }
        for (n, s) in items.iter().enumerate() {
            md.push_str(&format!("{}. {s}\n", n + 1));
        }
        md.push('\n');
    }
    if !r.notes.is_empty() {
        md.push_str("## Notes\n\n");
        for n in &r.notes {
            md.push_str(&format!("{n}\n\n"));
        }
    }
    md.trim_end().to_string() + "\n"
}

/// Plain text (PDF, TXT) or loosely structured markdown -> recipe-shaped markdown.
/// Only adds structure (headings, bullets); never rewords.
pub fn tidy_text(text: &str) -> String {
    static LABEL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^[*_#\s]*((?:[\w' -]{0,30}\s)?(?:ingredients?|you(?:'ll| will) need|directions?|instructions?|method|preparation|procedures?|steps|notes?))[*_\s]*:?[*_\s]*$").unwrap()
    });
    static QTY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\d|[½¼¾⅓⅔⅛]|(a|an|one|two|three|pinch|dash)\b)").unwrap());
    static LIST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([-*+•▢]|\d+[.)])\s+").unwrap());
    let mut out = vec![];
    let mut section = "";
    let mut titled = text.lines().any(|l| l.starts_with("# "));
    static NESTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[*+-]\s+){2,}|^\s{2,6}[*+-]\s+|^[*+]\s+").unwrap());
    static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)\.\s{2,}").unwrap());
    for raw in text.lines() {
        // Word/HTML exports: "*   *   6 egg whites", "    *   salt", "1.  Mix" -> plain "- " / "1. " lists
        let raw = NESTED.replace(raw, "- ");
        let raw = NUM.replace(&raw, "$1. ");
        let line = raw.trim_end().replace('\u{a0}', " ");
        let t = line.trim();
        if t.is_empty() {
            out.push(String::new());
            continue;
        }
        if !titled && !LIST.is_match(t) && t.len() <= 90 {
            out.push(format!("# {}", t.trim_matches(['*', '#', ' '])));
            out.push(String::new());
            titled = true;
            continue;
        }
        titled = true;
        if let Some(c) = LABEL.captures(t).filter(|_| !t.starts_with("##")) {
            let label = c[1].trim().to_string();
            let l = label.to_lowercase();
            section = if l.contains("ingredient") || l.contains("need") { "ing" } else if l.contains("note") { "notes" } else { "steps" };
            out.push(String::new());
            out.push(format!("## {label}"));
            out.push(String::new());
            continue;
        }
        if t.starts_with('#') || t.starts_with('|') {
            out.push(line.to_string());
            continue;
        }
        if section == "ing" && !LIST.is_match(t) && (QTY.is_match(t) || t.split_whitespace().count() <= 6) {
            out.push(format!("- {t}"));
        } else if LIST.is_match(t) && t.starts_with(['•', '▢']) {
            out.push(format!("- {}", LIST.replace(t, "")));
        } else {
            out.push(line.to_string());
        }
    }
    // collapse runs of blank lines; keep bullets of one list together
    let mut md = String::new();
    let mut blank = 0;
    for (i, l) in out.iter().enumerate() {
        if l.is_empty() {
            blank += 1;
            let between_bullets = out.get(i.wrapping_sub(blank)).is_some_and(|p| p.starts_with("- "))
                && out[i..].iter().find(|x| !x.is_empty()).is_some_and(|n| n.starts_with("- "));
            if blank == 1 && !between_bullets {
                md.push('\n');
            }
            continue;
        }
        blank = 0;
        md.push_str(l);
        md.push('\n');
    }
    md.trim().to_string() + "\n"
}

// ---------- entry points ----------

fn readable(html: &str, url: Option<&str>) -> (String, String, String) {
    let mut r = match dom_smoothie::Readability::new(html, url, None) {
        Ok(r) => r,
        Err(_) => return (String::new(), String::new(), String::new()),
    };
    match r.parse() {
        Ok(a) => (a.title, a.content.to_string(), a.text_content.to_string()),
        Err(_) => (String::new(), String::new(), String::new()),
    }
}

pub fn html_to_markdown(html: &str) -> String {
    htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "noscript", "iframe", "svg", "form", "button", "nav", "footer", "aside"])
        .build()
        .convert(html)
        .unwrap_or_default()
}

/// A recipe web page (or saved .html file) -> draft. `wprm_rest` is the `recipe` object from
/// the site's WP Recipe Maker REST endpoint when the caller could fetch it.
pub fn from_html(html: &str, url: Option<&url::Url>, wprm_rest: Option<&Value>) -> Draft {
    let doc = Html::parse_document(html);
    let mut images = vec![];
    let mut trace = vec![];
    let (rtitle, rcontent, rtext) = readable(html, url.map(|u| u.as_str()));
    let source = url.map(|u| u.as_str());
    let summary = |r: &Rec| format!("{} ingredients, {} steps, {} equipment, {} nutrition values", Rec::count(&r.ingredients), Rec::count(&r.steps), r.equipment.len(), r.nutrition.len());
    let rest = wprm_rest.and_then(|v| schema::wprm_rest(v, &mut images));
    let wp = wprm(&doc);
    match &wp {
        Some(w) => trace.push(("wprm-html", format!("WP Recipe Maker card: {}", summary(w)), true)),
        None => trace.push(("wprm-html", "no WP Recipe Maker card".into(), false)),
    }
    let (ld, ld_json) = jsonld(&doc, &mut images, &mut trace).unzip();
    let (rec, method) = match (rest, ld, wp) {
        // the site's own recipe data is the fullest; fill gaps from JSON-LD / the card
        (Some(mut r), ld, wp) => {
            for other in [ld, wp].into_iter().flatten() {
                r.fill_from(&other);
            }
            (Some(r), "wprm-rest")
        }
        (None, Some(mut r), Some(w)) => {
            // JSON-LD metadata, WPRM's grouped lists, equipment and nutrition
            if Rec::count(&w.ingredients) >= Rec::count(&r.ingredients) {
                r.ingredients = w.ingredients.clone();
            }
            if w.steps.len() > 1 {
                r.steps = w.steps.clone();
            }
            r.notes = w.notes.clone();
            if !w.equipment.is_empty() {
                r.equipment = w.equipment.clone();
            }
            if !w.nutrition.is_empty() {
                r.nutrition = w.nutrition.clone();
            }
            r.fill_from(&w);
            (Some(r), "json-ld")
        }
        (None, Some(r), None) => (Some(r), "json-ld"),
        (None, None, Some(w)) => (Some(w), "wprm"),
        (None, None, None) => match microdata(&doc, &mut images) {
            Some(r) => {
                trace.push(("microdata", format!("Recipe microdata: {}", summary(&r)), true));
                (Some(r), "microdata")
            }
            None => {
                trace.push(("microdata", "no Recipe microdata".into(), false));
                (None, "page")
            }
        },
    };
    let json = match method {
        "wprm-rest" => wprm_rest.cloned(),
        "json-ld" => ld_json,
        _ => None,
    };
    page_images(&doc, &mut images);
    let images = finish_images(images, url);
    trace.push(("images", format!("{} candidate picture(s)", images.len()), !images.is_empty()));
    match rec {
        Some(r) => {
            trace.push(("result", format!("{method}: {}", summary(&r)), true));
            Draft { title: r.name.clone(), markdown: rec_to_markdown(&r, source), source_text: rtext, images, method: method.into(), trace, json }
        }
        None => {
            trace.push(("readability", format!("no recipe data; using the page text ({} characters)", rtext.chars().count()), !rtext.is_empty()));
            let body = if rcontent.is_empty() { html_to_markdown(html) } else { html_to_markdown(&rcontent) };
            let title = if rtitle.is_empty() { doc.select(&sel("title")).next().map(el_text).unwrap_or_default() } else { rtitle };
            let fm = source.map(|s| format!("---\nsource: {}\n---\n\n", yaml(s))).unwrap_or_default();
            let md = format!("{fm}{}", tidy_text(&format!("# {title}\n\n{body}")));
            Draft { title, markdown: dedupe_title(&md), source_text: rtext, images, method: "page".into(), trace, json: None }
        }
    }
}

/// tidy_text promotes the first line to "# title"; drop it if it repeats the title we added.
fn dedupe_title(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let h1: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("# ")).map(|(i, _)| i).take(2).collect();
    if h1.len() == 2 && lines[h1[0]].to_lowercase() == lines[h1[1]].to_lowercase() {
        let mut v = lines.clone();
        v.remove(h1[1]);
        return v.join("\n") + "\n";
    }
    md.to_string()
}

/// A document's HTML (DOCX via mammoth, DOC/RTF via textutil) -> draft; embedded images
/// (data: URIs) are returned separately so they go through the image picker.
pub fn from_document_html(html: &str, fallback_title: &str) -> (String, Vec<(String, Vec<u8>)>) {
    static IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<img[^>]*src="data:([^;"]+);base64,([^"]+)"[^>]*>"#).unwrap());
    let mut imgs = vec![];
    let stripped = IMG.replace_all(html, |c: &regex::Captures| {
        if let Ok(b) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &c[2]) {
            imgs.push((c[1].to_string(), b));
        }
        ""
    });
    let md = html_to_markdown(&stripped);
    let mut md = tidy_text(&md);
    if !md.starts_with("# ") {
        md = format!("# {fallback_title}\n\n{md}");
    }
    (md, imgs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonld_graph_sections_and_shapes() {
        let html = r#"<html><head><script type="application/ld+json">{"@context":"https://schema.org","@graph":[
          {"@type":"WebPage","name":"x"},
          {"@type":["Recipe","NewsArticle"],"name":"Mac &amp; Cheese","author":[{"@type":"Person","name":"Ann"}],
           "recipeYield":["4","4 servings"],"prepTime":"PT15M","cookTime":"P0DT1H5M","totalTime":"P1DT2H",
           "image":[{"@type":"ImageObject","url":"https://ex.com/a.jpg"},"https://ex.com/b.jpg"],
           "recipeCategory":"Main","recipeIngredient":["1 lb pasta","2 cups <b>cheddar</b>"],
           "recipeInstructions":[{"@type":"HowToSection","name":"Sauce","itemListElement":[{"@type":"HowToStep","text":"Melt butter."},{"@type":"HowToStep","text":"Whisk flour.","image":"https://ex.com/s.jpg"}]},
             {"@type":"HowToStep","text":"Serve."}],
           "nutrition":{"calories":320}}]}</script></head><body></body></html>"#;
        let u = url::Url::parse("https://ex.com/r/mac").unwrap();
        let d = from_html(html, Some(&u), None);
        assert_eq!(d.method, "json-ld");
        assert_eq!(d.json.as_ref().and_then(|v| v["name"].as_str()), Some("Mac &amp; Cheese"), "the JSON-LD recipe is kept as published");
        assert_eq!(d.title, "Mac & Cheese");
        assert!(d.markdown.contains("author: Ann"));
        assert!(d.markdown.contains("yield: 4 servings"));
        assert!(d.markdown.contains("cook_time: 1 hr 5 min"));
        assert!(d.markdown.contains("total_time: 1 day 2 hr"));
        assert!(d.markdown.contains("- 2 cups cheddar"));
        assert!(d.markdown.contains("### Sauce\n\n1. Melt butter.\n2. Whisk flour."));
        assert!(d.markdown.contains("1. Serve."));
        assert!(d.markdown.contains("nutrition:\n  calories: 320 kcal\n"), "{}", d.markdown);
        assert!(d.trace.iter().any(|t| t.0 == "json-ld" && t.2));
        assert_eq!(d.images[0].url, "https://ex.com/a.jpg");
        assert!(d.images.iter().any(|c| c.kind == "step"));
        let p = crate::recipe::parse(&d.markdown);
        assert_eq!(p.recipe.unwrap().ingredient_count(), 2);
    }

    #[test]
    fn microdata_and_fallback() {
        let html = r#"<div itemscope itemtype="http://schema.org/Recipe"><h1 itemprop="name">Toast</h1>
          <span itemprop="recipeIngredient">1 slice bread</span><span itemprop="recipeIngredient">butter</span>
          <div itemprop="recipeInstructions">Toast the bread.</div></div>"#;
        let d = from_html(html, None, None);
        assert_eq!(d.method, "microdata");
        assert!(d.json.is_none());
        assert!(d.markdown.contains("- 1 slice bread"));

        let page = "<html><head><title>Grandma's Soup</title></head><body><article><h1>Grandma's Soup</h1><p>Ingredients</p><p>2 carrots</p><p>1 onion</p><p>Directions</p><p>Chop and simmer for an hour until tender.</p></article></body></html>";
        let d = from_html(page, None, None);
        assert_eq!(d.method, "page");
        assert!(d.json.is_none());
        assert!(d.markdown.contains("- 2 carrots"), "{}", d.markdown);
    }

    #[test]
    fn wprm_groups() {
        let html = r#"<div class="wprm-recipe-container"><h2 class="wprm-recipe-name">Pie</h2>
          <div class="wprm-recipe-ingredient-group"><h4 class="wprm-recipe-group-name">Crust</h4><ul>
            <li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-amount">1 ¼</span> <span class="wprm-recipe-ingredient-unit">cups</span> <span class="wprm-recipe-ingredient-name">flour</span></li></ul></div>
          <div class="wprm-recipe-instruction-group"><ul><li><div class="wprm-recipe-instruction-text">Mix.</div></li></ul></div></div>"#;
        let d = from_html(html, None, None);
        assert_eq!(d.method, "wprm");
        assert!(d.markdown.contains("### Crust\n\n- 1 ¼ cups flour"));
    }

    #[test]
    fn wprm_card_extras_and_rest_merge() {
        let html = r#"<script type="application/ld+json">{"@type":"Recipe","name":"Pie","recipeYield":"8","recipeIngredient":["flour"]}</script>
          <div id="wprm-recipe-container-77" class="wprm-recipe-container" data-recipe-id="77"><h2 class="wprm-recipe-name">Pie</h2>
          <span class="wprm-recipe-prep_time-hours">1</span><span class="wprm-recipe-prep_time-minutes">20<span>minutes</span></span>
          <div class="wprm-recipe-ingredient-group"><ul><li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-name">flour</span></li><li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-name">salt</span></li></ul></div>
          <div class="wprm-recipe-equipment-container"><div class="wprm-recipe-equipment-name">Pie dish</div></div>
          <span class="wprm-nutrition-label-text-nutrition-container wprm-nutrition-label-text-nutrition-container-calories"><span class="wprm-nutrition-label-text-nutrition-label">Calories: </span><span class="wprm-nutrition-label-text-nutrition-value">410</span><span class="wprm-nutrition-label-text-nutrition-unit">kcal</span></span>
          <span class="wprm-recipe-course">Dessert, Holiday</span></div>"#;
        assert_eq!(wprm_id(html), Some(77));
        let d = from_html(html, None, None);
        assert_eq!(d.method, "json-ld");
        for want in ["prep_time: 1 hr 20 min", "yield: 8", "## Equipment\n\n- Pie dish", "calories: 410 kcal", "- salt", "tags: [dessert, holiday]"] {
            assert!(d.markdown.contains(want), "missing {want:?} in\n{}", d.markdown);
        }
        let rest: Value = serde_json::from_str(include_str!("../tests/fixtures/wprm-rest.json")).unwrap();
        let d = from_html(html, None, Some(&rest["recipe"]));
        assert_eq!(d.method, "wprm-rest");
        assert_eq!(d.json.as_ref(), Some(&rest["recipe"]));
        assert!(d.markdown.contains("# Classic Pancakes"));
        assert!(d.markdown.contains("- 1 Rolling Pin (optional)"));
        assert!(d.markdown.contains("### Cook\n\n1. Heat the pan."), "{}", d.markdown);
    }

    #[test]
    fn tidy_word_lists() {
        let md = tidy_text("# Macaroons\n\n*   *   6 egg whites\n    *   ¼ teaspoon salt\n\n1.  Whisk egg whites\n2.  Bake\n");
        assert!(md.contains("- 6 egg whites\n- ¼ teaspoon salt"), "{md}");
        assert!(md.contains("1. Whisk egg whites"));
        let r = crate::recipe::parse(&md).recipe.unwrap();
        assert_eq!(r.ingredient_count(), 2);
        assert_eq!(r.steps[0].items.len(), 2);
    }

    #[test]
    fn tidy_plain_text() {
        let md = tidy_text("Aunt May's Bread\n\nIngredients:\n2 cups flour\nSalt\n\nDirections\nMix everything together and bake for 30 minutes.\n");
        assert!(md.starts_with("# Aunt May's Bread"));
        assert!(md.contains("## Ingredients\n\n- 2 cups flour\n- Salt"));
        assert!(md.contains("## Directions"));
    }
}
