//! Turn a web page (or document) into a draft markdown recipe, without AI.
//!
//! Chain: schema.org JSON-LD → WP Recipe Maker HTML (adds ingredient/step groups) →
//! microdata → readability fallback. Parsing heuristics ported from the old WordPress
//! importer (recipe-webapp/.../class-jsonld-parser.php, class-wprm-parser.php).

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

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
    pub steps: Vec<(Option<String>, Vec<String>)>,
    pub notes: Vec<String>,
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
    /// json-ld | wprm | microdata | page
    pub method: String,
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

// ---------- schema.org (JSON-LD and microdata share this) ----------

fn is_recipe(v: &Value) -> bool {
    let t = |s: &str| s.rsplit(['/', ':']).next() == Some("Recipe");
    match v.get("@type") {
        Some(Value::String(s)) => t(s),
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str().is_some_and(t)),
        _ => false,
    }
}

fn find_recipe(v: &Value, depth: usize) -> Option<&Value> {
    if depth > 8 {
        return None;
    }
    match v {
        Value::Array(a) => a.iter().find_map(|x| find_recipe(x, depth + 1)),
        Value::Object(o) => {
            if is_recipe(v) {
                return Some(v);
            }
            ["@graph", "mainEntity", "mainEntityOfPage", "itemListElement", "item", "hasPart"]
                .iter()
                .filter_map(|k| o.get(*k))
                .find_map(|x| find_recipe(x, depth + 1))
        }
        _ => None,
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => clean(s),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.iter().map(text).find(|s| !s.is_empty()).unwrap_or_default(),
        Value::Object(o) => o.get("name").or(o.get("text")).or(o.get("@value")).map(text).unwrap_or_default(),
        _ => String::new(),
    }
}

fn list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().map(text).filter(|s| !s.is_empty()).collect(),
        Some(Value::String(s)) => s.split(',').map(clean).filter(|s| !s.is_empty()).collect(),
        Some(x) => vec![text(x)].into_iter().filter(|s| !s.is_empty()).collect(),
        None => vec![],
    }
}

/// PT1H20M / P1DT2H -> "1 day 2 hr" style human text.
pub fn iso_duration(s: &str) -> String {
    static RX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^P(?:(\d+(?:\.\d+)?)D)?(?:T(?:(\d+(?:\.\d+)?)H)?(?:(\d+(?:\.\d+)?)M)?(?:(\d+(?:\.\d+)?)S)?)?$").unwrap()
    });
    let Some(c) = RX.captures(s.trim()) else { return clean(s) };
    let n = |i: usize| c.get(i).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
    let total_min = n(1) * 1440.0 + n(2) * 60.0 + n(3) + n(4) / 60.0;
    let total = total_min.round() as i64;
    if total <= 0 {
        return String::new();
    }
    let (d, h, m) = (total / 1440, (total % 1440) / 60, total % 60);
    let mut parts = vec![];
    if d > 0 {
        parts.push(format!("{d} day{}", if d > 1 { "s" } else { "" }));
    }
    if h > 0 {
        parts.push(format!("{h} hr"));
    }
    if m > 0 {
        parts.push(format!("{m} min"));
    }
    parts.join(" ")
}

fn instructions(v: Option<&Value>, out: &mut Vec<(Option<String>, Vec<String>)>, images: &mut Vec<Candidate>) {
    fn walk(v: &Value, group: &Option<String>, out: &mut Vec<(Option<String>, Vec<String>)>, images: &mut Vec<Candidate>) {
        let push = |out: &mut Vec<(Option<String>, Vec<String>)>, g: &Option<String>, s: String| {
            if s.is_empty() {
                return;
            }
            match out.last_mut() {
                Some((lg, items)) if lg == g => items.push(s),
                _ => out.push((g.clone(), vec![s])),
            }
        };
        match v {
            Value::String(s) => {
                // one string: split on line breaks / list items
                let s = s.replace("</li>", "\n").replace("</p>", "\n");
                for line in clean_lines(&s) {
                    push(out, group, line);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, group, out, images)),
            Value::Object(o) => {
                let ty = o.get("@type").map(text).unwrap_or_default();
                if ty.ends_with("HowToSection") || (o.contains_key("itemListElement") && !o.contains_key("text")) {
                    let name = o.get("name").map(text).filter(|s| !s.is_empty());
                    if let Some(items) = o.get("itemListElement") {
                        walk(items, &name.or(group.clone()), out, images);
                    }
                } else {
                    let t = o.get("text").map(text).filter(|s| !s.is_empty()).or_else(|| o.get("name").map(text)).unwrap_or_default();
                    if let Some(img) = o.get("image") {
                        for u in image_urls(img) {
                            images.push(Candidate { url: u, alt: String::new(), kind: "step".into(), step: Some(t.chars().take(60).collect()) });
                        }
                    }
                    push(out, group, t);
                }
            }
            _ => {}
        }
    }
    if let Some(v) = v {
        walk(v, &None, out, images);
    }
}

fn clean_lines(s: &str) -> Vec<String> {
    let frag = Html::parse_fragment(s);
    let t: String = frag.root_element().text().collect();
    t.lines().map(|l| WS.replace_all(l.trim(), " ").into_owned()).filter(|l| !l.is_empty()).collect()
}

pub fn image_urls(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.trim().to_string()],
        Value::Array(a) => a.iter().flat_map(image_urls).collect(),
        Value::Object(o) => o.get("url").or(o.get("contentUrl")).or(o.get("@id")).map(image_urls).unwrap_or_default(),
        _ => vec![],
    }
}

fn schema_to_rec(r: &Value, images: &mut Vec<Candidate>) -> Rec {
    let mut rec = Rec {
        name: r.get("name").map(text).unwrap_or_default(),
        description: r.get("description").map(text).unwrap_or_default(),
        ..Default::default()
    };
    rec.author = match r.get("author") {
        Some(Value::Array(a)) => a.iter().map(text).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
        Some(x) => text(x),
        None => String::new(),
    };
    rec.yield_ = match r.get("recipeYield").or(r.get("yield")) {
        // ["48", "4 dozen cookies"] -> prefer the descriptive one
        Some(Value::Array(a)) => {
            let all: Vec<String> = a.iter().map(text).filter(|s| !s.is_empty()).collect();
            all.iter().find(|s| s.chars().any(|c| c.is_alphabetic())).or(all.first()).cloned().unwrap_or_default()
        }
        Some(x) => text(x),
        None => String::new(),
    };
    let dur = |k: &str| r.get(k).map(|v| iso_duration(&text(v))).unwrap_or_default();
    rec.prep = dur("prepTime");
    rec.cook = dur("cookTime");
    rec.total = dur("totalTime");
    if rec.prep.is_empty() {
        rec.prep = dur("performTime");
    }
    let mut tags: Vec<String> = list(r.get("recipeCategory")).into_iter().chain(list(r.get("recipeCuisine"))).map(|t| t.to_lowercase()).collect();
    tags.dedup();
    tags.truncate(6);
    rec.tags = tags;
    let ings = list(r.get("recipeIngredient").or(r.get("ingredients")));
    if !ings.is_empty() {
        rec.ingredients.push((None, ings));
    }
    instructions(r.get("recipeInstructions"), &mut rec.steps, images);
    if let Some(img) = r.get("image").or(r.get("thumbnailUrl")) {
        for u in image_urls(img) {
            images.insert(images.iter().filter(|c| c.kind == "recipe").count(), Candidate { url: u, alt: rec.name.clone(), kind: "recipe".into(), step: None });
        }
    }
    rec
}

fn jsonld(doc: &Html, images: &mut Vec<Candidate>) -> Option<Rec> {
    for s in doc.select(&sel(r#"script[type="application/ld+json"]"#)) {
        let raw: String = s.text().collect();
        let raw = raw.trim().trim_start_matches("<!--").trim_end_matches("-->").trim();
        // entities are decoded per value later (decoding first would break the JSON)
        let v: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => match serde_json::from_str(&raw.replace(['\n', '\r', '\t'], " ")) {
                Ok(v) => v,
                Err(_) => continue,
            },
        };
        if let Some(r) = find_recipe(&v, 0) {
            return Some(schema_to_rec(r, images));
        }
    }
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
    (!rec.ingredients.is_empty()).then_some(rec)
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

/// A recipe web page (or saved .html file) -> draft.
pub fn from_html(html: &str, url: Option<&url::Url>) -> Draft {
    let doc = Html::parse_document(html);
    let mut images = vec![];
    let (rtitle, rcontent, rtext) = readable(html, url.map(|u| u.as_str()));
    let source = url.map(|u| u.as_str());
    let wp = wprm(&doc);
    let (rec, method) = match (jsonld(&doc, &mut images), wp) {
        (Some(mut r), Some(w)) => {
            // JSON-LD metadata, WPRM's grouped lists
            if w.ingredients.iter().map(|g| g.1.len()).sum::<usize>() >= r.ingredients.iter().map(|g| g.1.len()).sum::<usize>() {
                r.ingredients = w.ingredients;
            }
            if !w.steps.is_empty() && w.steps.len() > 1 {
                r.steps = w.steps;
            }
            r.notes = w.notes;
            (Some(r), "json-ld")
        }
        (Some(r), None) => (Some(r), "json-ld"),
        (None, Some(w)) => (Some(w), "wprm"),
        (None, None) => match microdata(&doc, &mut images) {
            Some(r) => (Some(r), "microdata"),
            None => (None, "page"),
        },
    };
    page_images(&doc, &mut images);
    let images = finish_images(images, url);
    match rec {
        Some(r) => Draft { title: r.name.clone(), markdown: rec_to_markdown(&r, source), source_text: rtext, images, method: method.into() },
        None => {
            let body = if rcontent.is_empty() { html_to_markdown(html) } else { html_to_markdown(&rcontent) };
            let title = if rtitle.is_empty() { doc.select(&sel("title")).next().map(el_text).unwrap_or_default() } else { rtitle };
            let fm = source.map(|s| format!("---\nsource: {}\n---\n\n", yaml(s))).unwrap_or_default();
            let md = format!("{fm}{}", tidy_text(&format!("# {title}\n\n{body}")));
            Draft { title, markdown: dedupe_title(&md), source_text: rtext, images, method: "page".into() }
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
        let d = from_html(html, Some(&u));
        assert_eq!(d.method, "json-ld");
        assert_eq!(d.title, "Mac & Cheese");
        assert!(d.markdown.contains("author: Ann"));
        assert!(d.markdown.contains("yield: 4 servings"));
        assert!(d.markdown.contains("cook_time: 1 hr 5 min"));
        assert!(d.markdown.contains("total_time: 1 day 2 hr"));
        assert!(d.markdown.contains("- 2 cups cheddar"));
        assert!(d.markdown.contains("### Sauce\n\n1. Melt butter.\n2. Whisk flour."));
        assert!(d.markdown.contains("1. Serve."));
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
        let d = from_html(html, None);
        assert_eq!(d.method, "microdata");
        assert!(d.markdown.contains("- 1 slice bread"));

        let page = "<html><head><title>Grandma's Soup</title></head><body><article><h1>Grandma's Soup</h1><p>Ingredients</p><p>2 carrots</p><p>1 onion</p><p>Directions</p><p>Chop and simmer for an hour until tender.</p></article></body></html>";
        let d = from_html(page, None);
        assert_eq!(d.method, "page");
        assert!(d.markdown.contains("- 2 carrots"), "{}", d.markdown);
    }

    #[test]
    fn wprm_groups() {
        let html = r#"<div class="wprm-recipe-container"><h2 class="wprm-recipe-name">Pie</h2>
          <div class="wprm-recipe-ingredient-group"><h4 class="wprm-recipe-group-name">Crust</h4><ul>
            <li class="wprm-recipe-ingredient"><span class="wprm-recipe-ingredient-amount">1 ¼</span> <span class="wprm-recipe-ingredient-unit">cups</span> <span class="wprm-recipe-ingredient-name">flour</span></li></ul></div>
          <div class="wprm-recipe-instruction-group"><ul><li><div class="wprm-recipe-instruction-text">Mix.</div></li></ul></div></div>"#;
        let d = from_html(html, None);
        assert_eq!(d.method, "wprm");
        assert!(d.markdown.contains("### Crust\n\n- 1 ¼ cups flour"));
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
