//! Deterministic schema.org Recipe JSON-LD **export** (import is P6). Validated against
//! Google's Recipe docs + schema.org: `recipeInstructions` as HowToStep/HowToSection,
//! `keywords` never duplicating `recipeCategory`/`recipeCuisine`, diet prefixes restored.

use super::{Body, Instruction, Recipe};
use serde_json::{json, Map, Value};

/// Strip `[[wiki-links]]`, keeping display text (`[[a|b]]` → `b`, `[[a]]` → `a`).
pub fn strip_wikilinks(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("]]") {
            Some(end) => {
                let inner = &after[..end];
                out.push_str(inner.rsplit('|').next().unwrap_or(inner));
                rest = &after[end + 2..];
            }
            None => {
                out.push_str("[[");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_group_label(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("**") && t.ends_with("**") && t.len() > 4
}

fn recipe_ingredients(body: &Body) -> Vec<Value> {
    body.ingredients
        .iter()
        .filter(|l| !is_group_label(l))
        .map(|l| Value::String(strip_wikilinks(l)))
        .collect()
}

fn how_to_step(text: &str) -> Value {
    json!({ "@type": "HowToStep", "text": strip_wikilinks(text) })
}

fn recipe_instructions(body: &Body) -> Vec<Value> {
    let has_sections = body
        .instructions
        .iter()
        .any(|i| matches!(i, Instruction::Section(_)));
    if !has_sections {
        return body
            .instructions
            .iter()
            .filter_map(|i| match i {
                Instruction::Step(t) => Some(how_to_step(t)),
                Instruction::Section(_) => None,
            })
            .collect();
    }
    // Group steps under their section; leading steps stay top-level HowToSteps.
    let mut out: Vec<Value> = Vec::new();
    let mut cur_name: Option<String> = None;
    let mut cur_steps: Vec<Value> = Vec::new();
    let flush = |out: &mut Vec<Value>, name: &mut Option<String>, steps: &mut Vec<Value>| {
        if let Some(n) = name.take() {
            out.push(json!({
                "@type": "HowToSection",
                "name": n,
                "itemListElement": std::mem::take(steps),
            }));
        }
    };
    for item in &body.instructions {
        match item {
            Instruction::Section(name) => {
                flush(&mut out, &mut cur_name, &mut cur_steps);
                cur_name = Some(strip_wikilinks(name));
            }
            Instruction::Step(t) => {
                if cur_name.is_some() {
                    cur_steps.push(how_to_step(t));
                } else {
                    out.push(how_to_step(t));
                }
            }
        }
    }
    flush(&mut out, &mut cur_name, &mut cur_steps);
    out
}

pub fn to_jsonld(recipe: &Recipe, category: Option<&str>) -> Value {
    let fm = &recipe.frontmatter;
    let mut m = Map::new();
    m.insert("@context".into(), json!("https://schema.org"));
    m.insert("@type".into(), json!("Recipe"));
    m.insert("name".into(), json!(fm.title));

    macro_rules! put {
        ($k:expr, $v:expr) => {
            if let Some(s) = &$v {
                m.insert($k.into(), json!(s));
            }
        };
    }
    put!("description", fm.description);
    put!("url", fm.source);
    put!("datePublished", fm.date_published);
    put!("prepTime", fm.prep_time);
    put!("cookTime", fm.cook_time);
    put!("totalTime", fm.total_time);
    put!("performTime", fm.perform_time);
    put!("recipeYield", fm.yield_);
    put!("cookingMethod", fm.cooking_method);
    put!("recipeCuisine", fm.cuisine);
    put!("estimatedCost", fm.estimated_cost);

    if let Some(author) = &fm.author {
        m.insert("author".into(), json!({ "@type": "Person", "name": author }));
    }
    if let Some(cat) = category {
        m.insert("recipeCategory".into(), json!(cat));
    }

    // keywords must NOT duplicate recipeCategory or recipeCuisine (Google rule).
    let banned: Vec<String> = [category.map(str::to_string), fm.cuisine.clone()]
        .into_iter()
        .flatten()
        .map(|s| s.to_lowercase())
        .collect();
    let keywords: Vec<&String> = fm
        .tags
        .iter()
        .filter(|t| !banned.contains(&t.to_lowercase()))
        .collect();
    if !keywords.is_empty() {
        let joined = keywords.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
        m.insert("keywords".into(), json!(joined));
    }

    if !fm.diet.is_empty() {
        let diets: Vec<String> = fm
            .diet
            .iter()
            .map(|d| {
                if d.starts_with("http") {
                    d.clone()
                } else {
                    format!("https://schema.org/{d}")
                }
            })
            .collect();
        m.insert("suitableForDiet".into(), json!(diets));
    }

    if let Some(rating) = &fm.rating {
        if let Ok(mut v) = serde_json::to_value(rating) {
            if let Some(obj) = v.as_object_mut() {
                let value = obj.remove("value");
                let count = obj.remove("count");
                let mut ar = Map::new();
                ar.insert("@type".into(), json!("AggregateRating"));
                if let Some(x) = value {
                    ar.insert("ratingValue".into(), x);
                }
                if let Some(x) = count {
                    ar.insert("ratingCount".into(), x);
                }
                m.insert("aggregateRating".into(), Value::Object(ar));
            }
        }
    }

    if let Some(nutrition) = &fm.nutrition {
        if let Ok(v) = serde_json::to_value(nutrition) {
            if let Some(obj) = v.as_object() {
                let mut n = Map::new();
                n.insert("@type".into(), json!("NutritionInformation"));
                for (k, val) in obj {
                    n.insert(nutrition_key(k), val.clone());
                }
                m.insert("nutrition".into(), Value::Object(n));
            }
        }
    }

    // image → image_source URL if present, else the vault-relative path.
    if let Some(url) = fm.image_source.as_ref().or(fm.image.as_ref()) {
        m.insert("image".into(), json!(url));
    }
    if let Some(video) = &fm.video {
        m.insert("video".into(), json!({ "@type": "VideoObject", "contentUrl": video }));
    }

    let ingredients = recipe_ingredients(&recipe.body);
    if !ingredients.is_empty() {
        m.insert("recipeIngredient".into(), Value::Array(ingredients));
    }
    let instructions = recipe_instructions(&recipe.body);
    if !instructions.is_empty() {
        m.insert("recipeInstructions".into(), Value::Array(instructions));
    }

    Value::Object(m)
}

fn nutrition_key(snake: &str) -> String {
    match snake {
        "calories" => "calories",
        "fat" => "fatContent",
        "saturated_fat" => "saturatedFatContent",
        "unsaturated_fat" => "unsaturatedFatContent",
        "trans_fat" => "transFatContent",
        "carbohydrate" => "carbohydrateContent",
        "fiber" => "fiberContent",
        "sugar" => "sugarContent",
        "protein" => "proteinContent",
        "sodium" => "sodiumContent",
        "cholesterol" => "cholesterolContent",
        "serving_size" => "servingSize",
        other => other,
    }
    .to_string()
}
