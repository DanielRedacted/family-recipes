//! YAML frontmatter ↔ typed struct, preserving unknown keys verbatim and emitting a
//! deterministic canonical key order (per SPEC "strict out").

use crate::error::{AppError, Result};
use serde_yaml::{Mapping, Value};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub title: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub author: Option<String>,
    pub created: Option<String>,
    pub date_published: Option<String>,
    pub prep_time: Option<String>,
    pub cook_time: Option<String>,
    pub total_time: Option<String>,
    pub perform_time: Option<String>,
    pub yield_: Option<String>,
    pub cooking_method: Option<String>,
    pub cuisine: Option<String>,
    pub tags: Vec<String>,
    pub diet: Vec<String>,
    pub rating: Option<Value>,
    pub nutrition: Option<Value>,
    pub image: Option<String>,
    pub image_source: Option<String>,
    pub video: Option<String>,
    pub estimated_cost: Option<String>,
    pub emoji: Option<String>,
    pub color: Option<String>,
    pub needs_review: Option<bool>,
    /// Keys the app doesn't own, preserved verbatim and re-emitted last (original order).
    pub extra: Vec<(String, Value)>,
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn value_to_list(v: &Value) -> Vec<String> {
    match v {
        Value::Sequence(seq) => seq.iter().filter_map(value_to_string).collect(),
        Value::String(s) => s
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        _ => vec![],
    }
}

fn value_to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

pub fn parse(yaml: &str) -> Result<Frontmatter> {
    let map: Mapping = serde_yaml::from_str(yaml)?;
    let mut fm = Frontmatter::default();
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        match key {
            "title" => fm.title = value_to_string(&v).unwrap_or_default(),
            "description" => fm.description = value_to_string(&v),
            "source" => fm.source = value_to_string(&v),
            "author" => fm.author = value_to_string(&v),
            "created" => fm.created = value_to_string(&v),
            "date_published" => fm.date_published = value_to_string(&v),
            "prep_time" => fm.prep_time = value_to_string(&v),
            "cook_time" => fm.cook_time = value_to_string(&v),
            "total_time" => fm.total_time = value_to_string(&v),
            "perform_time" => fm.perform_time = value_to_string(&v),
            "yield" => fm.yield_ = value_to_string(&v),
            "cooking_method" => fm.cooking_method = value_to_string(&v),
            "cuisine" => fm.cuisine = value_to_string(&v),
            "tags" => fm.tags = value_to_list(&v),
            "diet" => fm.diet = value_to_list(&v),
            "rating" => fm.rating = Some(v),
            "nutrition" => fm.nutrition = Some(v),
            "image" => fm.image = value_to_string(&v),
            "image_source" => fm.image_source = value_to_string(&v),
            "video" => fm.video = value_to_string(&v),
            "estimated_cost" => fm.estimated_cost = value_to_string(&v),
            "emoji" => fm.emoji = value_to_string(&v),
            "color" => fm.color = value_to_string(&v),
            "needs_review" => fm.needs_review = value_to_bool(&v),
            _ => fm.extra.push((key.to_string(), v)),
        }
    }
    if fm.title.is_empty() {
        return Err(AppError::Parse("recipe frontmatter missing `title`".into()));
    }
    Ok(fm)
}

pub fn emit(fm: &Frontmatter) -> Result<String> {
    let mut m = Mapping::new();
    let key = |k: &str| Value::String(k.to_string());
    let sval = |s: &str| Value::String(s.to_string());

    m.insert(key("title"), sval(&fm.title));
    macro_rules! put {
        ($k:expr, $o:expr) => {
            if let Some(v) = &$o {
                m.insert(key($k), sval(v));
            }
        };
    }
    put!("description", fm.description);
    put!("source", fm.source);
    put!("author", fm.author);
    put!("created", fm.created);
    put!("date_published", fm.date_published);
    put!("prep_time", fm.prep_time);
    put!("cook_time", fm.cook_time);
    put!("total_time", fm.total_time);
    put!("perform_time", fm.perform_time);
    put!("yield", fm.yield_);
    put!("cooking_method", fm.cooking_method);
    put!("cuisine", fm.cuisine);
    if !fm.tags.is_empty() {
        m.insert(key("tags"), Value::Sequence(fm.tags.iter().map(|t| sval(t)).collect()));
    }
    if !fm.diet.is_empty() {
        m.insert(key("diet"), Value::Sequence(fm.diet.iter().map(|t| sval(t)).collect()));
    }
    if let Some(r) = &fm.rating {
        m.insert(key("rating"), r.clone());
    }
    if let Some(n) = &fm.nutrition {
        m.insert(key("nutrition"), n.clone());
    }
    put!("image", fm.image);
    put!("image_source", fm.image_source);
    put!("video", fm.video);
    put!("estimated_cost", fm.estimated_cost);
    put!("emoji", fm.emoji);
    put!("color", fm.color);
    if let Some(b) = fm.needs_review {
        m.insert(key("needs_review"), Value::Bool(b));
    }
    for (k, v) in &fm.extra {
        m.insert(key(k), v.clone());
    }

    let yaml = serde_yaml::to_string(&m)?;
    Ok(format!("---\n{yaml}---\n"))
}
