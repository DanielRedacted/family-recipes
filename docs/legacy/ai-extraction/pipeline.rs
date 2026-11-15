use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::db::models::{
    SchemaOrgAuthor, SchemaOrgIngredient, SchemaOrgNutrition, SchemaOrgRating, SchemaOrgRecipe,
};
use crate::ollama::prompt::{
    build_metadata_prompt, build_ingredients_prompt, build_instructions_prompt, build_gap_fill_prompt,
    build_enrichment_prompt, build_ingredients_review_prompt,
    build_metadata_system_prompt, build_gap_fill_constraints,
    INGREDIENTS_SYSTEM_PROMPT, INSTRUCTIONS_SYSTEM_PROMPT, GAP_FILL_SYSTEM_PROMPT,
    enrichment_system_prompt, enrichment_with_desc_system_prompt,
    CORRECTION_METADATA_PROMPT, CORRECTION_INGREDIENTS_PROMPT, CORRECTION_INSTRUCTIONS_PROMPT,
    CORRECTION_GAP_FILL_PROMPT, INGREDIENTS_REVIEW_SYSTEM_PROMPT,
};
use crate::{AppError, Result};

// ---------------------------------------------------------------------------
// Provider enum + settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    Ollama,
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub provider: AiProvider,
    pub base_url: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub max_input_chars: usize,
    /// Sampling temperature (0.0–2.0). Lower = more deterministic. Default 0.1.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Maximum tokens the model may generate. Default 4096.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    // ── Ollama-specific sampling options ──────────────────────────────────────
    /// top-k sampling. 1 = greedy (most deterministic). Ollama only.
    #[serde(default = "default_top_k")]
    pub top_k: i32,
    /// Nucleus sampling threshold. With top_k=1 this has no effect. Ollama only.
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    /// Fixed RNG seed → reproducible output for identical inputs. Ollama only.
    #[serde(default = "default_seed")]
    pub seed: i32,
    /// Context window size in tokens. Ollama only.
    #[serde(default = "default_num_ctx")]
    pub num_ctx: i32,
}

fn default_temperature() -> f32 { 0.0 }
fn default_max_tokens() -> u32 { 4096 }
fn default_top_k() -> i32 { 1 }
fn default_top_p() -> f32 { 1.0 }
fn default_seed() -> i32 { 42 }
fn default_num_ctx() -> i32 { 8192 }

impl AiSettings {
    pub fn from_ollama_config(cfg: &crate::config::OllamaConfig) -> Self {
        Self {
            provider: AiProvider::Ollama,
            base_url: cfg.base_url.clone(),
            model: cfg.model.clone(),
            api_key: None,
            timeout_secs: cfg.timeout_secs,
            max_retries: cfg.max_retries,
            max_input_chars: cfg.max_input_chars,
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            top_k: default_top_k(),
            top_p: default_top_p(),
            seed: default_seed(),
            num_ctx: default_num_ctx(),
        }
    }
}

/// Load AI settings from the DB; fall back to config.toml defaults.
pub async fn load(pool: &sqlx::SqlitePool, default: &crate::config::OllamaConfig) -> AiSettings {
    let row = sqlx::query("SELECT value FROM settings WHERE key = 'ai_settings'")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    if let Some(row) = row {
        use sqlx::Row;
        let json: String = row.get("value");
        if let Ok(s) = serde_json::from_str::<AiSettings>(&json) {
            return s;
        }
    }

    AiSettings::from_ollama_config(default)
}

/// Persist AI settings to the DB.
pub async fn save(pool: &sqlx::SqlitePool, settings: &AiSettings) -> Result<()> {
    let json = serde_json::to_string(settings)?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('ai_settings', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&json)
    .execute(pool)
    .await?;
    Ok(())
}


/// Generic dispatcher: calls the configured provider with a custom system prompt,
/// then deserializes the raw JSON response into T.
async fn call_section<T: DeserializeOwned>(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    prompt: &str,
    label: &str,
) -> Result<T> {
    eprintln!(
        "\n╔══ AI query [{label}] ══════════════════════════════════════\n\
         ║ SYSTEM:\n{system}\n\
         ║\n\
         ║ USER:\n{prompt}\n\
         ╚════════════════════════════════════════════════════════════"
    );

    let text = match settings.provider {
        AiProvider::Ollama    => call_ollama(settings, client, system, prompt).await?,
        AiProvider::OpenAi    => call_openai(settings, client, system, prompt).await?,
        AiProvider::Anthropic => call_anthropic(settings, client, system, prompt).await?,
    };

    eprintln!(
        "\n╔══ AI response [{label}] ═════════════════════════════════════\n\
         {text}\n\
         ╚════════════════════════════════════════════════════════════"
    );

    serde_json::from_str::<T>(&text).map_err(|e| {
        AppError::Ollama(format!(
            "invalid JSON from AI: {e}\nResponse was: {}",
            &text[..text.len().min(300)]
        ))
    })
}

// ---------------------------------------------------------------------------
// Section structs (pub(crate) so ingest.rs can call sections with progress)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataSection {
    name: Option<String>,
    description: Option<String>,
    author: Option<SchemaOrgAuthor>,
    image: Option<serde_json::Value>,
    url: Option<String>,
    date_published: Option<String>,
    cook_time: Option<String>,
    prep_time: Option<String>,
    total_time: Option<String>,
    perform_time: Option<String>,
    recipe_yield: Option<serde_json::Value>,
    cooking_method: Option<String>,
    recipe_category: Option<serde_json::Value>,
    recipe_cuisine: Option<serde_json::Value>,
    keywords: Option<serde_json::Value>,
    suitable_for_diet: Option<serde_json::Value>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IngredientsSection {
    pub(crate) recipe_ingredient: Option<Vec<SchemaOrgIngredient>>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstructionsSection {
    recipe_instructions: Option<serde_json::Value>,
    tool: Option<Vec<serde_json::Value>>,
    supply: Option<Vec<serde_json::Value>>,
    nutrition: Option<SchemaOrgNutrition>,
    aggregate_rating: Option<SchemaOrgRating>,
}

// ---------------------------------------------------------------------------
// Per-section retry wrappers (pub(crate) so ingest.rs can interleave progress)
// ---------------------------------------------------------------------------

pub(crate) async fn call_metadata(
    settings: &AiSettings,
    client: &reqwest::Client,
    raw_text: &str,
    known_cuisines: &[String],
    known_categories: &[String],
) -> Result<MetadataSection> {
    let system = build_metadata_system_prompt(known_cuisines, known_categories);
    let mut last_err = String::new();
    for attempt in 0..=settings.max_retries {
        let prompt = if attempt == 0 {
            build_metadata_prompt(raw_text, settings.max_input_chars)
        } else {
            format!(
                "{}\n\nOriginal text:\n---\n{}\n---",
                CORRECTION_METADATA_PROMPT,
                &raw_text[..raw_text.len().min(settings.max_input_chars)]
            )
        };
        match call_section::<MetadataSection>(settings, client, &system, &prompt, "metadata").await {
            Ok(r) => return Ok(r),
            Err(e) => {
                tracing::warn!("metadata AI attempt {} failed: {}", attempt + 1, e);
                last_err = e.to_string();
            }
        }
    }
    Err(AppError::Ollama(format!(
        "metadata failed after {} attempts: {}",
        settings.max_retries + 1,
        last_err
    )))
}

pub(crate) async fn call_ingredients(
    settings: &AiSettings,
    client: &reqwest::Client,
    raw_text: &str,
) -> Result<IngredientsSection> {
    let mut last_err = String::new();
    for attempt in 0..=settings.max_retries {
        let prompt = if attempt == 0 {
            build_ingredients_prompt(raw_text, settings.max_input_chars)
        } else {
            format!(
                "{}\n\nOriginal text:\n---\n{}\n---",
                CORRECTION_INGREDIENTS_PROMPT,
                &raw_text[..raw_text.len().min(settings.max_input_chars)]
            )
        };
        match call_section::<IngredientsSection>(settings, client, INGREDIENTS_SYSTEM_PROMPT, &prompt, "ingredients").await {
            Ok(r) => return Ok(r),
            Err(e) => {
                tracing::warn!("ingredients AI attempt {} failed: {}", attempt + 1, e);
                last_err = e.to_string();
            }
        }
    }
    Err(AppError::Ollama(format!(
        "ingredients failed after {} attempts: {}",
        settings.max_retries + 1,
        last_err
    )))
}

/// Post-parse review pass: sends the already-parsed ingredient list back to the AI
/// with the schema rules and asks it to correct any violations.
/// Non-fatal — returns the original `IngredientsSection` unchanged on failure.
pub(crate) async fn call_ingredients_review(
    settings: &AiSettings,
    client: &reqwest::Client,
    ing: IngredientsSection,
) -> IngredientsSection {
    let parsed_json = match serde_json::to_string_pretty(&serde_json::json!({
        "recipeIngredient": ing.recipe_ingredient
    })) {
        Ok(j) => j,
        Err(_) => return ing,
    };

    let prompt = build_ingredients_review_prompt(&parsed_json);
    match call_section::<IngredientsSection>(
        settings,
        client,
        INGREDIENTS_REVIEW_SYSTEM_PROMPT,
        &prompt,
        "ingredients_review",
    )
    .await
    {
        Ok(corrected) => corrected,
        Err(e) => {
            tracing::warn!("ingredients review failed (non-fatal): {}", e);
            ing
        }
    }
}

pub(crate) async fn call_instructions_section(
    settings: &AiSettings,
    client: &reqwest::Client,
    raw_text: &str,
) -> Result<InstructionsSection> {
    let mut last_err = String::new();
    for attempt in 0..=settings.max_retries {
        let prompt = if attempt == 0 {
            build_instructions_prompt(raw_text, settings.max_input_chars)
        } else {
            format!(
                "{}\n\nOriginal text:\n---\n{}\n---",
                CORRECTION_INSTRUCTIONS_PROMPT,
                &raw_text[..raw_text.len().min(settings.max_input_chars)]
            )
        };
        match call_section::<InstructionsSection>(settings, client, INSTRUCTIONS_SYSTEM_PROMPT, &prompt, "instructions").await {
            Ok(r) => return Ok(r),
            Err(e) => {
                tracing::warn!("instructions AI attempt {} failed: {}", attempt + 1, e);
                last_err = e.to_string();
            }
        }
    }
    Err(AppError::Ollama(format!(
        "instructions failed after {} attempts: {}",
        settings.max_retries + 1,
        last_err
    )))
}


pub(crate) fn merge_sections(
    meta: MetadataSection,
    ing: IngredientsSection,
    ins: InstructionsSection,
) -> SchemaOrgRecipe {
    SchemaOrgRecipe {
        name:              meta.name,
        description:       meta.description,
        author:            meta.author,
        image:             meta.image,
        url:               meta.url,
        date_published:    meta.date_published,
        cook_time:         meta.cook_time,
        prep_time:         meta.prep_time,
        total_time:        meta.total_time,
        perform_time:      meta.perform_time,
        recipe_yield:      meta.recipe_yield,
        cooking_method:    meta.cooking_method,
        recipe_category:   meta.recipe_category,
        recipe_cuisine:    meta.recipe_cuisine,
        keywords:          meta.keywords,
        suitable_for_diet: meta.suitable_for_diet,
        recipe_ingredient:    ing.recipe_ingredient,
        recipe_instructions: ins.recipe_instructions,
        tool:                ins.tool,
        supply:              ins.supply,
        nutrition:           ins.nutrition,
        aggregate_rating:    ins.aggregate_rating,
        video:               None,
        estimated_cost:      None,
    }
}

// ---------------------------------------------------------------------------
// Gap-fill pass: one targeted query for whatever the first three left null
// ---------------------------------------------------------------------------

/// Returns (json_field_name, human description) for every null field worth filling.
fn collect_null_fields(r: &SchemaOrgRecipe) -> Vec<(&'static str, &'static str)> {
    let mut f: Vec<(&str, &str)> = vec![];
    if r.description.is_none()     { f.push(("description",     "the introductory description or headnote paragraph as it appears in the source text — copy verbatim, do not write your own summary; if no description exists in the source, use null")); }
    if r.prep_time.is_none()       { f.push(("prepTime",        "ISO 8601 duration e.g. PT15M")); }
    if r.cook_time.is_none()       { f.push(("cookTime",        "ISO 8601 duration e.g. PT30M")); }
    if r.total_time.is_none()      { f.push(("totalTime",       "ISO 8601 total duration e.g. PT45M")); }
    if r.recipe_yield.is_none()    { f.push(("recipeYield",     "e.g. '4 servings' or '12 cookies'")); }
    if r.cooking_method.is_none()  { f.push(("cookingMethod",   "e.g. 'Baking', 'Grilling'")); }
    if r.recipe_category.is_none() { f.push(("recipeCategory",  "e.g. 'Dessert', 'Main course'")); }
    if r.recipe_cuisine.is_none()  { f.push(("recipeCuisine",   "e.g. 'Italian', 'Mexican'")); }
    if r.keywords.is_none()        { f.push(("keywords",        "comma-separated tags")); }
    if r.suitable_for_diet.is_none() { f.push(("suitableForDiet", "comma-separated applicable diets")); }
    let ing_empty = r.recipe_ingredient.as_ref().map(|v| v.is_empty()).unwrap_or(true);
    if ing_empty { f.push(("recipeIngredient", "array of { quantity, unit, name, preparation } objects")); }
    if r.recipe_instructions.is_none() { f.push(("recipeInstructions", "flat array of plain step strings")); }
    if r.nutrition.is_none()       { f.push(("nutrition",       "NutritionInformation with calories, fat, carbs, protein")); }
    if r.aggregate_rating.is_none() { f.push(("aggregateRating", "AggregateRating with ratingValue, ratingCount, bestRating")); }
    f
}

/// Merge gap-fill result into the existing recipe: keep non-null fields, fill in nulls.
fn apply_gap_fill(r: SchemaOrgRecipe, fill: SchemaOrgRecipe) -> SchemaOrgRecipe {
    let ing = if r.recipe_ingredient.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        fill.recipe_ingredient
    } else {
        r.recipe_ingredient
    };
    SchemaOrgRecipe {
        name:               r.name.or(fill.name),
        description:        r.description.or(fill.description),
        author:             r.author.or(fill.author),
        image:              r.image.or(fill.image),
        url:                r.url.or(fill.url),
        date_published:     r.date_published.or(fill.date_published),
        cook_time:          r.cook_time.or(fill.cook_time),
        prep_time:          r.prep_time.or(fill.prep_time),
        total_time:         r.total_time.or(fill.total_time),
        perform_time:       r.perform_time.or(fill.perform_time),
        recipe_yield:       r.recipe_yield.or(fill.recipe_yield),
        cooking_method:     r.cooking_method.or(fill.cooking_method),
        recipe_category:    r.recipe_category.or(fill.recipe_category),
        recipe_cuisine:     r.recipe_cuisine.or(fill.recipe_cuisine),
        keywords:           r.keywords.or(fill.keywords),
        suitable_for_diet:  r.suitable_for_diet.or(fill.suitable_for_diet),
        recipe_ingredient:  ing,
        recipe_instructions: r.recipe_instructions.or(fill.recipe_instructions),
        tool:               r.tool.or(fill.tool),
        supply:             r.supply.or(fill.supply),
        nutrition:          r.nutrition.or(fill.nutrition),
        aggregate_rating:   r.aggregate_rating.or(fill.aggregate_rating),
        video:              r.video.or(fill.video),
        estimated_cost:     r.estimated_cost.or(fill.estimated_cost),
    }
}

/// Fourth-pass gap fill. Inspects the merged recipe for null fields, skips if nothing
/// is missing, otherwise makes one more AI call. Failure is non-fatal — the original
/// recipe is returned unchanged.
pub(crate) async fn call_gap_fill(
    settings: &AiSettings,
    client: &reqwest::Client,
    raw_text: &str,
    recipe: SchemaOrgRecipe,
    known_cuisines: &[String],
    known_categories: &[String],
) -> Result<SchemaOrgRecipe> {
    let missing = collect_null_fields(&recipe);
    if missing.is_empty() {
        tracing::debug!("gap_fill: all fields present, skipping");
        return Ok(recipe);
    }
    tracing::debug!("gap_fill: {} null fields to fill: {:?}", missing.len(), missing.iter().map(|(k,_)| k).collect::<Vec<_>>());

    let constraints = build_gap_fill_constraints(known_cuisines, known_categories);
    let gap_fill_system = format!("{}{}", GAP_FILL_SYSTEM_PROMPT, constraints);

    let mut last_err = String::new();
    for attempt in 0..=settings.max_retries {
        let prompt = if attempt == 0 {
            build_gap_fill_prompt(raw_text, &missing, settings.max_input_chars)
        } else {
            format!(
                "{}\n\nOriginal text:\n---\n{}\n---",
                CORRECTION_GAP_FILL_PROMPT,
                &raw_text[..raw_text.len().min(settings.max_input_chars)]
            )
        };
        match call_section::<SchemaOrgRecipe>(settings, client, &gap_fill_system, &prompt, "gap_fill").await {
            Ok(fill) => return Ok(apply_gap_fill(recipe, fill)),
            Err(e) => {
                tracing::warn!("gap_fill attempt {} failed: {}", attempt + 1, e);
                last_err = e.to_string();
            }
        }
    }
    tracing::warn!("gap_fill failed after {} attempts, using partial recipe: {}", settings.max_retries + 1, last_err);
    Ok(recipe)
}

// ---------------------------------------------------------------------------
// Enrichment pass: emoji, color, and optional description
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct EnrichmentResult {
    pub(crate) emojis: Vec<String>,
    pub(crate) color: String,
    pub(crate) description: Option<String>,
}

impl Default for EnrichmentResult {
    fn default() -> Self {
        Self {
            emojis: vec!["🍽".to_string()],
            color: "white".to_string(),
            description: None,
        }
    }
}

/// Non-fatal enrichment call. Assigns an emoji and color theme; generates a description
/// only when the recipe has none. Returns defaults on failure.
pub(crate) async fn call_enrichment(
    settings: &AiSettings,
    client: &reqwest::Client,
    name: &str,
    has_description: bool,
    category: &str,
    cuisine: &str,
) -> EnrichmentResult {
    let system = if has_description {
        enrichment_system_prompt()
    } else {
        enrichment_with_desc_system_prompt()
    };
    let prompt = build_enrichment_prompt(name, category, cuisine);
    match call_section::<EnrichmentResult>(
        settings,
        client,
        &system,
        &prompt,
        "enrichment",
    )
    .await
    {
        Ok(mut r) => {
            // Sanitise: clamp color to the allowed set; fall back to white.
            let valid_colors = ["red","orange","yellow","green","blue","purple","brown","white"];
            if !valid_colors.contains(&r.color.trim().to_lowercase().as_str()) {
                r.color = "white".to_string();
            } else {
                r.color = r.color.trim().to_lowercase();
            }
            // Sanitise: extract one grapheme per entry, drop empties, cap at 3.
            r.emojis = r.emojis.iter()
                .filter_map(|e| first_grapheme(e))
                .take(3)
                .collect();
            if r.emojis.is_empty() {
                r.emojis = vec!["🍽".to_string()];
            }
            r
        }
        Err(e) => {
            tracing::warn!("enrichment failed (non-fatal): {}", e);
            EnrichmentResult::default()
        }
    }
}

/// Return the first Unicode grapheme cluster from a string (i.e. one visible emoji/char).
fn first_grapheme(s: &str) -> Option<String> {
    // Walk char by char; emoji are typically 1–2 chars. We just take the first char(s)
    // that form a valid non-whitespace sequence.
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Collect chars until we have at least one non-whitespace char and hit whitespace or end.
    let mut out = String::new();
    for ch in trimmed.chars() {
        if out.is_empty() && ch.is_whitespace() {
            continue;
        }
        out.push(ch);
        // Stop after the first visible character cluster (emoji may be multi-char via ZWJ,
        // but a simple char boundary is good enough for our single-emoji use case).
        if out.chars().count() >= 2 {
            break;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

// ---------------------------------------------------------------------------
// Ollama caller
// ---------------------------------------------------------------------------

async fn call_ollama(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    prompt: &str,
) -> Result<String> {

    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        prompt: &'a str,
        system: &'a str,
        format: &'a str,
        stream: bool,
        options: Opts,
    }
    #[derive(serde::Serialize)]
    struct Opts {
        temperature: f32,
        num_predict: i32,
        /// Greedy decoding: only the single most-probable token is ever chosen.
        top_k: i32,
        /// With top_k=1, top_p has no effect — included for explicitness.
        top_p: f32,
        /// Fixed seed → identical output for identical input (reproducible).
        seed: i32,
        /// Context window in tokens — large enough for full recipe pages.
        num_ctx: i32,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        response: String,
    }

    let req = Req {
        model: &settings.model,
        prompt,
        system,
        format: "json",
        stream: false,
        options: Opts {
            temperature: settings.temperature,
            num_predict: settings.max_tokens as i32,
            top_k: settings.top_k,
            top_p: settings.top_p,
            seed: settings.seed,
            num_ctx: settings.num_ctx,
        },
    };

    let resp = timed_post(
        client,
        &format!("{}/api/generate", settings.base_url),
        None,
        &req,
        settings.timeout_secs,
    )
    .await?;

    let body: Resp = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("failed to parse Ollama response: {e}")))?;
    Ok(body.response)
}

// ---------------------------------------------------------------------------
// OpenAI-compatible caller (also covers LM Studio, Groq, etc.)
// ---------------------------------------------------------------------------

async fn call_openai(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    prompt: &str,
) -> Result<String> {

    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        messages: Vec<Msg<'a>>,
        response_format: ResponseFormat,
        temperature: f32,
        max_tokens: u32,
    }
    #[derive(serde::Serialize)]
    struct Msg<'a> {
        role: &'a str,
        content: &'a str,
    }
    #[derive(serde::Serialize)]
    struct ResponseFormat {
        #[serde(rename = "type")]
        kind: &'static str,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        choices: Vec<Choice>,
    }
    #[derive(serde::Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(serde::Deserialize)]
    struct Message {
        content: String,
    }

    let req = Req {
        model: &settings.model,
        messages: vec![
            Msg { role: "system", content: system },
            Msg { role: "user",   content: prompt },
        ],
        response_format: ResponseFormat { kind: "json_object" },
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
    };

    let api_key = settings.api_key.as_deref().unwrap_or("");
    let resp = timed_post(
        client,
        &format!("{}/v1/chat/completions", settings.base_url.trim_end_matches('/')),
        Some(api_key),
        &req,
        settings.timeout_secs,
    )
    .await?;

    let body: Resp = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("failed to parse OpenAI response: {e}")))?;

    body.choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| AppError::Ollama("OpenAI returned empty choices".into()))
}

// ---------------------------------------------------------------------------
// Anthropic Claude caller
// ---------------------------------------------------------------------------

async fn call_anthropic(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    prompt: &str,
) -> Result<String> {

    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        system: &'a str,
        messages: Vec<Msg<'a>>,
        max_tokens: u32,
        temperature: f32,
    }
    #[derive(serde::Serialize)]
    struct Msg<'a> {
        role: &'a str,
        content: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        content: Vec<Block>,
    }
    #[derive(serde::Deserialize)]
    struct Block {
        text: Option<String>,
    }

    let base = settings
        .base_url
        .trim_end_matches('/')
        .to_string();
    let url = if base.is_empty() || base == "https://api.anthropic.com" {
        "https://api.anthropic.com/v1/messages".to_string()
    } else {
        format!("{}/v1/messages", base)
    };

    let api_key = settings.api_key.as_deref().unwrap_or("");
    let req = Req {
        model: &settings.model,
        system,
        messages: vec![Msg { role: "user", content: prompt }],
        max_tokens: settings.max_tokens,
        temperature: settings.temperature,
    };

    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(settings.timeout_secs),
        client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&req)
            .send(),
    )
    .await
    .map_err(|_| AppError::Ollama("Anthropic request timed out".into()))?
    .map_err(|e| AppError::Ollama(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Ollama(format!(
            "Anthropic returned HTTP {status}: {body}"
        )));
    }

    let body: Resp = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("failed to parse Anthropic response: {e}")))?;

    body.content
        .into_iter()
        .find_map(|b| b.text)
        .ok_or_else(|| AppError::Ollama("Anthropic returned no text content".into()))
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

pub async fn health_check(client: &reqwest::Client, settings: &AiSettings) -> bool {
    match settings.provider {
        AiProvider::Ollama => {
            client
                .get(format!("{}/api/tags", settings.base_url))
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        AiProvider::OpenAi => {
            // Hit /v1/models — works for real OpenAI and most compat servers
            let url = format!(
                "{}/v1/models",
                settings.base_url.trim_end_matches('/')
            );
            let mut req = client.get(&url).timeout(std::time::Duration::from_secs(4));
            if let Some(key) = &settings.api_key {
                req = req.bearer_auth(key);
            }
            req.send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        AiProvider::Anthropic => {
            // Anthropic has no free health endpoint; just check TCP reachability
            // by attempting a HEAD on the base URL
            client
                .head("https://api.anthropic.com")
                .timeout(std::time::Duration::from_secs(4))
                .send()
                .await
                .is_ok()
        }
    }
}

/// Returns available model names for the configured provider.
pub async fn list_models(client: &reqwest::Client, settings: &AiSettings) -> Result<Vec<String>> {
    match settings.provider {
        AiProvider::Ollama => {
            #[derive(serde::Deserialize)]
            struct Tags { models: Vec<Model> }
            #[derive(serde::Deserialize)]
            struct Model { name: String }

            let resp: Tags = client
                .get(format!("{}/api/tags", settings.base_url))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
                .map_err(|e| AppError::Ollama(e.to_string()))?
                .json()
                .await
                .map_err(|e| AppError::Ollama(e.to_string()))?;
            Ok(resp.models.into_iter().map(|m| m.name).collect())
        }
        AiProvider::OpenAi => {
            #[derive(serde::Deserialize)]
            struct Models { data: Vec<Model> }
            #[derive(serde::Deserialize)]
            struct Model { id: String }

            let url = format!("{}/v1/models", settings.base_url.trim_end_matches('/'));
            let mut req = client.get(&url).timeout(std::time::Duration::from_secs(5));
            if let Some(key) = &settings.api_key {
                req = req.bearer_auth(key);
            }
            let resp: Models = req
                .send()
                .await
                .map_err(|e| AppError::Ollama(e.to_string()))?
                .json()
                .await
                .map_err(|e| AppError::Ollama(e.to_string()))?;
            Ok(resp.data.into_iter().map(|m| m.id).collect())
        }
        AiProvider::Anthropic => {
            // Anthropic doesn't expose a public models list endpoint —
            // return a curated list of current production models.
            Ok(vec![
                "claude-opus-4-5".into(),
                "claude-sonnet-4-5".into(),
                "claude-haiku-4-5-20251001".into(),
            ])
        }
    }
}

// ---------------------------------------------------------------------------
// Plain-text chat (for recipe Q&A — no JSON format constraint)
// ---------------------------------------------------------------------------

/// Single-turn chat that returns a plain-text response. Used for recipe Q&A where
/// the model should respond conversationally rather than emitting structured JSON.
pub async fn call_chat_plain(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    user_message: &str,
) -> Result<String> {
    match settings.provider {
        AiProvider::Ollama    => call_ollama_plain(settings, client, system, user_message).await,
        AiProvider::OpenAi    => call_openai_plain(settings, client, system, user_message).await,
        AiProvider::Anthropic => call_anthropic(settings, client, system, user_message).await,
    }
}

async fn call_ollama_plain(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    prompt: &str,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        prompt: &'a str,
        system: &'a str,
        stream: bool,
        options: Opts,
    }
    #[derive(serde::Serialize)]
    struct Opts {
        temperature: f32,
        num_predict: i32,
        num_ctx: i32,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        response: String,
    }

    let req = Req {
        model: &settings.model,
        prompt,
        system,
        stream: false,
        options: Opts {
            temperature: 0.7,  // more natural conversational replies
            num_predict: settings.max_tokens as i32,
            num_ctx: settings.num_ctx,
        },
    };

    let resp = timed_post(
        client,
        &format!("{}/api/generate", settings.base_url),
        None,
        &req,
        settings.timeout_secs,
    )
    .await?;

    let body: Resp = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("failed to parse Ollama response: {e}")))?;
    Ok(body.response)
}

async fn call_openai_plain(
    settings: &AiSettings,
    client: &reqwest::Client,
    system: &str,
    prompt: &str,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct Req<'a> {
        model: &'a str,
        messages: Vec<Msg<'a>>,
        temperature: f32,
        max_tokens: u32,
    }
    #[derive(serde::Serialize)]
    struct Msg<'a> {
        role: &'a str,
        content: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        choices: Vec<Choice>,
    }
    #[derive(serde::Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(serde::Deserialize)]
    struct Message {
        content: String,
    }

    let req = Req {
        model: &settings.model,
        messages: vec![
            Msg { role: "system", content: system },
            Msg { role: "user",   content: prompt },
        ],
        temperature: 0.7,
        max_tokens: settings.max_tokens,
    };

    let api_key = settings.api_key.as_deref().unwrap_or("");
    let resp = timed_post(
        client,
        &format!("{}/v1/chat/completions", settings.base_url.trim_end_matches('/')),
        Some(api_key),
        &req,
        settings.timeout_secs,
    )
    .await?;

    let body: Resp = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("failed to parse OpenAI response: {e}")))?;

    body.choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| AppError::Ollama("OpenAI returned empty choices".into()))
}

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

async fn timed_post<T: serde::Serialize>(
    client: &reqwest::Client,
    url: &str,
    bearer_token: Option<&str>,
    body: &T,
    timeout_secs: u64,
) -> Result<reqwest::Response> {
    let mut req = client.post(url).json(body);
    if let Some(token) = bearer_token {
        if !token.is_empty() {
            req = req.bearer_auth(token);
        }
    }

    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        req.send(),
    )
    .await
    .map_err(|_| AppError::Ollama("request timed out".into()))?
    .map_err(|e| AppError::Ollama(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(AppError::Ollama(format!(
            "AI provider returned HTTP {status}: {}",
            &body_text[..body_text.len().min(200)]
        )));
    }

    Ok(resp)
}
