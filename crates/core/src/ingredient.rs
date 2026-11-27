//! Ingredient catalogue: every ingredient line in the vault split into quantity / unit / name,
//! names grouped under a normalised key so "Eggs", "2 large eggs, beaten" and "egg" are one
//! ingredient. Read-only; used for the Ingredients page and editor suggestions.

use crate::server::Bk;
use crate::vault::Index;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

#[derive(Debug, PartialEq, Default)]
pub struct IngredientLine {
    pub qty: Option<String>,
    pub unit: Option<String>,
    pub name: String,
    pub note: Option<String>,
    /// "large", "medium", "small" written before the name ("2 large eggs")
    pub size: Option<String>,
}

/// Units as written → the one spelling the app writes (ui/src/lib/units.ts), shown in suggestions.
const UNITS: &[(&str, &str)] = &[
    ("cups", "cup"), ("cup", "cup"), ("c", "cup"),
    ("tablespoons", "tbsp"), ("tablespoon", "tbsp"), ("tbsps", "tbsp"), ("tbsp", "tbsp"), ("tbs", "tbsp"), ("tbl", "tbsp"), ("tbps", "tbsp"), ("tbp", "tbsp"), ("tb", "tbsp"), ("t", "tbsp"),
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
    "approximately", "room", "temperature", "to", "taste", "of", "a", "an", "the", "plus", "more", "for", "and", "or", "some",
    "organic",
];

/// Words that only mean something together, read before `DESCRIPTORS` drop "hot" or keep "non": "Hot Dogs"
/// is not "dog", "non-fat" is not "fat", "non-GMO" says nothing about the food.
const PHRASES: &[(&str, &str)] = &[("non gmo", ""), ("hot dog", "hotdog"), ("hot sauce", "hotsauce"), ("non fat", "nonfat")];

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
    Regex::new(&format!(r"(?i)^(?:about\s+|approx\.?\s+)?((?:{n}|{w})(?:\s*(?:-|–|to|or)\s*(?:{n}|{w})|\s*(?:\+|and)\s*{n})?)\s*(?:\+\s*)?")).unwrap()
});
static PAREN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\(([^)]*)\)\s*").unwrap());
static UNIT: LazyLock<Regex> = LazyLock::new(|| {
    let mut u: Vec<&str> = UNITS.iter().map(|x| x.0).collect();
    u.sort_by_key(|s| std::cmp::Reverse(s.len()));
    Regex::new(&format!(r"(?i)^({})\.?\+?,?(?:\s+|$)(?:of\s+)?", u.iter().map(|s| regex::escape(s)).collect::<Vec<_>>().join("|"))).unwrap()
});
static CONTAINER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(cans?|jars?|packages?|pkgs?|box(es)?|bags?|bottles?|cartons?|containers?)\s+(of\s+)?").unwrap());
/// a package size after the count: "1 14-oz can", "2 8oz packages"
static SIZE2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\d+(?:\.\d+)?\s*-?\s*(?:fl\.?\s*oz|oz|ounces?|g|grams?|ml|lbs?|pounds?))\.?\s+").unwrap());
static SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(extra[- ]large|large|medium|small|jumbo|heaping|level|scant|generous|slight)\s+").unwrap());
/// "1 - 15 oz can": a count, then a package size (not a range)
static COUNT_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)\s*[-–]\s*(\d+(?:\.\d+)?)$").unwrap());
static SIZE_CONTAINER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:fl\.?\s*oz|oz|ounces?|g|grams?|ml|lbs?|pounds?)\.?\s+(?:cans?|jars?|packages?|pkgs?|box(?:es)?|bags?|bottles?|cartons?|containers?)\b").unwrap()
});
/// What is left of a line split mid-amount ("+ 1 tbsp milk", "and 1/2 cup", "less 3 tbsp flour") or a
/// label in front of it ("Optional: ½ tsp vanilla").
static LEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:[+\-–/]\s*|(?:and|(?:an?\s+)?additional|less|or(?:\s+more)?|substitute)\s+|(?:optional|garnish|toppings?)\s*:\s*)+").unwrap()
});
/// A trailing temperature ("hot water 105-115˚F") or source ("Orange Zest from 1 orange") goes to the note.
static TAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+(?:at\s+)?\d[\d\s\-–]*[°˚º]|\s+from\s+\d").unwrap());
/// Alternatives: "butter or ghee", "AP flour and/or rice flour", "egg + 1 tbsp water" ("/" in `alternatives`).
static ALT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+(?:and\s*/\s*or|or)\s+|\s+\+\s+").unwrap());
static AND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+(?:and|&)\s+").unwrap());
/// Directions, suggestions and labels written in the ingredient list.
static NOT_INGREDIENT: LazyLock<Regex> = LazyLock::new(|| {
    let verbs = "add|bake|cook|mix|stir|preheat|pour|serve|fold|combine|place|let|heat|beat|blend|cover|remove|gently|melt|sift|shape|whip|make|marinate|build|reduce|thicken|line";
    Regex::new(&format!(r"(?i)^(?:(?:{verbs})\b|whisk\s+\S|(?:in|on)\s+(?:a|an|the|your)\b|try\b|day\s*\d|makes\b|serves\b)|preheat|minutes|degrees|[°˚]")).unwrap()
});
/// A nutrition statement among the ingredients ("1 serving (1 slice) equals 476 calories, …").
static CALORIES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:calories|kcal)\b").unwrap());
/// Equipment ("mixing bowl and spoon"), singular; with `CONNECTORS` and `DESCRIPTORS` the only words of such a name.
const EQUIPMENT: &[&str] = &[
    "bowl", "mixing", "microwave", "safe", "scissor", "mixer", "hand", "whisk", "spoon", "wooden", "measuring", "cup", "pan", "baking",
    "sheet", "parchment", "paper", "foil", "plastic", "wrap", "thermometer", "skewer", "toothpick",
];
const CONNECTORS: &[&str] = &["with", "in", "on", "size", "sized"];
/// Dotted or spaced abbreviations: "A.P." / "A P" → "ap".
static ABBR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\p{L}\.?(?:\s?\p{L}\.)+|\b\p{L} \p{L}\b").unwrap());
static MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]*)?\]\]|\[([^\]]*)\]\([^)]*\)|[*_`]").unwrap());

fn plain(s: &str) -> String {
    let s = s.replace('⁄', "/").replace('\\', "");
    MD.replace_all(&s, |c: &regex::Captures| c.get(1).or(c.get(2)).map_or(String::new(), |m| m.as_str().to_string())).trim().to_string()
}

/// "1 ½ cups flour, sifted" → qty "1 ½", unit "cup", name "flour", note "sifted".
pub fn parse_line(text: &str) -> IngredientLine {
    let text = LEAD.replace(&plain(text), "").into_owned();
    // "I lb Italian sausage"
    let text = match text.strip_prefix("I ") {
        Some(r) if UNIT.is_match(r) => format!("1 {r}"),
        _ => text,
    };
    let mut rest = text.as_str();
    let mut out = IngredientLine::default();
    if let Some(c) = QTY.captures(rest) {
        // "a" / "an" only count as a quantity when a unit follows ("a pinch of salt", not "a bay leaf"? both fine)
        out.qty = Some(c[1].trim().to_string());
        rest = &rest[c[0].len()..];
        rest = rest.trim_start_matches(['+', '-', '–']).trim_start(); // "3/4 - cup milk"
        // "1 - 15 oz can chickpeas": one can of 15 oz, not the range 1 to 15
        if let Some(r) = COUNT_SIZE.captures(&c[1]).filter(|_| SIZE_CONTAINER.is_match(rest)) {
            out.qty = Some(r[1].to_string());
            rest = &text[c.get(1).unwrap().start() + r.get(2).unwrap().start()..];
        }
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
        out.size = Some(c[1].to_lowercase());
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
            // "Teaspoon Or more citric acid", "1 cup and 2 tbsp warm water", "½ tsp to 1 tsp sugar"
            rest = rest.strip_prefix("to ").unwrap_or(rest);
            rest = &rest[LEAD.find(rest).map_or(0, |m| m.end())..];
            // written twice ("1 ¼ tsp teaspoon salt") or an alternative amount ("15 oz 14 oz milk")
            if let Some(c) = QTY.captures(rest).filter(|c| UNIT.is_match(&rest[c[0].len()..])) {
                let alt_end = c[0].len() + UNIT.find(&rest[c[0].len()..]).map_or(0, |m| m.end());
                out.note = Some(rest[..alt_end].trim().to_string());
                rest = &rest[alt_end..];
            } else if let Some(c) = UNIT.captures(rest).filter(|c| c[1].chars().count() >= 2) {
                rest = &rest[c[0].len()..];
            }
            // "1 ½ cups (150 g) lupin flour": the weight in brackets is a note, not the end of the name
            if let Some(c) = PAREN.captures(rest).filter(|c| c[0].len() < rest.len()) {
                let n = c[1].trim().to_string();
                out.note = Some(out.note.take().map_or(n.clone(), |p| format!("{p}; {n}")));
                rest = &rest[c[0].len()..];
            }
            if let Some(c) = SIZE.captures(rest).filter(|_| out.size.is_none()) {
                out.size = Some(c[1].to_lowercase());
                rest = &rest[c[0].len()..];
            }
        }
    }
    let lower = rest.to_lowercase();
    let cut = rest.find([',', '(', ';'])
        .into_iter()
        .chain([" - ", " – ", " to taste", " for ", " if "].iter().filter_map(|w| lower.find(w)))
        .chain(TAIL.find(rest).map(|m| m.start()))
        .min()
        .unwrap_or(rest.len());
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

/// What one ingredient line (or one portion of a table row) asks for, read for working out nutrition.
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
pub struct Amount {
    /// the amount as a number; a range counts its first number ("2-3" → 2). None when none is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qty: Option<f64>,
    /// one spelling (`UNITS`): "cup", "tbsp", "clove"…
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// the ingredient's normalised name (`key`); the first one when the line offers alternatives
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// a weight written for the whole amount: "1 ½ cups (150 g)" → 150, "2 (15 oz) cans" → 850
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grams: Option<f64>,
    /// why the line is left out although it names an ingredient: "optional", "garnish", "for frying"…
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<String>,
}

static MIXED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)-(\d+\s*/\s*\d+)").unwrap());
static RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(?:\bto\b|\bor\b|–|-)\s*").unwrap());
static PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(?:\+|\band\b)\s*").unwrap());
static NUMBER: LazyLock<Regex> =LazyLock::new(|| Regex::new(r"^(\d+(?:[.,]\d+)?)?\s*(?:-?\s*(\d+)\s*/\s*(\d+)|([½¼¾⅓⅔⅛⅜⅝⅞]))?$").unwrap());
static WEIGHT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:about\s+|approx\.?\s+)?(\d+(?:\.\d+)?)\s*-?\s*(oz|ounces?|g|grams?|lbs?|pounds?|kg)\b").unwrap()
});
/// Not part of the dish as made: left out of nutrition.
static LEFT_OUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(optional|if desired|for (?:garnish|serving|topping|frying|deep frying|dusting|brushing|greasing|rolling|the pan)|garnish)\b").unwrap()
});

/// "1 ½" → 1.5, "1-1/2" → 1.5, "2 ¼ to 2 ¾" → 2.25, "1 and 1/2" → 1.5, "a" → 1, "two" → 2.
pub fn number(q: &str) -> Option<f64> {
    let q = q.trim().to_lowercase();
    let words = [("a", 1.0), ("an", 1.0), ("one", 1.0), ("two", 2.0), ("three", 3.0), ("four", 4.0), ("five", 5.0), ("six", 6.0),
        ("seven", 7.0), ("eight", 8.0), ("nine", 9.0), ("ten", 10.0), ("twelve", 12.0), ("half", 0.5), ("dozen", 12.0)];
    // "1-1/2" is a mixed number, not a range; a range or alternative counts its first number
    let q = MIXED.replace(&q, "$1 $2");
    let head = RANGE.split(&q).next().unwrap_or_default();
    let mut total = 0.0;
    for part in PLUS.split(head) {
        if let Some(w) = words.iter().find(|w| w.0 == part) {
            total += w.1;
            continue;
        }
        let c = NUMBER.captures(part)?;
        if c.get(1).is_none() && c.get(2).is_none() && c.get(4).is_none() {
            return None;
        }
        let whole = c.get(1).map_or(Some(0.0), |m| m.as_str().replace(',', ".").parse().ok())?;
        let frac = match (c.get(2), c.get(3), c.get(4)) {
            (Some(a), Some(b), _) => a.as_str().parse::<f64>().ok()? / b.as_str().parse::<f64>().ok().filter(|b| *b > 0.0)?,
            (_, _, Some(g)) => match g.as_str() { "½" => 0.5, "¼" => 0.25, "¾" => 0.75, "⅓" => 1.0 / 3.0, "⅔" => 2.0 / 3.0, "⅛" => 0.125, "⅜" => 0.375, "⅝" => 0.625, _ => 0.875 },
            _ => 0.0,
        };
        total += whole + frac;
    }
    (total > 0.0).then_some(total)
}

/// Grams in the factor table of a weight unit.
fn unit_grams(u: &str) -> Option<f64> {
    match u.to_lowercase().trim_end_matches('s') {
        "oz" | "ounce" => Some(28.3495),
        "lb" | "pound" => Some(453.592),
        "g" | "gram" => Some(1.0),
        "kg" => Some(1000.0),
        _ => None,
    }
}

/// The amount one ingredient line asks for (`None` for a label, direction or equipment line).
pub fn amount(text: &str) -> Option<Amount> {
    // alternatives read against the foods the nutrition table knows ("milk and/or half and half")
    if CALORIES.is_match(text) {
        return None; // "1 serving (1 slice) equals 476 calories, …" is the recipe's own nutrition
    }
    let l = parse_line(text);
    let key = names_in(text, crate::nutrition::known())
        .first()
        .and_then(|n| key(n))
        // a long measured line names its ingredient first ("3 lb chicken tenderloins or chicken breast cut against the grain")
        .or_else(|| l.qty.as_ref().and(ALT.split(&l.name).next()).filter(|n| !NOT_INGREDIENT.is_match(n)).and_then(key));
    // a measured line naming nothing known ("3 tbsp non-GMO, low inflammatory oil") is still an ingredient
    if key.is_none() && (l.qty.is_none() || l.name.is_empty() || NOT_INGREDIENT.is_match(&plain(text))) {
        return None;
    }
    // a unit without a number is one of it ("Pinch of salt", "Tbsp sugar")
    let qty = l.qty.as_deref().and_then(number).or(l.unit.as_ref().map(|_| 1.0));
    // a weight in the note: per container or item when the amount is a count, else the same amount in grams
    let per_item = matches!(l.unit.as_deref(), None | Some("can" | "jar" | "package" | "packet" | "envelope" | "box" | "bag"));
    let grams = l.note.as_deref().and_then(|n| WEIGHT.captures(n)).and_then(|c| {
        let g = c[1].parse::<f64>().ok()? * unit_grams(&c[2])?;
        Some(if per_item { g * qty.unwrap_or(1.0) } else { g })
    });
    let skip = LEFT_OUT.captures(&plain(text)).map(|c| c[1].to_lowercase());
    Some(Amount { qty, unit: l.unit, size: l.size, key, grams, skip })
}

/// Fill in `amounts` on every ingredient line of a parsed recipe: one per portion of a table row.
pub fn annotate(r: &mut crate::recipe::Recipe) {
    for it in r.ingredients.iter_mut().flat_map(|g| g.items.iter_mut()) {
        it.amounts = if it.alts.is_empty() { vec![amount(&it.text)] } else { it.alts.iter().map(|a| amount(a)).collect() };
    }
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
    let mut lower: String = name.nfkc().collect::<String>().to_lowercase().replace(['-', '/'], " ");
    for (from, to) in PHRASES {
        lower = lower.replace(from, to);
    }
    let lower = ABBR.replace_all(&lower, |c: &regex::Captures| c[0].chars().filter(|c| c.is_alphabetic()).collect::<String>());
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

/// A name made only of equipment words ("microwave-safe bowl", "whisk or hand mixer").
fn equipment(name: &str) -> bool {
    let words: Vec<String> = name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(singular).collect();
    words.iter().any(|w| EQUIPMENT.contains(&w.as_str()))
        && words.iter().all(|w| [EQUIPMENT, CONNECTORS, DESCRIPTORS].iter().any(|l| l.contains(&w.as_str())) || w.chars().count() == 1)
}

fn keep(name: &str) -> bool {
    !NOT_INGREDIENT.is_match(name) && !equipment(name) && !name.contains(char::is_numeric) && key(name).is_some()
}

/// The line's name split into alternatives, each read again for an amount ("Garlic Powder or 2 garlic
/// cloves" → "Garlic Powder", "garlic cloves"). Empty for labels, sentences and directions.
fn alternatives(text: &str) -> Vec<String> {
    if text.trim_end().trim_end_matches(['*', '_']).ends_with(':') || text.trim_start().starts_with("\\*") || CALORIES.is_match(text) {
        return vec![]; // a label inside the list ("Mix:"), a footnote ("\* Best blueberries are …"), nutrition facts
    }
    let name = parse_line(text).name;
    if name.chars().count() > 50 || name.split_whitespace().count() > 6 || NOT_INGREDIENT.is_match(&name) {
        return vec![];
    }
    let mut parts: Vec<String> = vec![];
    for alt in ALT.split(&name) {
        // "/" between words ("Allulose/monk fruit"), not in a fraction ("1/2")
        for (i, s) in alt.split('/').enumerate() {
            match parts.last_mut() {
                Some(p) if i > 0 && !(p.ends_with(char::is_alphabetic) && s.trim_start().starts_with(char::is_alphabetic)) => *p = format!("{p}/{s}"),
                _ => parts.push(s.trim().to_string()),
            }
        }
    }
    parts
        .into_iter()
        .map(|p| {
            // only an amount is read again: "half and half" must not lose "half" as one
            let p = LEAD.replace(p.trim(), "");
            if p.starts_with(char::is_numeric) { parse_line(&p).name } else { p.into_owned() }
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// A part that may borrow the next part's noun: "Avocado" in "Avocado or Olive oil".
fn borrows(parts: &[String], i: usize) -> bool {
    parts.get(i + 1).is_some_and(|next| next.contains(' '))
}

/// The ingredient names one ingredient line contributes (none, one or several).
pub fn names(text: &str) -> Vec<String> {
    names_in(text, &HashSet::new())
}

/// `names`, knowing the keys written elsewhere in the vault (`catalogue`'s first pass), which decide
/// shared nouns and "and" splits.
fn names_in(text: &str, known: &HashSet<String>) -> Vec<String> {
    let is_known = |n: &str| key(n).is_some_and(|k| known.contains(&k));
    let mut parts = alternatives(text);
    for i in 0..parts.len() {
        if borrows(&parts, i) {
            // "AP or Gluten Free flour" → AP flour; "brown rice or tapioca syrup" → brown rice syrup
            let joined = format!("{} {}", parts[i], parts[i + 1].rsplit(' ').next().unwrap_or_default());
            // not "oil or oil blend" → oil blend twice, nor "jalapeno sauce or Red Hot®" → jalapeno sauce Hot®
            let new = key(&joined) != key(&parts[i]) && key(&joined) != key(&parts[i + 1]);
            if new && (is_known(&joined) || (!parts[i].contains(' ') && !is_known(&parts[i]))) {
                parts[i] = joined;
            }
        }
    }
    parts
        .into_iter()
        .flat_map(|p| {
            // "Salt and pepper" splits, "half and half" and "Oil flavor & color" don't
            let and: Vec<String> = AND.split(&p).map(str::to_string).collect();
            if and.len() > 1 && and.iter().all(|a| is_known(a)) { and } else { vec![p] }
        })
        .filter(|p| keep(p))
        .collect()
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

fn merge(by_key: &mut BTreeMap<String, Agg>, into: &HashMap<String, String>) {
    for (small, big) in into {
        let big = into.get(big).unwrap_or(big).clone();
        if let Some(s) = by_key.remove(small) {
            let b = by_key.entry(big).or_default();
            for (f, n) in s.forms {
                *b.forms.entry(f).or_default() += n;
            }
            b.recipes.extend(s.recipes);
        }
    }
}

pub fn catalogue(idx: &Index) -> Vec<Ingredient> {
    let lines: Vec<(&str, &str)> = idx
        .files
        .values()
        .filter_map(|e| Some((e.path.as_str(), e.recipe.as_ref()?)))
        .flat_map(|(path, r)| r.ingredients.iter().flat_map(|g| &g.items).map(move |it| (path, it.text.as_str())))
        .collect();
    // first pass: the keys written on their own, i.e. not a word that may borrow a noun ("Avocado or Olive oil")
    let known: HashSet<String> = lines
        .iter()
        .flat_map(|(_, text)| {
            let parts = alternatives(text);
            (0..parts.len()).filter(|&i| parts[i].contains(' ') || !borrows(&parts, i)).map(|i| parts[i].clone()).collect::<Vec<_>>()
        })
        .filter(|p| keep(p))
        .filter_map(|p| key(&p))
        .collect();
    let mut by_key: BTreeMap<String, Agg> = BTreeMap::new();
    for (path, text) in &lines {
        for name in names_in(text, &known) {
            let Some(k) = key(&name) else { continue };
            let a = by_key.entry(k).or_default();
            *a.forms.entry(name).or_default() += 1;
            a.recipes.insert(path.to_string());
        }
    }
    // the same words spaced differently ("lo han" / "lohan") join the more common one
    let mut into: HashMap<String, String> = HashMap::new();
    let mut by_compact: HashMap<String, String> = HashMap::new();
    for k in by_key.keys() {
        let first = by_compact.entry(k.replace(' ', "")).or_insert_with(|| k.clone());
        if first != k {
            let (small, big) = if by_key[k].recipes.len() > by_key[first.as_str()].recipes.len() { (first.clone(), k.clone()) } else { (k.clone(), first.clone()) };
            into.insert(small, big.clone());
            *first = big;
        }
    }
    merge(&mut by_key, &into);
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
    merge(&mut by_key, &into);
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
pub async fn route(Bk(s): Bk, headers: HeaderMap) -> Result<Response, Response> {
    let etag = s.payload.read().unwrap().etag.clone();
    let cached = s.ingredients.0.lock().unwrap().clone().filter(|c| c.0 == etag);
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
            *s.ingredients.0.lock().unwrap() = Some((etag.clone(), b.clone()));
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
    fn numbers() {
        for (s, n) in [("1", 1.0), ("1 ½", 1.5), ("1½", 1.5), ("1/2", 0.5), ("1 1/2", 1.5), ("1-1/2", 1.5), ("2 ¼ to 2 ¾", 2.25),
            ("3-4", 3.0), ("1 and 1/2", 1.5), ("1 + 1/2", 1.5), ("a", 1.0), ("One", 1.0), ("half", 0.5), ("2.5", 2.5), ("⅓", 1.0 / 3.0)] {
            assert_eq!(number(s), Some(n), "{s}");
        }
        assert_eq!(number("a few"), None);
        assert_eq!(number("several"), None);
    }

    #[test]
    fn amounts() {
        let a = |s: &str| amount(s).unwrap();
        let x = a("1 ½ cups all-purpose flour, sifted");
        assert_eq!((x.qty, x.unit.as_deref(), x.key.as_deref(), x.grams), (Some(1.5), Some("cup"), Some("all purpose flour"), None));
        let x = a("2 large eggs, beaten");
        assert_eq!((x.qty, x.unit, x.size.as_deref(), x.key.as_deref()), (Some(2.0), None, Some("large"), Some("egg")));
        // a weight in brackets: the whole amount, or per container
        let x = a("1 ½ cups (150 g) lupin flour");
        assert_eq!((x.key.as_deref(), x.grams), (Some("lupin flour"), Some(150.0)));
        assert_eq!(a("2 (15 oz) cans pumpkin").grams, Some(2.0 * 15.0 * 28.3495));
        assert_eq!(a("1 14-oz can of coconut cream").grams, Some(14.0 * 28.3495));
        // "1 - 15 oz can": one can, not 1 to 15
        let x = a("1 - 15 oz can of chickpeas");
        assert_eq!((x.qty, x.key.as_deref(), x.grams), (Some(1.0), Some("chickpea"), Some(15.0 * 28.3495)));
        assert_eq!(p("3 - 15 oz cans black beans").0, Some("3"));
        assert_eq!(a("3-4 cloves garlic").qty, Some(3.0));
        // left out of nutrition, and why
        assert_eq!(a("1 tsp MSG (optional)").skip.as_deref(), Some("optional"));
        assert_eq!(a("Lard for frying").skip.as_deref(), Some("for frying"));
        assert_eq!(a("Salt to taste").qty, None);
        assert_eq!(a("1 1/4 tsp kosher salt, plus more to taste").skip, None);
        // alternatives: the first one
        assert_eq!(a("½ cup butter or ghee").key.as_deref(), Some("butter"));
        assert_eq!(a("3 lb chicken tenderloins or chicken breast cut against the grain").key.as_deref(), Some("chicken tenderloin"));
        // measured, but naming nothing the table could know: still an ingredient, so it's listed as not counted
        assert_eq!(a("3 tbsp non-GMO, low inflammatory oil").key, None);
        assert_eq!(a("1 ½ cups milk and/or half and half").key.as_deref(), Some("milk"));
        assert!(amount("and 1/2 cup").is_none());
        assert!(amount("Mix:").is_none());
        assert!(amount("1 serving (1 slice) equals 476 calories, 26 g fat (11 g saturated fat)").is_none());
        assert!(amount("Preheat oven to 350").is_none());
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
        assert_eq!(key("A.P. flour").as_deref(), Some("all purpose flour"));
        assert_eq!(key("A P flour").as_deref(), Some("all purpose flour"));
        assert_eq!(key("Hot Dogs").as_deref(), Some("hotdog"));
        assert_eq!(key("hot sauce").as_deref(), Some("hotsauce"));
        assert_eq!(key("hot water").as_deref(), Some("water"));
        assert_eq!(key("non-fat cottage cheese").as_deref(), Some("nonfat cottage cheese"));
        assert_eq!(key("non-GMO canola oil").as_deref(), Some("canola oil"));
    }

    fn n(s: &str) -> Vec<String> {
        names(s)
    }

    /// `names` with these keys written on their own elsewhere in the vault
    fn nk(s: &str, known: &[&str]) -> Vec<String> {
        names_in(s, &known.iter().filter_map(|k| key(k)).collect())
    }

    #[test]
    fn alternatives() {
        assert_eq!(n("butter or ghee"), ["butter", "ghee"]);
        assert_eq!(n("AP flour and/or rice flour"), ["AP flour", "rice flour"]);
        assert_eq!(n("all-purpose flour or bread flour"), ["all-purpose flour", "bread flour"]);
        assert_eq!(n("Avocado or Olive oil"), ["Avocado oil", "Olive oil"]);
        assert_eq!(nk("Avocado or Olive oil", &["avocado", "avocado oil"]), ["Avocado oil", "Olive oil"]);
        assert_eq!(n("AP or Gluten Free flour"), ["AP flour", "Gluten Free flour"]);
        assert_eq!(nk("allulose or powdered monk fruit", &["allulose"]), ["allulose", "powdered monk fruit"]);
        assert_eq!(nk("Allulose/monk fruit", &["allulose"]), ["Allulose", "monk fruit"]);
        assert_eq!(n("beef or chicken stock/broth"), ["beef stock", "chicken stock", "broth"]);
        assert_eq!(n("chicken and/or beef broth"), ["chicken broth", "beef broth"]);
        assert_eq!(n("brown rice or tapioca syrup"), ["brown rice", "tapioca syrup"]);
        assert_eq!(nk("brown rice or tapioca syrup", &["brown rice syrup"]), ["brown rice syrup", "tapioca syrup"]);
        assert_eq!(n("caesar salad dressing or pesto sauce"), ["caesar salad dressing", "pesto sauce"]);
        assert_eq!(n("Beaten egg and/or Butter"), ["Beaten egg", "Butter"]);
        assert_eq!(n("honey or substitute stevia"), ["honey", "stevia"]);
        assert_eq!(n("cooled gelatin or 1 egg"), ["cooled gelatin", "egg"]);
        assert_eq!(n("Garlic Powder or 2 garlic cloves"), ["Garlic Powder", "garlic cloves"]);
        assert_eq!(n("egg + 1 tbsp water"), ["egg", "water"]);
        assert_eq!(n("8 cups+ blackberries or boysenberries"), ["blackberries", "boysenberries"]);
        assert_eq!(n("chicken or 8 thighs"), ["chicken", "thighs"]);
        assert_eq!(nk("salt/potassium mix", &["salt"]), ["salt", "potassium mix"]);
        assert_eq!(n("1 ½ tsp ½ tsp"), Vec::<String>::new());
        // "and" / "&" only between keys written on their own
        assert_eq!(n("Salt and pepper"), ["Salt and pepper"]);
        assert_eq!(nk("Salt and pepper", &["salt", "pepper"]), ["Salt", "pepper"]);
        assert_eq!(nk("1 ½ cups milk and/or half and half", &["milk"]), ["milk", "half and half"]);
    }

    #[test]
    fn leading_fragments() {
        assert_eq!(n("+ 1 tbsp milk"), ["milk"]);
        assert_eq!(n("+ 1 tsp white granulated sugar"), ["white granulated sugar"]);
        assert_eq!(n("+ 2 tbsp Non-gmo corn starch"), ["Non-gmo corn starch"]);
        assert_eq!(n("+ cups of water"), ["water"]);
        assert_eq!(n("+ teaspoon Oil flavor\\ & color"), ["Oil flavor & color"]);
        assert_eq!(n("+ Teaspoon Or more citric acid\\\\"), ["citric acid"]);
        assert_eq!(n("1 + Teaspoon Or more citric acid"), ["citric acid"]);
        assert_eq!(n("12 + 12 Green (Anaheim) chilies"), ["Green"]);
        assert_eq!(n("- cup milk"), ["milk"]);
        assert_eq!(n("3/4 - cup milk"), ["milk"]);
        assert_eq!(n("-1/2 tsp taco seasoning"), ["taco seasoning"]);
        assert_eq!(n("/2 teaspoon Baking Soda"), ["Baking Soda"]);
        assert_eq!(n("and 1/2 cup"), Vec::<String>::new());
        assert_eq!(n("and 2 tbsp warm water"), ["warm water"]);
        assert_eq!(n("1 cup and 2 tbsp warm water"), ["warm water"]);
        assert_eq!(n("½ tsp to 1 tsp sugar (cuts acidity)"), ["sugar"]);
        assert_eq!(n("2 tbsp – 4 tbsp Cumin"), ["Cumin"]);
        assert_eq!(n("An additional 1/8 tsp salt"), ["salt"]);
        assert_eq!(n("less 3 tbsp AP flour"), ["AP flour"]);
        assert_eq!(n("cup+ lard"), ["lard"]);
        assert_eq!(n("3 cups+ frying oil"), ["frying oil"]);
        assert_eq!(n("I lb Italian sausage, drain some of the fat"), ["Italian sausage"]);
        assert_eq!(n("¼ slight tsp salt"), ["salt"]);
        assert_eq!(n("2 Tbp honey"), ["honey"]);
        assert_eq!(n("Optional: 1/2 cup chopped walnuts"), ["chopped walnuts"]);
        assert_eq!(n("Optional: ½ tsp vanilla"), ["vanilla"]);
        assert_eq!(n("Garnish: Bacon bits, sour cream"), ["Bacon bits"]);
        assert_eq!(n("hot water 105-115˚F"), ["hot water"]);
        assert_eq!(n("1 cup water at 85°F"), ["water"]);
        assert_eq!(n("Orange Zest from 1 orange"), ["Orange Zest"]);
        assert_eq!(n("Meat from 1 Rotisserie Chicken"), ["Meat"]);
        assert_eq!(p("hot water 105-115˚F").3.as_deref(), Some("105-115˚F"));
    }

    #[test]
    fn not_ingredients() {
        for s in [
            // directions
            "Add pasta and cook", "Add salt", "Bake", "Bake uncovered 35 to 40 minutes", "Cook", "Fold and proof",
            "Gently fold and proof", "Mix starter", "Pour the mix in", "Preheat oven to 350", "Preheat the oven to 325 degrees",
            "Preheat your oven to 400°F", "Serve", "Serve over pasta with grated mozzarella", "Stir in the starter and proof",
            "Whisk all ingredients into an oversized", "In large bowl", "In a small saucepan", "On Day 1",
            // labels and suggestions
            "Day 1: Preparing the Starter", "Makes 3 cups", "Try adding RED FOOD COLORING", "Try adding sacha inchi",
            "Try substituting ¼ cup psyllium", "\\* Best blueberries are organic and small",
            // a nutrition statement written among the ingredients
            "1 serving (1 slice) equals 476 calories, 26 g fat (11 g saturated fat)",
            // equipment
            "scissors", "mixing bowl and spoon", "microwave-safe bowl", "medium-size d mixing bowl", "whisk or hand mixer",
            "measuring cup and spoons", "wooden spoon", "whisk",
            // a number left in the name
            "to 1 tsp sugar",
        ] {
            assert_eq!(n(s), Vec::<String>::new(), "{s}");
        }
    }

    #[test]
    fn real_ingredients_stay() {
        for s in ["Kodiak buttermilk mix", "Dry seasoning mix", "Oil blend", "Romo House Blend", "1 cup half and half", "macaroni and cheese", "Oil flavor & color", "Whisked egg", "cooking spray", "mixed greens"] {
            assert_eq!(nk(s, &["cheese", "oil flavor", "egg"]), [s.trim_start_matches("1 cup ")], "{s}");
        }
    }

    /// `VAULT=~/Recipes cargo test -p recipes-core -- --ignored`
    #[test]
    #[ignore = "needs VAULT"]
    fn vault_catalogue() {
        let path = std::env::var("VAULT").expect("set VAULT to a recipes folder, e.g. VAULT=~/Recipes");
        let root = std::path::Path::new(&path);
        let idx = crate::vault::scan(root, None, None);
        let cat = catalogue(&idx);
        let egg = cat.iter().find(|i| i.key == "egg").expect("egg");
        assert!(egg.count >= 40, "eggs in {} recipes", egg.count);
        assert!(cat.iter().all(|i| !i.name.is_empty()));
        for i in &cat {
            let lower = i.name.to_lowercase();
            assert!(!i.name.contains(|c: char| c.is_ascii_digit()), "digit: {}", i.name);
            assert!(!lower.starts_with(['+', '-', '/']) && !lower.starts_with("and "), "fragment: {}", i.name);
            assert!(!lower.contains(" or ") && !lower.contains("and/or"), "alternatives: {}", i.name);
            let first = lower.split_whitespace().next().unwrap_or_default();
            assert!(!["try", "preheat", "add", "bake", "serve"].contains(&first), "direction: {}", i.name);
            assert!(!equipment(&i.name), "equipment: {}", i.name);
        }
        let mut compact = HashSet::new();
        assert!(cat.iter().all(|i| compact.insert(i.key.replace(' ', ""))), "keys equal but for spaces");
        eprintln!("{} ingredients; most used: {:?}", cat.len(), {
            let mut v: Vec<_> = cat.iter().map(|i| (i.count, i.name.as_str())).collect();
            v.sort_by(|a, b| b.0.cmp(&a.0));
            v.into_iter().take(15).collect::<Vec<_>>()
        });
    }
}
