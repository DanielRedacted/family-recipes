//! Tolerant reader for schema.org `Recipe` data (JSON-LD, microdata folded into the same shape,
//! and WP Recipe Maker's REST JSON).
//!
//! Properties read (schema.org Recipe, plus what it inherits from HowTo / CreativeWork / Thing):
//!
//! | property | shapes accepted |
//! |---|---|
//! | name, description | Text, array (first non-empty), object with name/text/@value |
//! | author | Text, Person/Organization, array of either |
//! | recipeYield / yield | Number, Text, QuantitativeValue (value + unitText), array (prefers the entry with words) |
//! | prepTime, cookTime, totalTime, performTime | ISO 8601 duration (P…W…DT…H…M…S), or free text kept as written |
//! | recipeIngredient / ingredients | Text (comma list), array, ItemList / ListItem |
//! | recipeInstructions / step | Text (split on line breaks / list items), array, HowToStep, HowToSection (nested), ItemList / ListItem, CreativeWork |
//! | image, thumbnailUrl, video.thumbnailUrl | URL, ImageObject (url / contentUrl / @id), array |
//! | nutrition | NutritionInformation: calories, carbohydrateContent, cholesterolContent, fatContent, fiberContent, proteinContent, saturatedFatContent, servingSize, sodiumContent, sugarContent, transFatContent, unsaturatedFatContent — numbers or text |
//! | tool, supply | Text, HowToTool / HowToSupply, array → equipment |
//! | recipeCategory, recipeCuisine, keywords, cookingMethod, suitableForDiet | Text (comma list), array, DefinedTerm, RestrictedDiet URL → tags |
//!
//! Ignored on purpose: aggregateRating, review, datePublished, estimatedCost.
//! References: https://schema.org/Recipe, https://schema.org/NutritionInformation,
//! https://developers.google.com/search/docs/appearance/structured-data/recipe

use crate::extract::{Candidate, Rec, clean};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub fn is_recipe(v: &Value) -> bool {
    let t = |s: &str| s.rsplit(['/', ':']).next() == Some("Recipe");
    match v.get("@type") {
        Some(Value::String(s)) => t(s),
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str().is_some_and(t)),
        _ => false,
    }
}

pub fn find_recipe(v: &Value, depth: usize) -> Option<&Value> {
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

pub fn text(v: &Value) -> String {
    match v {
        Value::String(s) => clean(s),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.iter().map(text).find(|s| !s.is_empty()).unwrap_or_default(),
        Value::Object(o) => o.get("name").or(o.get("text")).or(o.get("@value")).or(o.get("item")).map(text).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Text list: arrays, ItemList/ListItem, or one comma-separated string.
pub fn list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().flat_map(|x| match x {
            Value::Object(o) if o.contains_key("itemListElement") => list(o.get("itemListElement")),
            _ => vec![text(x)],
        }).filter(|s| !s.is_empty()).collect(),
        Some(Value::String(s)) => s.split(',').map(clean).filter(|s| !s.is_empty()).collect(),
        Some(Value::Object(o)) if o.contains_key("itemListElement") => list(o.get("itemListElement")),
        Some(x) => vec![text(x)].into_iter().filter(|s| !s.is_empty()).collect(),
        None => vec![],
    }
}

/// Ingredient lists are never split on commas ("1 onion, diced").
fn lines(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => clean_lines(&s.replace("</li>", "\n").replace("</p>", "\n")),
        other => list(other),
    }
}

/// PT1H20M / P1DT2H / P1W -> "1 day 2 hr" style human text; anything else is kept as written.
pub fn iso_duration(s: &str) -> String {
    static RX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^P(?:(\d+(?:\.\d+)?)W)?(?:(\d+(?:\.\d+)?)D)?(?:T(?:(\d+(?:\.\d+)?)H)?(?:(\d+(?:\.\d+)?)M)?(?:(\d+(?:\.\d+)?)S)?)?$").unwrap()
    });
    let Some(c) = RX.captures(s.trim()) else { return clean(s) };
    let n = |i: usize| c.get(i).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
    minutes_text((n(1) * 10080.0 + n(2) * 1440.0 + n(3) * 60.0 + n(4) + n(5) / 60.0).round() as i64)
}

pub fn minutes_text(total: i64) -> String {
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

pub type Groups = Vec<(Option<String>, Vec<String>)>;

pub fn push_item(out: &mut Groups, g: &Option<String>, s: String) {
    if s.is_empty() {
        return;
    }
    match out.last_mut() {
        Some((lg, items)) if lg == g => items.push(s),
        _ => out.push((g.clone(), vec![s])),
    }
}

pub fn instructions(v: Option<&Value>, out: &mut Groups, images: &mut Vec<Candidate>) {
    fn walk(v: &Value, group: &Option<String>, out: &mut Groups, images: &mut Vec<Candidate>, depth: usize) {
        if depth > 8 {
            return;
        }
        match v {
            Value::String(s) => {
                for line in clean_lines(&s.replace("</li>", "\n").replace("</p>", "\n")) {
                    push_item(out, group, line);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, group, out, images, depth + 1)),
            Value::Object(o) => {
                let ty = o.get("@type").map(text).unwrap_or_default();
                if ty.ends_with("HowToSection") || ty.ends_with("ItemList") || (o.contains_key("itemListElement") && !o.contains_key("text")) {
                    let name = o.get("name").map(text).filter(|s| !s.is_empty() && !ty.ends_with("ItemList"));
                    if let Some(items) = o.get("itemListElement") {
                        walk(items, &name.or(group.clone()), out, images, depth + 1);
                    }
                } else if ty.ends_with("ListItem") && !o.contains_key("text") {
                    if let Some(item) = o.get("item") {
                        walk(item, group, out, images, depth + 1);
                    }
                } else {
                    let t = o.get("text").map(text).filter(|s| !s.is_empty()).or_else(|| o.get("name").map(text)).unwrap_or_default();
                    if let Some(img) = o.get("image") {
                        for u in image_urls(img) {
                            images.push(Candidate { url: u, alt: String::new(), kind: "step".into(), step: Some(t.chars().take(60).collect()) });
                        }
                    }
                    push_item(out, group, t);
                }
            }
            _ => {}
        }
    }
    if let Some(v) = v {
        walk(v, &None, out, images, 0);
    }
}

pub fn clean_lines(s: &str) -> Vec<String> {
    static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
    let frag = scraper::Html::parse_fragment(&BR.replace_all(s, "\n"));
    let t: String = frag.root_element().text().collect();
    static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\u{a0}]+").unwrap());
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

/// schema.org NutritionInformation property -> (our key, unit added to bare numbers).
const NUTRITION: &[(&str, &str, &str)] = &[
    ("servingSize", "serving_size", ""),
    ("calories", "calories", "kcal"),
    ("carbohydrateContent", "carbohydrates", "g"),
    ("proteinContent", "protein", "g"),
    ("fatContent", "fat", "g"),
    ("saturatedFatContent", "saturated_fat", "g"),
    ("unsaturatedFatContent", "unsaturated_fat", "g"),
    ("transFatContent", "trans_fat", "g"),
    ("cholesterolContent", "cholesterol", "mg"),
    ("sodiumContent", "sodium", "mg"),
    ("fiberContent", "fiber", "g"),
    ("sugarContent", "sugar", "g"),
];

/// A nutrition value as written; bare numbers get the property's usual unit.
pub fn nutrient(v: &Value, unit: &str) -> Option<String> {
    let s = match v {
        Value::Number(n) => n.to_string(),
        Value::String(_) | Value::Object(_) | Value::Array(_) => text(v),
        _ => return None,
    };
    let s = s.trim().to_string();
    if s.is_empty() {
        return None;
    }
    let bare = s.parse::<f64>().is_ok();
    Some(if bare && !unit.is_empty() { format!("{s} {unit}") } else { s })
}

fn nutrition(v: Option<&Value>) -> Vec<(String, String)> {
    let Some(Value::Object(o)) = v else { return vec![] };
    NUTRITION.iter().filter_map(|(k, ours, unit)| o.get(*k).and_then(|x| nutrient(x, unit)).map(|s| (ours.to_string(), s))).collect()
}

/// "https://schema.org/GlutenFreeDiet" -> "gluten-free"
fn diet(s: &str) -> String {
    let seg = s.rsplit(['/', ':']).next().unwrap_or(s).trim_end_matches("Diet");
    let mut out = String::new();
    for (i, c) in seg.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Lowercase, de-duplicated (order kept), at most 12.
pub fn add_tags(tags: &mut Vec<String>, new: impl IntoIterator<Item = String>) {
    for t in new {
        let t = t.trim().trim_start_matches('#').replace(['’', '‘'], "'").to_lowercase();
        if !t.is_empty() && t.chars().count() <= 30 && !tags.contains(&t) && tags.len() < 12 {
            tags.push(t);
        }
    }
}

pub fn schema_to_rec(r: &Value, images: &mut Vec<Candidate>) -> Rec {
    let mut rec = Rec { name: r.get("name").map(text).unwrap_or_default(), description: r.get("description").map(text).unwrap_or_default(), ..Default::default() };
    rec.author = match r.get("author") {
        Some(Value::Array(a)) => a.iter().map(text).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
        Some(x) => text(x),
        None => String::new(),
    };
    let one_yield = |v: &Value| match v {
        Value::Object(o) if o.contains_key("value") => {
            format!("{} {}", o.get("value").map(text).unwrap_or_default(), o.get("unitText").map(text).unwrap_or_default()).trim().to_string()
        }
        _ => text(v),
    };
    rec.yield_ = match r.get("recipeYield").or(r.get("yield")) {
        // ["48", "4 dozen cookies"] -> prefer the descriptive one
        Some(Value::Array(a)) => {
            let all: Vec<String> = a.iter().map(one_yield).filter(|s| !s.is_empty()).collect();
            all.iter().find(|s| s.chars().any(|c| c.is_alphabetic())).or(all.first()).cloned().unwrap_or_default()
        }
        Some(x) => one_yield(x),
        None => String::new(),
    };
    let dur = |k: &str| r.get(k).map(|v| iso_duration(&text(v))).unwrap_or_default();
    rec.prep = dur("prepTime");
    rec.cook = dur("cookTime");
    rec.total = dur("totalTime");
    if rec.prep.is_empty() {
        rec.prep = dur("performTime");
    }
    let mut tags = vec![];
    add_tags(&mut tags, list(r.get("recipeCategory")));
    add_tags(&mut tags, list(r.get("recipeCuisine")));
    add_tags(&mut tags, list(r.get("cookingMethod")));
    add_tags(&mut tags, list(r.get("suitableForDiet")).iter().map(|d| diet(d)));
    add_tags(&mut tags, list(r.get("keywords")));
    rec.tags = tags;
    let ings = lines(r.get("recipeIngredient").or(r.get("ingredients")));
    if !ings.is_empty() {
        rec.ingredients.push((None, ings));
    }
    instructions(r.get("recipeInstructions").or(r.get("step")), &mut rec.steps, images);
    for k in ["tool", "supply"] {
        for t in lines(r.get(k)) {
            if !rec.equipment.contains(&t) {
                rec.equipment.push(t);
            }
        }
    }
    rec.nutrition = nutrition(r.get("nutrition"));
    if let Some(img) = r.get("image").or(r.get("thumbnailUrl")) {
        for u in image_urls(img) {
            images.insert(images.iter().filter(|c| c.kind == "recipe").count(), Candidate { url: u, alt: rec.name.clone(), kind: "recipe".into(), step: None });
        }
    }
    if let Some(t) = r.get("video").and_then(|v| v.get("thumbnailUrl")) {
        for u in image_urls(t) {
            images.push(Candidate { url: u, alt: "video".into(), kind: "page".into(), step: None });
        }
    }
    rec
}

// ---------- WP Recipe Maker REST (`/wp-json/wp/v2/wprm_recipe/{id}` → `recipe`) ----------

/// WPRM nutrition keys -> unit for bare numbers (WPRM's own label units).
const WPRM_NUTRITION: &[(&str, &str)] = &[
    ("calories", "kcal"),
    ("carbohydrates", "g"),
    ("protein", "g"),
    ("fat", "g"),
    ("saturated_fat", "g"),
    ("polyunsaturated_fat", "g"),
    ("monounsaturated_fat", "g"),
    ("trans_fat", "g"),
    ("cholesterol", "mg"),
    ("sodium", "mg"),
    ("potassium", "mg"),
    ("fiber", "g"),
    ("sugar", "g"),
    ("vitamin_a", "IU"),
    ("vitamin_c", "mg"),
    ("calcium", "mg"),
    ("iron", "mg"),
];

fn num_or_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => clean(s),
        _ => String::new(),
    }
}

fn minutes(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0).round() as i64,
        Some(Value::String(s)) => s.trim().parse::<f64>().map(|f| f.round() as i64).unwrap_or(0),
        _ => 0,
    }
}

pub fn wprm_nutrition_unit(key: &str) -> &'static str {
    WPRM_NUTRITION.iter().find(|(k, _)| *k == key).map(|x| x.1).unwrap_or("")
}

pub fn wprm_rest(r: &Value, images: &mut Vec<Candidate>) -> Option<Rec> {
    let name = num_or_text(r.get("name"));
    if name.is_empty() {
        return None;
    }
    let mut rec = Rec { name, description: num_or_text(r.get("summary")), ..Default::default() };
    if r.get("author_display").and_then(Value::as_str) != Some("disabled") {
        rec.author = num_or_text(r.get("author_name"));
    }
    let servings = num_or_text(r.get("servings"));
    if !servings.is_empty() && servings != "0" {
        rec.yield_ = format!("{servings} {}", num_or_text(r.get("servings_unit"))).trim().to_string();
    }
    rec.prep = minutes_text(minutes(r.get("prep_time")));
    rec.cook = minutes_text(minutes(r.get("cook_time")));
    rec.total = minutes_text(minutes(r.get("total_time")));
    let custom = minutes_text(minutes(r.get("custom_time")));

    let ing = |i: &Value| -> String {
        if let Some(raw) = i.get("raw").and_then(Value::as_str) {
            return clean(raw);
        }
        let notes = num_or_text(i.get("notes"));
        let main = ["amount", "unit", "name"].iter().map(|k| num_or_text(i.get(*k))).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        match notes.as_str() {
            "" => main,
            n if n.starts_with(',') => format!("{main}{n}"),
            n if n.starts_with('(') => format!("{main} {n}"),
            n => format!("{main}, {n}"),
        }
    };
    rec.ingredients = wprm_groups(r, "ingredients", "ingredients_flat", "ingredient", &mut |i| ing(i));
    rec.steps = wprm_groups(r, "instructions", "instructions_flat", "instruction", &mut |i| {
        let t = num_or_text(i.get("text"));
        let t = if t.is_empty() { num_or_text(i.get("name")) } else { t };
        if let Some(u) = i.get("image_url").and_then(Value::as_str).filter(|u| !u.is_empty()) {
            images.push(Candidate { url: u.into(), alt: String::new(), kind: "step".into(), step: Some(t.chars().take(60).collect()) });
        }
        t
    });

    if let Some(n) = r.get("notes").and_then(Value::as_str) {
        rec.notes = clean_lines(&n.replace("</p>", "\n").replace("</li>", "\n"));
    }
    if !custom.is_empty() {
        let label = num_or_text(r.get("custom_time_label"));
        rec.notes.push(format!("{}: {custom}", if label.is_empty() { "Other time" } else { &label }));
    }
    for e in r.get("equipment").and_then(Value::as_array).into_iter().flatten() {
        let s = ["amount", "name", "notes"].iter().map(|k| num_or_text(e.get(*k))).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        if !s.is_empty() && !rec.equipment.contains(&s) {
            rec.equipment.push(s);
        }
    }
    if let Some(Value::Object(n)) = r.get("nutrition") {
        let size = num_or_text(n.get("serving_size"));
        if !size.is_empty() && size != "0" {
            rec.nutrition.push(("serving_size".into(), format!("{size} {}", num_or_text(n.get("serving_unit"))).trim().to_string()));
        }
        for (k, unit) in WPRM_NUTRITION {
            if let Some(s) = n.get(*k).and_then(|v| match v {
                Value::String(s) if s.trim().is_empty() => None,
                v => nutrient(v, unit),
            }) {
                rec.nutrition.push((k.to_string(), s));
            }
        }
    }
    if let Some(Value::Object(t)) = r.get("tags") {
        for terms in t.values() {
            add_tags(&mut rec.tags, terms.as_array().into_iter().flatten().map(text));
        }
    }
    if let Some(u) = r.get("image_url").and_then(Value::as_str).filter(|u| !u.is_empty()) {
        images.insert(0, Candidate { url: u.into(), alt: rec.name.clone(), kind: "recipe".into(), step: None });
    }
    Some(rec)
}

/// WPRM lists come grouped (`ingredients: [{name, ingredients: [...]}]`) or flat with group
/// markers (`ingredients_flat: [{type: "group"}, {type: "ingredient"}]`).
fn wprm_groups(r: &Value, key: &str, flat_key: &str, item_type: &str, f: &mut dyn FnMut(&Value) -> String) -> Groups {
    let mut out: Groups = vec![];
    if let Some(Value::Array(groups)) = r.get(key) {
        for g in groups {
            let gname = Some(num_or_text(g.get("name"))).filter(|s| !s.is_empty());
            for i in g.get(key).and_then(Value::as_array).into_iter().flatten() {
                push_item(&mut out, &gname, f(i));
            }
        }
    } else if let Some(Value::Array(flat)) = r.get(flat_key) {
        let mut gname = None;
        for i in flat {
            match i.get("type").and_then(Value::as_str) {
                Some("group") => gname = Some(num_or_text(i.get("name"))).filter(|s| !s.is_empty()),
                Some(t) if t == item_type => push_item(&mut out, &gname, f(i)),
                _ => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shapes() {
        let r = json!({
            "@type": "Recipe", "name": "Soup",
            "recipeYield": {"@type": "QuantitativeValue", "value": 6, "unitText": "bowls"},
            "recipeIngredient": {"@type": "ItemList", "itemListElement": [{"@type": "ListItem", "name": "2 carrots, diced"}, {"@type": "ListItem", "name": "1 onion"}]},
            "recipeInstructions": {"@type": "ItemList", "itemListElement": [
                {"@type": "ListItem", "item": {"@type": "HowToStep", "text": "Chop."}},
                {"@type": "HowToSection", "name": "Finish", "itemListElement": [{"@type": "HowToStep", "name": "Simmer."}]}]},
            "nutrition": {"@type": "NutritionInformation", "calories": 320, "fatContent": "12 g", "sodiumContent": "450", "servingSize": "1 bowl"},
            "tool": ["Stock pot", {"@type": "HowToTool", "name": "Ladle"}], "supply": "Ladle",
            "suitableForDiet": "https://schema.org/GlutenFreeDiet", "keywords": "easy, Soup, weeknight", "recipeCategory": ["Soup"],
            "cookTime": "PT1H30M", "totalTime": "P1W"
        });
        let mut imgs = vec![];
        let rec = schema_to_rec(&r, &mut imgs);
        assert_eq!(rec.yield_, "6 bowls");
        assert_eq!(rec.ingredients[0].1, vec!["2 carrots, diced", "1 onion"]);
        assert_eq!(rec.steps, vec![(None, vec!["Chop.".to_string()]), (Some("Finish".into()), vec!["Simmer.".to_string()])]);
        assert_eq!(rec.equipment, vec!["Stock pot", "Ladle"]);
        assert_eq!(rec.tags, vec!["soup", "gluten-free", "easy", "weeknight"]);
        assert_eq!(rec.cook, "1 hr 30 min");
        assert_eq!(rec.total, "7 days");
        let n: Vec<(&str, &str)> = rec.nutrition.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        assert_eq!(n, vec![("serving_size", "1 bowl"), ("calories", "320 kcal"), ("fat", "12 g"), ("sodium", "450 mg")]);
    }

    #[test]
    fn ingredients_text_block_not_split_on_commas() {
        let r = json!({"@type": "Recipe", "name": "x", "recipeIngredient": "1 onion, diced<br>2 eggs"});
        let rec = schema_to_rec(&r, &mut vec![]);
        assert_eq!(rec.ingredients[0].1, vec!["1 onion, diced", "2 eggs"]);
    }

    #[test]
    fn wprm_rest_payload() {
        let v: Value = serde_json::from_str(include_str!("../tests/fixtures/wprm-rest.json")).unwrap();
        let mut imgs = vec![];
        let rec = wprm_rest(&v["recipe"], &mut imgs).unwrap();
        assert_eq!(rec.name, "Classic Pancakes");
        assert_eq!(rec.yield_, "4 people");
        assert_eq!(rec.prep, "5 min");
        assert_eq!(rec.total, "40 min");
        assert_eq!(rec.ingredients[0].0.as_deref(), Some("Batter"));
        assert_eq!(rec.ingredients[0].1, vec!["1 cup flour, sifted", "2 large eggs", "1 cup onion, diced"]);
        assert_eq!(rec.steps[1].0.as_deref(), Some("Cook"));
        assert_eq!(rec.steps[1].1, vec!["Heat the pan.", "Fry until golden."]);
        assert_eq!(rec.equipment, vec!["1 Rolling Pin (optional)", "Skillet"]);
        assert!(rec.nutrition.contains(&("calories".into(), "231 kcal".into())));
        assert!(rec.nutrition.contains(&("serving_size".into(), "123 g".into())));
        assert!(!rec.nutrition.iter().any(|(k, _)| k == "iron"));
        assert_eq!(rec.tags, vec!["breakfast", "american", "easy"]);
        assert!(rec.notes.iter().any(|n| n == "Resting Time: 10 min"));
        assert_eq!(imgs[0].url, "https://ex.com/pancakes.jpg");
        assert!(imgs.iter().any(|c| c.kind == "step"));
    }
}
