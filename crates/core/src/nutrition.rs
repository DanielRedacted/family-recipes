//! The nutrition table: for each ingredient key (`ingredient::key`), USDA FoodData Central's values per
//! 100 g and its household weights (1 cup, 1 large, 1 clove). Three layers, later ones winning:
//! - `data/foods.json`, shipped with the app: built by `cargo run -p recipes-core --bin foods` from the
//!   SR Legacy download and the reviewed `data/foods-map.tsv`;
//! - lookups: with a USDA API key (Settings → Nutrition), ingredients the table lacks are searched on
//!   FoodData Central in the background and kept in the data folder's `foods.json`, marked `auto`;
//! - choices made on the Ingredients page: another USDA food, or "don't count" (`null`).
//!
//! `GET /api/foods` serves the merged table; the UI does the sums (`ui/src/lib/nutrition.ts`), so
//! nothing is ever written to a recipe. Every number is USDA's.

use crate::auth::{Caller, Role};
use crate::server::{AppState, err, write_private};
use anyhow::{Result, anyhow, bail};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

const BUNDLED: &str = include_str!("../data/foods.json");
/// In the data folder: `{ "foods": { key: Food | null }, "missed": [key…] }`.
pub const FILE: &str = "foods.json";
const API: &str = "https://api.nal.usda.gov/fdc/v1";
const KEYCHAIN_SERVICE: &str = "family.recipes.desktop";
const KEYCHAIN_USER: &str = "usda-api-key";
/// Lookups per background run: two requests each, well under the key's 1,000 an hour.
const PER_RUN: usize = 150;

// ---------- the table ----------

/// A USDA food as the app uses it: `n` per 100 g, `ml` grams per millilitre, `g` grams of one item.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Food {
    pub fdc: u64,
    pub name: String,
    pub n: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml: Option<f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub g: BTreeMap<String, f64>,
    /// found by a lookup, not chosen by anyone
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto: bool,
}

/// The parts of a FoodData Central food record (download or API) the table needs.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UsdaFood {
    pub fdc_id: u64,
    pub description: String,
    #[serde(default)]
    pub food_nutrients: Vec<UsdaNutrient>,
    #[serde(default)]
    pub food_portions: Vec<UsdaPortion>,
}
#[derive(Deserialize, Debug)]
pub struct UsdaNutrient {
    pub nutrient: Option<UsdaNutrientId>,
    pub amount: Option<f64>,
}
#[derive(Deserialize, Debug)]
pub struct UsdaNutrientId {
    pub number: Option<String>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UsdaPortion {
    pub amount: Option<f64>,
    pub modifier: Option<String>,
    pub gram_weight: f64,
    pub measure_unit: Option<UsdaUnit>,
}
#[derive(Deserialize, Debug)]
pub struct UsdaUnit {
    pub name: Option<String>,
}

/// USDA nutrient numbers → the `nutrition` keys of docs/FORMAT.md, in the label's order.
const NUTRIENTS: &[(&str, &str)] = &[
    ("208", "calories"), ("204", "fat"), ("606", "saturated_fat"), ("605", "trans_fat"), ("601", "cholesterol"), ("307", "sodium"),
    ("205", "carbohydrates"), ("291", "fiber"), ("269", "sugar"), ("203", "protein"), ("328", "vitamin_d"), ("301", "calcium"),
    ("303", "iron"), ("306", "potassium"),
];
const VOLUME: &[(&str, f64)] = &[("cup", 236.588), ("tbsp", 14.787), ("tsp", 4.929), ("fl oz", 29.574)];

/// A portion's name: "medium (2-1/2" dia)", "cup, chopped"; Foundation foods put the unit apart ("cup" + "chopped").
fn portion_label(p: &UsdaPortion) -> String {
    let unit = p.measure_unit.as_ref().and_then(|u| u.name.as_deref()).filter(|u| !matches!(*u, "" | "undetermined" | "RACC"));
    let m = p.modifier.as_deref().unwrap_or("").trim();
    let s = match unit {
        Some(u) if !m.is_empty() => format!("{u}, {m}"),
        Some(u) => u.to_string(),
        None => m.to_string(),
    };
    let lower = s.to_lowercase();
    for (long, short) in [("tablespoons", "tbsp"), ("tablespoon", "tbsp"), ("teaspoons", "tsp"), ("teaspoon", "tsp"), ("cups", "cup")] {
        if let Some(rest) = lower.strip_prefix(long) {
            return format!("{short}{rest}");
        }
    }
    lower
}

/// The volume unit a portion measures ("cup, chopped" → cup), and whether it is the plain unit.
fn volume_of(label: &str) -> Option<(f64, bool)> {
    VOLUME.iter().find(|(u, _)| label == *u || label.starts_with(&format!("{u} ")) || label.starts_with(&format!("{u},"))).map(|(u, ml)| (*ml, label == *u))
}

/// "medium (2-1/2" dia)" → "medium", "cloves" → "clove", "slice, thin" → "slice".
fn portion_word(label: &str) -> String {
    let w = label.split([',', '(']).next().unwrap_or("").trim();
    match w {
        "leaves" => "leaf".into(),
        "cloves" | "slices" | "sticks" | "pieces" | "stalks" | "sprigs" | "heads" | "strips" | "wedges" | "cans" | "packages" => w[..w.len() - 1].into(),
        _ => w.into(),
    }
}

/// A USDA record as a table entry. `each`: the portion one item (or, for a volume portion, one cup)
/// means, when USDA's first choice isn't the right one ("large" eggs, "cup, ground" almonds).
pub fn to_food(f: &UsdaFood, each: Option<&str>) -> Food {
    let mut n = BTreeMap::new();
    let mut atwater = None;
    for x in &f.food_nutrients {
        let (Some(num), Some(a)) = (x.nutrient.as_ref().and_then(|n| n.number.as_deref()), x.amount) else { continue };
        if let Some((_, k)) = NUTRIENTS.iter().find(|(id, _)| *id == num) {
            n.insert(k.to_string(), a);
        } else if num == "958" || (num == "957" && atwater.is_none()) {
            atwater = Some(a); // Foundation foods: energy by Atwater factors instead of "208"
        }
    }
    if let (false, Some(a)) = (n.contains_key("calories"), atwater) {
        n.insert("calories".into(), a);
    }
    let portions: Vec<(String, f64)> = f.food_portions.iter().filter(|p| p.gram_weight > 0.0).map(|p| (portion_label(p), p.gram_weight / p.amount.filter(|a| *a > 0.0).unwrap_or(1.0))).collect();
    let chosen = each.and_then(|e| portions.iter().find(|(l, _)| l == &e.to_lowercase()));
    // grams per ml: the chosen cup, else a plain cup, tbsp, tsp, then a described one ("cup, chopped")
    let mut vol = chosen.filter(|(l, _)| volume_of(l).is_some());
    for plain in [true, false] {
        for (u, _) in VOLUME {
            vol = vol.or_else(|| portions.iter().find(|(l, _)| l.starts_with(u) && volume_of(l).is_some_and(|(_, p)| p == plain)));
        }
    }
    let ml = vol.and_then(|(l, g)| volume_of(l).map(|(ml, _)| (g / ml * 10000.0).round() / 10000.0));
    let mut g = BTreeMap::new();
    for (l, w) in &portions {
        let word = portion_word(l);
        if volume_of(l).is_none() && !word.is_empty() && !matches!(word.as_str(), "oz" | "lb" | "serving" | "undetermined") {
            g.entry(word).or_insert((w * 100.0).round() / 100.0);
        }
    }
    if let Some((_, w)) = chosen.filter(|(l, _)| volume_of(l).is_none()) {
        g.insert("each".into(), (w * 100.0).round() / 100.0);
    }
    Food { fdc: f.fdc_id, name: f.description.clone(), n, ml, g, auto: false }
}

#[derive(Deserialize, Serialize, Default)]
struct Own {
    #[serde(default)]
    foods: BTreeMap<String, Option<Food>>,
    /// keys a lookup found nothing for (not searched again until the key is cleared)
    #[serde(default)]
    missed: BTreeSet<String>,
}

fn bundled() -> &'static Value {
    static B: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(BUNDLED).expect("data/foods.json is valid JSON"));
    &B
}

/// The ingredient keys the shipped table knows: `ingredient::annotate` reads alternatives with them.
pub fn known() -> &'static HashSet<String> {
    static K: LazyLock<HashSet<String>> = LazyLock::new(|| bundled()["foods"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default());
    &K
}

fn load(s: &AppState) -> Own {
    std::fs::read(s.data_dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(s: &AppState, o: &Own) -> Result<()> {
    write_private(&s.data_dir.join(FILE), &serde_json::to_vec_pretty(o)?)
}

/// The shipped table with the data folder's entries on top.
pub fn table(s: &AppState) -> Value {
    let mut t = bundled().clone();
    let own = load(s);
    // which keys come from this Mac (a lookup or a choice), so the Ingredients page can offer to undo them
    t["own"] = json!(own.foods.keys().collect::<Vec<_>>());
    if let Some(foods) = t["foods"].as_object_mut() {
        for (k, v) in own.foods {
            foods.insert(k, serde_json::to_value(v).unwrap());
        }
    }
    t
}

/// `GET /api/foods`: everyone who can read recipes can read this.
async fn route(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let body = serde_json::to_vec(&table(&s)).unwrap();
    let tag = format!("\"f{}\"", crate::vault::etag_of(&body));
    if headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(tag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }
    ([(header::CONTENT_TYPE, "application/json".to_string()), (header::ETAG, tag), (header::CACHE_CONTROL, "no-cache".to_string())], body).into_response()
}

// ---------- USDA lookups ----------

fn entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)?)
}

fn api_key() -> Option<String> {
    entry().and_then(|e| Ok(e.get_password()?)).ok().filter(|k| !k.is_empty())
}

pub fn key_set() -> bool {
    api_key().is_some()
}

static HTTP: LazyLock<reqwest::Client> =
    LazyLock::new(|| reqwest::Client::builder().timeout(Duration::from_secs(30)).connect_timeout(Duration::from_secs(10)).build().expect("http client"));

#[derive(Serialize, Debug)]
pub struct Hit {
    pub fdc: u64,
    pub name: String,
    pub kind: String,
}

struct RateLimited;
impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("USDA's hourly limit for this API key is reached; lookups continue later")
    }
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

async fn send(r: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let r = r.send().await?;
    match r.status().as_u16() {
        429 => Err(anyhow!(RateLimited.to_string())),
        401 | 403 => bail!("USDA refused the API key"),
        _ => Ok(r.error_for_status()?),
    }
}

/// Foundation and SR Legacy foods matching a name, USDA's best first.
async fn search(key: &str, q: &str) -> Result<Vec<Hit>> {
    let body = json!({ "query": q, "dataType": ["Foundation", "SR Legacy"], "pageSize": 15 });
    let v: Value = send(HTTP.post(format!("{API}/foods/search?api_key={}", urlencode(key))).json(&body)).await?.json().await?;
    Ok(v["foods"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some(Hit { fdc: f["fdcId"].as_u64()?, name: f["description"].as_str()?.to_string(), kind: f["dataType"].as_str().unwrap_or("").to_string() }))
        .collect())
}

async fn details(key: &str, fdc: u64) -> Result<Food> {
    let f: UsdaFood = send(HTTP.get(format!("{API}/food/{fdc}?api_key={}", urlencode(key)))).await?.json().await?;
    Ok(to_food(&f, None))
}

/// The lookup's choice among results naming every word of the ingredient: SR Legacy first (it has
/// household weights), then the plainest (fewest other words: "Nuts, almonds" over "Beverages, almond
/// milk", "Onions, raw" over "Onions, cooked, boiled"). None rather than a stretch: an uncounted line
/// beats a wrong food.
fn pick<'a>(name: &str, hits: &'a [Hit]) -> Option<&'a Hit> {
    let words: Vec<&str> = name.split_whitespace().filter(|w| w.len() >= 3).collect();
    if words.is_empty() {
        return None;
    }
    hits.iter()
        .filter(|h| words.iter().all(|w| h.name.to_lowercase().contains(w)))
        .min_by_key(|h| (h.kind != "SR Legacy", h.name.split(|c: char| !c.is_alphanumeric()).filter(|x| !x.is_empty()).count()))
}

/// Ingredient keys in every cookbook's recipes that no layer of the table has, most used first.
fn missing(s: &AppState) -> Vec<String> {
    let idxs: Vec<_> = s.books.read().unwrap().iter().map(|b| b.index.read().unwrap().clone()).collect();
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    for r in idxs.iter().flat_map(|i| i.files.values()).filter_map(|e| e.recipe.as_ref()) {
        for a in r.ingredients.iter().flat_map(|g| &g.items).flat_map(|it| &it.amounts).flatten() {
            if let (Some(k), None) = (&a.key, &a.skip) {
                *uses.entry(k.clone()).or_default() += 1;
            }
        }
    }
    let own = load(s);
    let have = bundled()["foods"].as_object();
    let mut out: Vec<(String, usize)> =
        uses.into_iter().filter(|(k, _)| !own.foods.contains_key(k) && !own.missed.contains(k) && !have.is_some_and(|h| h.contains_key(k))).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out.into_iter().map(|x| x.0).collect()
}

/// One background run: look up what's missing (a few at a time, politely spaced).
async fn run(s: &Arc<AppState>) -> Result<usize> {
    let Some(key) = tokio::task::spawn_blocking(api_key).await? else { return Ok(0) };
    let todo = missing(s);
    let mut found = 0;
    for name in todo.into_iter().take(PER_RUN) {
        let hit = match search(&key, &name).await {
            Ok(hits) => pick(&name, &hits).map(|h| h.fdc),
            Err(e) => return Err(e),
        };
        let food = match hit {
            Some(fdc) => Some(details(&key, fdc).await?),
            None => None,
        };
        let _g = s.ext.nutrition.file.lock().unwrap();
        let mut own = load(s);
        match food {
            Some(mut f) => {
                f.auto = true;
                own.foods.insert(name, Some(f));
                found += 1;
            }
            None => {
                own.missed.insert(name);
            }
        }
        save(s, &own)?;
        drop(_g);
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    Ok(found)
}

/// Background state: wakes after the recipes change or a key is saved.
#[derive(Default)]
pub struct Lookups {
    notify: tokio::sync::Notify,
    file: Mutex<()>,
    last: Mutex<Option<String>>,
}

pub fn changed(s: &AppState) {
    s.ext.nutrition.notify.notify_one();
}

pub fn spawn(s: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            let result = run(&s).await;
            let rate_limited = matches!(&result, Err(e) if e.to_string() == RateLimited.to_string());
            *s.ext.nutrition.last.lock().unwrap() = match result {
                Ok(n) if n > 0 => Some(format!("Found {n} ingredients on USDA FoodData Central")),
                Ok(_) => None,
                Err(e) => {
                    tracing::warn!("nutrition lookups: {e:#}");
                    Some(format!("{e:#}"))
                }
            };
            tokio::select! {
                _ = s.ext.nutrition.notify.notified() => {}
                _ = tokio::time::sleep(if rate_limited { Duration::from_secs(3600) } else { Duration::from_secs(6 * 3600) }) => {}
            }
            tokio::time::sleep(Duration::from_secs(5)).await; // a burst of saves is one run
        }
    });
}

/// What the last run reported (an error, or how many it found), for Settings.
pub fn status(s: &AppState) -> Option<String> {
    s.ext.nutrition.last.lock().unwrap().clone()
}

// ---------- routes ----------

type Res = Result<Response, Response>;

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/foods", get(route))
        .route("/api/foods/search", get(search_route))
        .route("/api/foods/{key}", put(choose).delete(reset))
        .route("/api/settings/usda-key", put(set_key))
}

#[derive(Deserialize)]
struct KeyBody {
    key: String,
}

async fn set_key(State(s): State<Arc<AppState>>, caller: Caller, Json(b): Json<KeyBody>) -> Res {
    caller.need(Role::Admin)?;
    let k = b.key.trim().to_string();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let e = entry()?;
        if k.is_empty() {
            let _ = e.delete_credential();
        } else {
            e.set_password(&k)?;
        }
        Ok(())
    })
    .await
    .unwrap()
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("Keychain: {e}")))?;
    changed(&s);
    Ok(StatusCode::NO_CONTENT.into_response())
}

fn need_key() -> Result<String, Response> {
    api_key().ok_or_else(|| err(StatusCode::BAD_REQUEST, "Add a USDA API key in Settings → Nutrition first"))
}

#[derive(Deserialize)]
struct Q {
    q: String,
}

/// `GET /api/foods/search?q=`: USDA foods to choose from on the Ingredients page.
async fn search_route(caller: Caller, Query(q): Query<Q>) -> Res {
    caller.need(Role::Admin)?;
    let key = tokio::task::spawn_blocking(need_key).await.unwrap()?;
    let hits = search(&key, q.q.trim()).await.map_err(|e| err(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
    Ok(Json(hits).into_response())
}

#[derive(Deserialize)]
struct Choice {
    /// a USDA food, or null: don't count this ingredient
    fdc: Option<u64>,
}

/// `PUT /api/foods/{key}`: use this USDA food for an ingredient, or `{fdc: null}` to leave it out.
async fn choose(State(s): State<Arc<AppState>>, caller: Caller, Path(key): Path<String>, Json(c): Json<Choice>) -> Res {
    caller.need(Role::Admin)?;
    let food = match c.fdc {
        Some(fdc) => {
            let k = tokio::task::spawn_blocking(need_key).await.unwrap()?;
            Some(details(&k, fdc).await.map_err(|e| err(StatusCode::BAD_GATEWAY, format!("{e:#}")))?)
        }
        None => None,
    };
    let _g = s.ext.nutrition.file.lock().unwrap();
    let mut own = load(&s);
    own.foods.insert(key, food);
    save(&s, &own).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `DELETE /api/foods/{key}`: undo a choice or lookup (back to the shipped table; looked up again).
async fn reset(State(s): State<Arc<AppState>>, caller: Caller, Path(key): Path<String>) -> Res {
    caller.need(Role::Admin)?;
    {
        let _g = s.ext.nutrition.file.lock().unwrap();
        let mut own = load(&s);
        own.foods.remove(&key);
        own.missed.remove(&key);
        save(&s, &own).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    changed(&s);
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn food(json: &str) -> UsdaFood {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn bundled_table_is_valid() {
        let foods = bundled()["foods"].as_object().unwrap();
        assert!(foods.len() > 100);
        for (k, f) in foods {
            let f: Food = serde_json::from_value(f.clone()).unwrap_or_else(|e| panic!("{k}: {e}"));
            assert!(f.n.contains_key("calories"), "{k}: calories");
        }
        assert!(bundled()["source"].as_str().unwrap().contains("FoodData Central"));
        assert!(known().contains("egg"));
    }

    /// A trimmed SR Legacy record, as in the download and the API.
    const EGG: &str = r#"{"fdcId": 171287, "description": "Egg, whole, raw, fresh", "dataType": "SR Legacy",
        "foodNutrients": [{"nutrient": {"number": "208", "name": "Energy"}, "amount": 143},
                          {"nutrient": {"number": "204"}, "amount": 9.51}, {"nutrient": {"number": "999"}, "amount": 1}],
        "foodPortions": [{"amount": 1, "modifier": "cup (4.86 large eggs)", "gramWeight": 243, "measureUnit": {"name": "undetermined"}},
                         {"amount": 1, "modifier": "medium", "gramWeight": 44}, {"amount": 1, "modifier": "large", "gramWeight": 50},
                         {"amount": 3, "modifier": "cloves", "gramWeight": 9}]}"#;

    #[test]
    fn usda_records() {
        let f = to_food(&food(EGG), Some("large"));
        assert_eq!(f.n.get("calories"), Some(&143.0));
        assert_eq!(f.n.get("fat"), Some(&9.51));
        assert_eq!(f.n.len(), 2);
        assert_eq!(f.ml, Some((243.0 / 236.588 * 10000.0_f64).round() / 10000.0));
        assert_eq!((f.g["medium"], f.g["large"], f.g["each"], f.g["clove"]), (44.0, 50.0, 50.0, 3.0));
        // Foundation foods: the unit apart from the modifier, energy by Atwater factors
        let f = to_food(&food(r#"{"fdcId": 1, "description": "Almonds", "foodNutrients": [{"nutrient": {"number": "957"}, "amount": 600}],
            "foodPortions": [{"amount": 1, "modifier": "whole", "gramWeight": 143, "measureUnit": {"name": "cup"}},
                             {"amount": 1, "modifier": "ground", "gramWeight": 95, "measureUnit": {"name": "cup"}}]}"#), Some("cup, ground"));
        assert_eq!(f.n["calories"], 600.0);
        assert_eq!(f.ml, Some((95.0 / 236.588 * 10000.0_f64).round() / 10000.0)); // the chosen cup
    }

    #[test]
    fn picks_only_what_it_names() {
        let hits = vec![
            Hit { fdc: 1, name: "Beverages, almond milk, sweetened".into(), kind: "SR Legacy".into() },
            Hit { fdc: 2, name: "Flour, almond".into(), kind: "Foundation".into() },
            Hit { fdc: 3, name: "Nuts, almonds, blanched".into(), kind: "SR Legacy".into() },
        ];
        assert_eq!(pick("almond flour", &hits).map(|h| h.fdc), Some(2));
        assert_eq!(pick("almond", &hits).map(|h| h.fdc), Some(3)); // SR Legacy, and the plainest: not almond milk
        assert!(pick("tigernut flour", &hits).is_none());
    }
}
