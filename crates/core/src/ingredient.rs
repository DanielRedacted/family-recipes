//! Ingredient catalogue: every ingredient line in the vault split into quantity / unit / name,
//! names grouped under a normalised key so "Eggs", "2 large eggs, beaten" and "egg" are one
//! ingredient. Read-only; used for the Ingredients page and editor suggestions.

use crate::server::AppState;
use crate::vault::Index;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, Mutex};

#[derive(Debug, PartialEq, Default)]
pub struct IngredientLine {
    pub qty: Option<String>,
    pub unit: Option<String>,
    pub name: String,
    pub note: Option<String>,
}

/// Units as written → canonical spelling shown in suggestions.
const UNITS: &[(&str, &str)] = &[
    ("cups", "cup"), ("cup", "cup"), ("c", "cup"),
    ("tablespoons", "tbsp"), ("tablespoon", "tbsp"), ("tbsps", "tbsp"), ("tbsp", "tbsp"), ("tbs", "tbsp"), ("tbl", "tbsp"), ("tbps", "tbsp"), ("tb", "tbsp"), ("t", "tbsp"),
    ("teaspoons", "tsp"), ("teaspoon", "tsp"), ("tsps", "tsp"), ("tsp", "tsp"), ("ts", "tsp"),
    ("ounces", "oz"), ("ounce", "oz"), ("oz", "oz"), ("fl oz", "fl oz"),
    ("pounds", "lb"), ("pound", "lb"), ("lbs", "lb"), ("lb", "lb"),
    ("grams", "g"), ("gram", "g"), ("g", "g"), ("kilograms", "kg"), ("kg", "kg"),
    ("milliliters", "ml"), ("millilitres", "ml"), ("ml", "ml"), ("liters", "l"), ("litres", "l"), ("liter", "l"), ("litre", "l"), ("l", "l"),
    ("quarts", "qt"), ("quart", "qt"), ("qt", "qt"), ("pints", "pt"), ("pint", "pt"), ("pt", "pt"), ("gallons", "gal"), ("gallon", "gal"), ("gal", "gal"),
    ("pinches", "pinch"), ("pinch", "pinch"), ("dashes", "dash"), ("dash", "dash"), ("drops", "drop"), ("drop", "drop"),
    ("cloves", "clove"), ("clove", "clove"), ("cans", "can"), ("can", "can"), ("jars", "jar"), ("jar", "jar"),
    ("packages", "package"), ("package", "package"), ("pkgs", "package"), ("pkg", "package"), ("packets", "packet"), ("packet", "packet"), ("envelopes", "envelope"), ("envelope", "envelope"),
    ("sticks", "stick"), ("stick", "stick"), ("slices", "slice"), ("slice", "slice"), ("pieces", "piece"), ("piece", "piece"),
    ("bunches", "bunch"), ("bunch", "bunch"), ("sprigs", "sprig"), ("sprig", "sprig"), ("stalks", "stalk"), ("stalk", "stalk"), ("heads", "head"), ("head", "head"),
    ("boxes", "box"), ("box", "box"), ("bags", "bag"), ("bag", "bag"), ("handfuls", "handful"), ("handful", "handful"),
];

/// Words that describe preparation or size, not the ingredient ("2 large eggs, beaten").
const DESCRIPTORS: &[&str] = &[
    "fresh", "freshly", "chopped", "minced", "diced", "sliced", "grated", "shredded", "melted", "softened", "sifted", "divided",
    "optional", "finely", "coarsely", "roughly", "thinly", "packed", "heaping", "level", "scant", "cold", "warm", "hot",
    "cooked", "uncooked", "raw", "ripe", "peeled", "seeded", "crushed", "large", "medium", "small", "extra", "about",
    "approximately", "room", "temperature", "to", "taste", "of", "a", "an", "the", "plus", "more", "for", "and", "or",
];

/// Different names for the same thing (after normalising). Only true synonyms.
const ALIASES: &[(&str, &str)] = &[
    ("ap flour", "all purpose flour"),
    ("plain flour", "all purpose flour"),
    ("vanilla extract", "vanilla"),
    ("pure vanilla extract", "vanilla"),
    ("powdered sugar", "confectioners sugar"),
    ("confectioner's sugar", "confectioners sugar"),
    ("icing sugar", "confectioners sugar"),
    ("bicarbonate of soda", "baking soda"),
    ("bicarb", "baking soda"),
    ("evoo", "extra virgin olive oil"),
    ("kosher salt", "kosher salt"),
    ("granulated sugar", "sugar"),
    ("white sugar", "sugar"),
];

/// Plurals that are not "word + s".
const KEEP: &[&str] = &["molasses", "hummus", "couscous", "asparagus", "oats", "grits", "greens", "chips", "sprinkles", "flakes", "swiss", "brussels", "bbq", "dates"];

static QTY: LazyLock<Regex> = LazyLock::new(|| {
    let n = r"(?:\d+\s+\d+/\d+|\d+/\d+|\d+(?:[.,]\d+)?(?:\s*[½¼¾⅓⅔⅛⅜⅝⅞])?|[½¼¾⅓⅔⅛⅜⅝⅞])";
    let w = r"(?:a few|a|an|one|two|three|four|five|six|seven|eight|nine|ten|twelve|half|few|several|dozen)\b";
    Regex::new(&format!(r"(?i)^(?:about\s+|approx\.?\s+)?((?:{n}|{w})(?:\s*(?:-|–|to|or)\s*(?:{n}|{w}))?)\s*")).unwrap()
});
static PAREN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\(([^)]*)\)\s*").unwrap());
static UNIT: LazyLock<Regex> = LazyLock::new(|| {
    let mut u: Vec<&str> = UNITS.iter().map(|x| x.0).collect();
    u.sort_by_key(|s| std::cmp::Reverse(s.len()));
    Regex::new(&format!(r"(?i)^({})\.?,?(?:\s+|$)(?:of\s+)?", u.iter().map(|s| regex::escape(s)).collect::<Vec<_>>().join("|"))).unwrap()
});
static CONTAINER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(cans?|jars?|packages?|pkgs?|box(es)?|bags?|bottles?|cartons?|containers?)\s+(of\s+)?").unwrap());
/// a package size after the count: "1 14-oz can", "2 8oz packages"
static SIZE2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\d+(?:\.\d+)?\s*-?\s*(?:fl\.?\s*oz|oz|ounces?|g|grams?|ml|lbs?|pounds?))\.?\s+").unwrap());
static SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(large|medium|small|heaping|level|scant|generous)\s+").unwrap());
static MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]*)?\]\]|\[([^\]]*)\]\([^)]*\)|[*_`]").unwrap());
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d|[½¼¾⅓⅔⅛⅜⅝⅞])").unwrap());

fn plain(s: &str) -> String {
    let s = s.replace('⁄', "/");
    MD.replace_all(&s, |c: &regex::Captures| c.get(1).or(c.get(2)).map_or(String::new(), |m| m.as_str().to_string())).trim().to_string()
}

/// "1 ½ cups flour, sifted" → qty "1 ½", unit "cup", name "flour", note "sifted".
pub fn parse_line(text: &str) -> IngredientLine {
    let text = plain(text);
    // a table row "1 ½ cups · AP flour · 3 cups": the cell without a quantity holds the name
    if text.contains(" · ") {
        let cell = text.split(" · ").find(|c| !NUM.is_match(c.trim())).unwrap_or("").trim();
        return IngredientLine { qty: None, ..parse_line(cell) };
    }
    let mut rest = text.as_str();
    let mut out = IngredientLine::default();
    if let Some(c) = QTY.captures(rest) {
        // "a" / "an" only count as a quantity when a unit follows ("a pinch of salt", not "a bay leaf"? both fine)
        out.qty = Some(c[1].trim().to_string());
        rest = &rest[c[0].len()..];
    }
    if let Some(c) = PAREN.captures(rest) {
        out.note = Some(c[1].trim().to_string());
        rest = &rest[c[0].len()..];
    } else if let Some(c) = out.qty.as_ref().and_then(|_| SIZE2.captures(rest)) {
        // "1 14-oz can of …": the size is a note, the container part of the amount
        out.note = Some(c[1].trim().to_string());
        rest = &rest[c[0].len()..];
        if let Some(c) = CONTAINER.captures(rest) {
            rest = &rest[c[0].len()..];
        }
    }
    if let Some(c) = SIZE.captures(rest) {
        rest = &rest[c[0].len()..];
    }
    if let Some(c) = UNIT.captures(rest) {
        // without a number in front only spelled-out units count ("Tbsp sugar", not "T-bone")
        let bare_ok = c[1].chars().count() >= 2 && rest[c[0].len()..].chars().any(char::is_alphabetic);
        if out.qty.is_some() || bare_ok {
            let u = c[1].to_lowercase();
            out.unit = UNITS.iter().find(|(w, _)| *w == u).map(|(_, canon)| canon.to_string());
            rest = &rest[c[0].len()..];
            // "15 oz can pumpkin": the container is part of the amount
            if let Some(c) = CONTAINER.captures(rest) {
                rest = &rest[c[0].len()..];
            }
            // written twice ("1 ¼ tsp teaspoon salt") or an alternative amount ("15 oz 14 oz milk")
            if let Some(c) = QTY.captures(rest).filter(|c| UNIT.is_match(&rest[c[0].len()..])) {
                let alt_end = c[0].len() + UNIT.find(&rest[c[0].len()..]).map_or(0, |m| m.end());
                out.note = Some(rest[..alt_end].trim().to_string());
                rest = &rest[alt_end..];
            } else if let Some(c) = UNIT.captures(rest).filter(|c| c[1].chars().count() >= 2) {
                rest = &rest[c[0].len()..];
            }
        }
    }
    let lower = rest.to_lowercase();
    let cut = rest.find([',', '(', ';']).into_iter().chain([" - ", " – ", " to taste", " for "].iter().filter_map(|w| lower.find(w))).min().unwrap_or(rest.len());
    out.name = rest[..cut].trim().trim_end_matches(['.', ':']).trim().to_string();
    let note = rest[cut..].trim_start_matches([',', ';', ' ', '-', '–']).trim();
    let note = note.strip_prefix('(').and_then(|n| n.strip_suffix(')')).filter(|n| !n.contains(['(', ')'])).unwrap_or(note);
    if !note.is_empty() {
        out.note = Some(match out.note.take() {
            Some(p) => format!("{p}; {note}"),
            None => note.to_string(),
        });
    }
    out
}

fn singular(w: &str) -> String {
    if KEEP.contains(&w) || w.len() <= 3 || w.ends_with("ss") || w.ends_with("us") {
        return w.to_string();
    }
    for (suf, rep) in [("ies", "y"), ("oes", "o"), ("ches", "ch"), ("shes", "sh"), ("xes", "x"), ("ves", "f")] {
        if let Some(stem) = w.strip_suffix(suf) {
            // "cloves" → "clove", "olives" → "olive": only leaves/halves/loaves take "f"
            if suf == "ves" && !matches!(stem, "lea" | "hal" | "loa" | "cal") {
                return format!("{stem}ve");
            }
            return format!("{stem}{rep}");
        }
    }
    w.strip_suffix('s').map(str::to_string).unwrap_or_else(|| w.to_string())
}

/// Normalised grouping key for an ingredient name ("All-Purpose Flour" → "all purpose flour").
pub fn key(name: &str) -> Option<String> {
    use unicode_normalization::UnicodeNormalization;
    let lower: String = name.nfkc().collect::<String>().to_lowercase().replace(['-', '/'], " ");
    let words: Vec<String> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '&' || c == '\''))
        .map(|w| w.trim_matches('\''))
        .filter(|w| !w.is_empty() && !DESCRIPTORS.contains(w) && !w.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect();
    let (last, init) = words.split_last()?;
    let mut k = init.join(" ");
    if !k.is_empty() {
        k.push(' ');
    }
    k.push_str(&singular(last));
    let k = ALIASES.iter().find(|(a, _)| *a == k).map_or(k, |(_, to)| to.to_string());
    (k.chars().any(char::is_alphabetic)).then_some(k)
}

#[derive(Serialize, Debug)]
pub struct Ingredient {
    pub name: String,
    pub key: String,
    /// number of recipes using it
    pub count: usize,
    pub recipes: Vec<String>,
    /// other ways it is written (most common first)
    pub forms: Vec<String>,
}

#[derive(Default)]
struct Agg {
    forms: HashMap<String, usize>,
    recipes: BTreeSet<String>,
}

pub fn catalogue(idx: &Index) -> Vec<Ingredient> {
    let mut by_key: BTreeMap<String, Agg> = BTreeMap::new();
    for e in idx.files.values() {
        let Some(r) = &e.recipe else { continue };
        for it in r.ingredients.iter().flat_map(|g| &g.items) {
            if it.text.trim_end().trim_end_matches(['*', '_']).ends_with(':') {
                continue; // a label inside the list ("Mix:")
            }
            let l = parse_line(&it.text);
            // sentences and odd lines are not ingredient names
            if l.name.is_empty() || l.name.chars().count() > 50 || l.name.split_whitespace().count() > 6 {
                continue;
            }
            let Some(k) = key(&l.name) else { continue };
            let a = by_key.entry(k).or_default();
            *a.forms.entry(l.name).or_default() += 1;
            a.recipes.insert(e.path.as_str().to_string());
        }
    }
    // near-duplicates (typos): keys of 7+ letters one edit apart join the more common one
    // pairwise within same first letter and length ±1; an alias file if merges go wrong
    let keys: Vec<String> = by_key.keys().cloned().collect();
    let mut into: HashMap<String, String> = HashMap::new();
    for (i, a) in keys.iter().enumerate() {
        if a.chars().count() < 7 {
            continue;
        }
        for b in &keys[i + 1..] {
            if b.chars().next() != a.chars().next() {
                break; // keys are sorted: no later key shares the first letter
            }
            let (la, lb) = (a.chars().count(), b.chars().count());
            if la.abs_diff(lb) > 1 || lb < 7 || strsim::damerau_levenshtein(a, b) > 1 {
                continue;
            }
            let (na, nb) = (by_key[a].recipes.len(), by_key[b].recipes.len());
            let (small, big) = if na <= nb { (a, b) } else { (b, a) };
            if by_key[big].recipes.len() >= 2 {
                into.insert(small.clone(), big.clone());
            }
        }
    }
    for (small, big) in &into {
        let big = into.get(big).unwrap_or(big).clone();
        if let Some(s) = by_key.remove(small) {
            let b = by_key.entry(big).or_default();
            for (f, n) in s.forms {
                *b.forms.entry(f).or_default() += n;
            }
            b.recipes.extend(s.recipes);
        }
    }
    let mut out: Vec<Ingredient> = by_key
        .into_iter()
        .map(|(key, a)| {
            let mut forms: Vec<(String, usize)> = a.forms.into_iter().collect();
            forms.sort_by(|x, y| y.1.cmp(&x.1).then_with(|| x.0.cmp(&y.0)));
            let name = forms[0].0.clone();
            Ingredient {
                name,
                key,
                count: a.recipes.len(),
                recipes: a.recipes.into_iter().collect(),
                forms: forms.into_iter().skip(1).take(6).map(|f| f.0).collect(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Cached JSON, rebuilt when the vault payload changes.
#[derive(Default)]
pub struct Cache(Mutex<Option<(String, Arc<Vec<u8>>)>>);

/// `GET /api/ingredients`: everyone who can read recipes can read this.
pub async fn route(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, Response> {
    let etag = s.payload.read().unwrap().etag.clone();
    let cached = s.ext.ingredients.0.lock().unwrap().clone().filter(|c| c.0 == etag);
    let body = match cached {
        Some((_, b)) => b,
        None => {
            let idx = s.index.read().unwrap().clone();
            let b = tokio::task::spawn_blocking(move || {
                let mut units: Vec<&str> = UNITS.iter().map(|u| u.1).collect();
                units.dedup();
                let units: BTreeSet<&str> = units.into_iter().collect();
                serde_json::to_vec(&serde_json::json!({ "ingredients": catalogue(&idx), "units": units })).unwrap()
            })
            .await
            .unwrap();
            let b = Arc::new(b);
            *s.ext.ingredients.0.lock().unwrap() = Some((etag.clone(), b.clone()));
            b
        }
    };
    let tag = format!("\"i{etag}\"");
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(tag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    Ok(([(header::CONTENT_TYPE, "application/json".to_string()), (header::ETAG, tag), (header::CACHE_CONTROL, "no-cache".to_string())], body.as_ref().clone()).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (Option<&'static str>, Option<&'static str>, String, Option<String>) {
        let l = parse_line(s);
        (l.qty.map(|q| Box::leak(q.into_boxed_str()) as &str), l.unit.map(|u| Box::leak(u.into_boxed_str()) as &str), l.name, l.note)
    }

    #[test]
    fn lines() {
        assert_eq!(p("1 ½ cups flour, sifted"), (Some("1 ½"), Some("cup"), "flour".into(), Some("sifted".into())));
        assert_eq!(p("½ cup Warm milk"), (Some("½"), Some("cup"), "Warm milk".into(), None));
        assert_eq!(p("2 large eggs, beaten"), (Some("2"), None, "eggs".into(), Some("beaten".into())));
        assert_eq!(p("1 (15 oz) can pumpkin"), (Some("1"), Some("can"), "pumpkin".into(), Some("15 oz".into())));
        assert_eq!(p("3-4 cloves garlic, minced"), (Some("3-4"), Some("clove"), "garlic".into(), Some("minced".into())));
        assert_eq!(p("Salt to taste"), (None, None, "Salt".into(), Some("to taste".into())));
        assert_eq!(p("1 Tbsp sugar for dusting"), (Some("1"), Some("tbsp"), "sugar".into(), Some("for dusting".into())));
        assert_eq!(p("1/2 tsp. **baking soda**"), (Some("1/2"), Some("tsp"), "baking soda".into(), None));
        assert_eq!(p("2 Tbsp unsalted butter - softened"), (Some("2"), Some("tbsp"), "unsalted butter".into(), Some("softened".into())));
        assert_eq!(p("1 cup of sugar"), (Some("1"), Some("cup"), "sugar".into(), None));
        assert_eq!(p("Pinch of salt"), (None, Some("pinch"), "salt".into(), None));
        assert_eq!(p("1 ½ cups · AP flour · 3 cups"), (None, None, "AP flour".into(), None));
        assert_eq!(p("1 · Tbsp sugar · 2"), (None, Some("tbsp"), "sugar".into(), None));
        assert_eq!(p("2 TB garlic cloves"), (Some("2"), Some("tbsp"), "garlic cloves".into(), None));
        assert_eq!(p("Tea bags"), (None, None, "Tea bags".into(), None));
        assert_eq!(p("1 1⁄2 tablespoons [cinnamon](http://x)"), (Some("1 1/2"), Some("tbsp"), "cinnamon".into(), None));
        assert_eq!(p("1 14-oz can of coconut cream"), (Some("1"), None, "coconut cream".into(), Some("14-oz".into())));
        assert_eq!(p("1 ¼ tsp teaspoon salt").2, "salt");
        assert_eq!(p("15 oz 14 oz milk"), (Some("15"), Some("oz"), "milk".into(), Some("14 oz".into())));
        assert_eq!(p("¼ tsp, pepper").2, "pepper");
        assert_eq!(p("pinch tsp salt").2, "salt");
        assert_eq!(p("12 oz can evaporated milk (or 8 oz Half n Half)"), (Some("12"), Some("oz"), "evaporated milk".into(), Some("or 8 oz Half n Half".into())));
    }

    #[test]
    fn keys() {
        assert_eq!(key("Eggs").as_deref(), Some("egg"));
        assert_eq!(key("large eggs").as_deref(), Some("egg"));
        assert_eq!(key("All-Purpose Flour"), key("all purpose flour"));
        assert_eq!(key("tomatoes").as_deref(), Some("tomato"));
        assert_eq!(key("cloves").as_deref(), Some("clove"));
        assert_eq!(key("molasses").as_deref(), Some("molasses"));
        assert_eq!(key("Salt to taste").as_deref(), Some("salt"));
        assert_eq!(key("fresh chopped parsley").as_deref(), Some("parsley"));
        assert_eq!(key("123"), None);
        assert_eq!(key("AP flour"), key("All-purpose flour"));
        assert_eq!(key("pure vanilla extract").as_deref(), Some("vanilla"));
    }

    #[test]
    fn seed_catalogue() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../vault-seed"));
        let idx = crate::vault::scan(root, None, None);
        let cat = catalogue(&idx);
        let egg = cat.iter().find(|i| i.key == "egg").expect("egg");
        assert!(egg.count >= 40, "eggs in {} recipes", egg.count);
        assert!(cat.iter().all(|i| !i.name.is_empty()));
        eprintln!("{} ingredients; most used: {:?}", cat.len(), {
            let mut v: Vec<_> = cat.iter().map(|i| (i.count, i.name.as_str())).collect();
            v.sort_by(|a, b| b.0.cmp(&a.0));
            v.into_iter().take(15).collect::<Vec<_>>()
        });
    }
}
