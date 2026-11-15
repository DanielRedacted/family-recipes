use crate::db::models::{ParsedIngredient, SchemaOrgIngredient, SchemaOrgRecipe};

// ---------------------------------------------------------------------------
// Section system prompts
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Section-specific system prompts
// ---------------------------------------------------------------------------

pub const METADATA_SYSTEM_PROMPT: &str = r#"You are a recipe data extraction agent. Given raw article text, extract ONLY the recipe
metadata and return a valid JSON object. Do not include ingredients, instructions,
nutrition, or rating data.

Return ONLY this JSON structure (set fields to null when not present):

{
  "name": "string — recipe title (REQUIRED)",
  "description": "string — the recipe's introductory description or headnote, copied verbatim from the source text. Do NOT write your own summary. If no description appears in the source, use null.",
  "author": "string — author name, or null",
  "image": "string — image URL, or null",
  "url": "string — original URL if present, or null",
  "datePublished": "string — publication date if present, or null",
  "prepTime": "ISO 8601 duration, e.g. PT15M, or null",
  "cookTime": "ISO 8601 duration, e.g. PT30M, or null",
  "totalTime": "ISO 8601 duration, e.g. PT45M, or null",
  "performTime": "ISO 8601 active cooking time if distinct from cookTime, or null",
  "recipeYield": "string, e.g. '4 servings' or '12 cookies', or null",
  "cookingMethod": "string, e.g. 'Baking', 'Frying', 'Grilling', or null",
  "recipeCategory": "string, e.g. 'Dessert', 'Main course', or null",
  "recipeCuisine": "string, e.g. 'Italian', 'Mexican', or null",
  "keywords": "comma-separated string of tags, or null",
  "suitableForDiet": "comma-separated applicable diets from: VeganDiet, VegetarianDiet, GlutenFreeDiet, DiabeticDiet, LowCalorieDiet, LowFatDiet, LowSaltDiet, HalalDiet, KosherDiet, LowLactoseDiet — or null"
}

Rules:
- VERBATIM: copy all text fields (name, description, author, recipeYield, cookingMethod,
  recipeCategory, recipeCuisine, keywords) exactly as they appear in the source text.
  Do NOT paraphrase, summarize, reword, or elaborate.
- The only fields that require format transformation are the time fields (prepTime, cookTime,
  totalTime, performTime): convert human-readable durations to ISO 8601 (e.g. "30 minutes" → "PT30M").
  All other fields must be transcribed verbatim.
- If a field is not explicitly present in the source text, use null — never invent or infer data.
- Do NOT include recipeIngredient, recipeInstructions, tool, supply, nutrition, or aggregateRating."#;

pub const INGREDIENTS_SYSTEM_PROMPT: &str = r#"You are a recipe ingredient extraction agent. Given raw article text, extract ONLY
the ingredient list and return a valid JSON object.

Return ONLY this JSON structure:

{
  "recipeIngredient": [
    {
      "quantity": "fraction or whole number as string, e.g. '1/2' or '2', or null",
      "unit": "unit of measure, e.g. 'tsp', 'tbsp', 'fl oz', 'cup', 'pint', 'quart', 'gal', 'ml', 'l','mg', 'milligram', 'g', 'kg', 'oz', 'lb', 'pinch', 'dash', 'smidgen', 'handful', 'drop', 'stick', 'clove', 'head', 'slice', 'piece', 'sprig', 'sprigs', 'stalk', 'stalks', 'ear', 'fillet', 'dozen', 'score', etc. or null"
      "name": "plain ingredient name, lowercase singular",
      "preparation": "preparation note, e.g. 'minced', 'chopped', 'at room temperature', or null"
    }
  ]
}

Rules — these are CRITICAL:
- VERBATIM: copy ingredient name, quantity, unit, and preparation notes exactly as written in the source text.
  Do NOT rephrase, or use synonyms. Transcribe what the recipe says.
- name: plain ingredient ONLY — strip the quantity, unit, and preparation out into their own fields,
  but keep the ingredient name itself as it appears in the source.
- quantity: fraction string ("1/2", "3/4", "1 1/2") or whole number string ("2") or a decimal ("0.5", "1.5").
- name: always lowercase and singular ("tomato" not "Tomatoes", "egg" not "Eggs").
- preparation: copy the preparation descriptor verbatim from the source ("finely diced", "at room temperature") — no quantities.
- If an ingredient has no quantity (e.g. "salt to taste"), set quantity and unit to null.
- Do NOT include any fields other than recipeIngredient.

SEPARATION RULES — read carefully, these are the most common mistakes:
- quantity contains a NUMBER only. Never include the unit word inside quantity.
  RIGHT: {"quantity":"1","unit":"Tbsp","name":"chili powder","preparation":null}
- unit is null when there is no unit word (e.g. "2 eggs" → quantity: 2, unit: null, name: eggs).
- Size adjectives like "small", "large", "medium" go in preparation, not in quantity.
  RIGHT: {"quantity":"1","unit":null,"name":"onion","preparation":"small, chopped"}
- Lines beginning with "Or" or "or" are alternative suggestions, not ingredients — skip them.
- Use JSON null (not empty string "") for any missing value.

Examples (input line → output object):

"1 1/2 lb. ground beef"
→ {"quantity":"1 1/2","unit":"lb","name":"ground beef","preparation":null}

"1 small onion, chopped"
→ {"quantity":"1","unit":null,"name":"onion","preparation":"small, chopped"}

"2 cloves garlic, minced"
→ {"quantity":"2","unit":"cloves","name":"garlic","preparation":"minced"}

"1/2 tsp ground black pepper"
→ {"quantity":"1/2","unit":"tsp","name":"ground black pepper","preparation":null}

"6 oz tomato paste"
→ {"quantity":"6","unit":"oz","name":"tomato paste","preparation":null}

"salt to taste"
→ {"quantity":null,"unit":null,"name":"salt","preparation":"to taste"}"#;

pub const INSTRUCTIONS_SYSTEM_PROMPT: &str = r#"You are a recipe instruction extraction agent. Given raw article text, extract the
cooking steps, equipment, and optional nutrition/rating data, and return a valid JSON object.

Return ONLY this JSON structure (set fields to null when not present):

{
  "recipeInstructions": ["Step 1 text.", "Step 2 text.", "Step 3 text."],
  "tool": ["utensil or equipment name 1", "utensil or equipment name 2"],
  "supply": ["consumable supply name 1"],
  "nutrition": {
    "@type": "NutritionInformation",
    "servingSize": "string, e.g. '1 slice', or null",
    "calories": "string, e.g. '250 calories', or null",
    "fatContent": "string, e.g. '12 g', or null",
    "saturatedFatContent": "string or null",
    "carbohydrateContent": "string or null",
    "fiberContent": "string or null",
    "sugarContent": "string or null",
    "proteinContent": "string or null",
    "sodiumContent": "string or null",
    "cholesterolContent": "string or null"
  },
  "aggregateRating": {
    "@type": "AggregateRating",
    "ratingValue": "number as string, e.g. '4.5', or null",
    "ratingCount": "integer as string, or null",
    "bestRating": "5"
  }
}

Rules:
- VERBATIM: copy each instruction step exactly as it appears in the source text.
  Do NOT reword, shorten, combine, or elaborate on any step. Transcribe every step word-for-word.
- If a recipe does not have any instructions or steps, do not generate any.
- recipeInstructions: flat array of plain step strings — NEVER objects, NEVER HowToStep wrappers.
- tool: copy equipment names verbatim from the source (e.g. "large cast-iron skillet", "balloon whisk").
- nutrition values: copy numbers and units verbatim from the source (e.g. "12 g", "250 calories").
  Include ONLY if the source text explicitly states this data. Never invent or calculate nutrition values.
- aggregateRating: copy the rating value verbatim. Include ONLY if a rating appears in the source text.
- If a field has no data, use null."#;

pub const CORRECTION_METADATA_PROMPT: &str =
    "The previous response was not valid JSON. Return ONLY the metadata JSON object \
     (name, description, author, timing, cuisine, etc.) with no markdown, no explanation, \
     and no text before or after the opening brace.";

pub const CORRECTION_INGREDIENTS_PROMPT: &str =
    "Your previous response had formatting errors. Return ONLY the JSON object \
     { \"recipeIngredient\": [...] } with no markdown, no explanation, \
     and no text before or after the opening brace. Fix these issues: \
     (1) quantity must contain a number only — never include the unit word inside quantity; \
     (2) use JSON null, not empty string \"\", for missing fields; \
     (3) skip any line that begins with \"Or\" or \"or\".";

pub const CORRECTION_INSTRUCTIONS_PROMPT: &str =
    "The previous response was not valid JSON. Return ONLY the instructions JSON object \
     (recipeInstructions, tool, supply, nutrition, aggregateRating) with no markdown, no explanation, \
     and no text before or after the opening brace. \
     Remember: recipeInstructions must be a flat array of plain strings, never objects.";

/// Build a metadata system prompt that constrains recipeCuisine and recipeCategory
/// to the provided canonical lists. The AI is told to use null if none of the listed
/// values apply — it must never invent a new cuisine or category.
pub fn build_metadata_system_prompt(cuisines: &[String], categories: &[String]) -> String {
    let mut prompt = METADATA_SYSTEM_PROMPT.to_string();
    if !cuisines.is_empty() {
        prompt.push_str(&format!(
            "\n\nAllowed values for recipeCuisine (choose the closest match, or null if none fit): {}.",
            cuisines.join(", ")
        ));
    }
    if !categories.is_empty() {
        prompt.push_str(&format!(
            "\nAllowed values for recipeCategory (choose the closest match, or null if none fit): {}.",
            categories.join(", ")
        ));
    }
    prompt
}

/// Build constraint notes to append to the gap-fill system prompt.
pub fn build_gap_fill_constraints(cuisines: &[String], categories: &[String]) -> String {
    let mut s = String::new();
    if !cuisines.is_empty() {
        s.push_str(&format!(
            "\n\nAllowed values for recipeCuisine: {}. Use null if none apply.",
            cuisines.join(", ")
        ));
    }
    if !categories.is_empty() {
        s.push_str(&format!(
            "\nAllowed values for recipeCategory: {}. Use null if none apply.",
            categories.join(", ")
        ));
    }
    s
}

pub const GAP_FILL_SYSTEM_PROMPT: &str = r#"You are a recipe data extraction agent.
Given the original article text and a list of fields, extract ONLY the listed fields
and return a valid JSON object containing only those fields.

Rules:
- VERBATIM: copy all text fields exactly as they appear in the source text.
  Do NOT paraphrase, summarize, reword, or elaborate. Transcribe what is written.
  The only exception is time fields (prepTime, cookTime, totalTime): convert to ISO 8601.
- Return ONLY the fields explicitly listed in the prompt. Do NOT include any other fields.
- Set a field to null if genuinely not present in the source text. Never invent data."#;

pub const INGREDIENTS_REVIEW_SYSTEM_PROMPT: &str = r#"You are a recipe ingredient validator. You will receive a parsed ingredient list as JSON.
Review each ingredient object against the schema rules and return a fully corrected version.

The expected schema for each ingredient object:
{
  "quantity": "fraction or whole number as string, e.g. '1/2' or '2', or null",
  "unit": "unit of measure, e.g. 'tsp', 'tbsp', 'fl oz', 'cup', 'pint', 'quart', 'gal', 'ml', 'l','mg', 'milligram', 'g', 'kg', 'oz', 'lb', 'pinch', 'dash', 'smidgen', 'handful', 'drop', 'stick', 'clove', 'head', 'slice', 'piece', 'sprig', 'sprigs', 'stalk', 'stalks', 'ear', 'fillet', 'dozen', 'score', etc. or null"
  "name": "plain ingredient name, lowercase singular",
  "preparation": "preparation note, e.g. 'minced', 'chopped', or null"
}

Return ONLY this JSON structure (same shape as input, but corrected):
{
  "recipeIngredient": [ ... ]
}

Validation rules — fix every violation you find:
- quantity must contain a NUMBER only. Never include the unit word inside quantity.
- unit is null when there is no unit word. "2 eggs" → unit: null.
- Size adjectives ("small", "large", "medium") belong in preparation, not quantity.
- name must be lowercase and singular ("tomato" not "Tomatoes", "egg" not "Eggs").
- Use JSON null (not empty string "") for any missing field.
- Do NOT add, remove, or reorder ingredients — only correct existing field values."#;

pub fn build_ingredients_review_prompt(parsed_json: &str) -> String {
    format!(
        "Review and correct the following parsed ingredient list. \
         Fix any schema violations — do not add or remove ingredients.\n\n\
         Parsed ingredients:\n{parsed_json}"
    )
}

pub const CORRECTION_GAP_FILL_PROMPT: &str =
    "The previous response was not valid JSON. Return ONLY a JSON object containing the \
     requested missing fields, with no markdown, no explanation, and no text before or after \
     the opening brace. quantity in recipeIngredient must be a fraction string, never a decimal. \
     recipeInstructions must be a flat array of plain strings, never objects.";

const FOOD_EMOJI_REFERENCE: &str =
    include_str!("../data/food_emoji_reference.txt");

const ENRICHMENT_BASE: &str = r#"You are a recipe visual enrichment agent. Given a recipe name and basic metadata, assign 1 to 3 representative emojis and a color theme.

Use the food emoji reference below to select the most fitting emojis for this dish. Choose as many as are genuinely relevant (minimum 1, maximum 3), ordered from most to least representative.

Color selection guide:
- red: tomato-based, meat, berry, or bold savory dishes
- orange: pumpkin, carrot, citrus, squash, or spicy dishes
- yellow: egg, lemon, corn, curry, or golden-baked dishes
- green: salad, herb, vegetable, avocado, or fresh dishes
- blue: blueberry, seafood, or cool refreshing dishes
- purple: beet, grape, eggplant, or lavender dishes
- brown: chocolate, coffee, bread, stew, mushroom, or hearty dishes
- white: dairy, cream, pasta, rice, fish, or neutral dishes"#;

/// Used when the recipe already has a description — only emojis and color are requested.
pub fn enrichment_system_prompt() -> String {
    format!(
        "{}\n\nFood emoji reference:\n{}\n\nReturn ONLY this JSON object:\n{{\n  \"emojis\": [\"most representative emoji\", \"second emoji if relevant\", \"third emoji if relevant\"],\n  \"color\": \"exactly one of: red, orange, yellow, green, blue, purple, brown, white\"\n}}\n\nRules:\n- emojis: array of 1 to 3 single emoji characters; each entry is one emoji only, no text\n- order from most to least representative; do not pad to 3 if fewer genuinely fit\n- color: exactly one word from the list above, lowercase",
        ENRICHMENT_BASE,
        FOOD_EMOJI_REFERENCE,
    )
}

/// Used when the recipe has no description — emojis, color, and a generated description are requested.
pub fn enrichment_with_desc_system_prompt() -> String {
    format!(
        "{}\n\nFood emoji reference:\n{}\n\nReturn ONLY this JSON object:\n{{\n  \"emojis\": [\"most representative emoji\", \"second emoji if relevant\", \"third emoji if relevant\"],\n  \"color\": \"exactly one of: red, orange, yellow, green, blue, purple, brown, white\",\n  \"description\": \"1-2 casual appetizing sentences describing the dish\"\n}}\n\nRules:\n- emojis: array of 1 to 3 single emoji characters; each entry is one emoji only, no text\n- order from most to least representative; do not pad to 3 if fewer genuinely fit\n- color: exactly one word from the list above, lowercase\n- description: keep it under 40 words; do not start with the recipe name",
        ENRICHMENT_BASE,
        FOOD_EMOJI_REFERENCE,
    )
}

pub fn build_enrichment_prompt(name: &str, category: &str, cuisine: &str) -> String {
    format!("Recipe name: {name}\nCategory: {category}\nCuisine: {cuisine}")
}

// ---------------------------------------------------------------------------
// Unicode fraction normalisation
// ---------------------------------------------------------------------------

/// Replace Unicode fraction characters with their ASCII equivalents so the AI
/// always sees plain strings like "1/2" rather than "½".  Called on source text
/// before it is embedded in any prompt, and also inside `parse_fraction_str` as
/// a defensive fallback.
pub fn normalise_fractions(text: &str) -> String {
    text.replace('½', "1/2")
        .replace('⅓', "1/3")
        .replace('⅔', "2/3")
        .replace('¼', "1/4")
        .replace('¾', "3/4")
        .replace('⅕', "1/5")
        .replace('⅖', "2/5")
        .replace('⅗', "3/5")
        .replace('⅘', "4/5")
        .replace('⅙', "1/6")
        .replace('⅚', "5/6")
        .replace('⅛', "1/8")
        .replace('⅜', "3/8")
        .replace('⅝', "5/8")
        .replace('⅞', "7/8")
}

// ---------------------------------------------------------------------------
// User-prompt builders
// ---------------------------------------------------------------------------

pub fn build_metadata_prompt(raw_text: &str, max_chars: usize) -> String {
    let normalised = normalise_fractions(raw_text);
    let truncated = &normalised[..normalised.len().min(max_chars)];
    format!(
        "Extract the recipe metadata (title, timing, cuisine, yield, etc.) from the following text. \
         Copy all text fields verbatim — do not paraphrase or summarize. \
         Do NOT extract ingredients or instructions.\n\n---\n{}\n---",
        truncated
    )
}

pub fn build_ingredients_prompt(raw_text: &str, max_chars: usize) -> String {
    let normalised = normalise_fractions(raw_text);
    let truncated = &normalised[..normalised.len().min(max_chars)];
    format!(
        "Extract ONLY the ingredient list from the following recipe text. \
         Copy ingredient names and preparation notes verbatim — do not rephrase.\n\n---\n{}\n---",
        truncated
    )
}

pub fn build_instructions_prompt(raw_text: &str, max_chars: usize) -> String {
    let normalised = normalise_fractions(raw_text);
    let truncated = &normalised[..normalised.len().min(max_chars)];
    format!(
        "Extract ONLY the cooking instructions, equipment, and any nutrition/rating data \
         from the following recipe text. \
         Copy every instruction step verbatim — do not reword, shorten, or combine steps.\n\n---\n{}\n---",
        truncated
    )
}

pub fn build_gap_fill_prompt(
    raw_text: &str,
    null_fields: &[(&str, &str)],
    max_chars: usize,
) -> String {
    let field_list = null_fields
        .iter()
        .map(|(name, desc)| format!("  - {name}: {desc}"))
        .collect::<Vec<_>>()
        .join("\n");
    let normalised = normalise_fractions(raw_text);
    let truncated = &normalised[..normalised.len().min(max_chars)];
    format!(
        "Extract ONLY the following fields from the recipe text. \
         Copy all text verbatim — do not paraphrase or invent. Set to null if not present.\n\n\
         Missing fields:\n{field_list}\n\n---\n{truncated}\n---"
    )
}

// ---------------------------------------------------------------------------
// Schema.org extraction from HTML
// ---------------------------------------------------------------------------

/// Extract a Schema.org Recipe from any `<script type="application/ld+json">` block in the HTML.
/// Returns `(parsed_recipe, raw_json_of_the_recipe_object)` so callers can persist
/// the original JSON-LD blob unmodified.
///
/// Handles three common embedding patterns:
/// - Top-level Recipe object   `{ "@type": "Recipe", ... }`
/// - Top-level array           `[{ "@type": "Recipe", ... }, ...]`
/// - `@graph` wrapper          `{ "@graph": [ { "@type": "Recipe", ... }, ... ] }`
pub fn extract_schema_org(html: &str) -> Option<(SchemaOrgRecipe, String)> {
    use scraper::{Html, Selector};

    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;

    for element in doc.select(&sel) {
        // Use .text() rather than .inner_html() — inner_html() re-serialises the
        // DOM and re-encodes & → &amp;, < → &lt;, etc., breaking JSON parsing.
        let raw: String = element.text().collect();
        // Always unescape HTML entities before JSON parsing. Many CMSes (WordPress,
        // Yoast, Wix, etc.) incorrectly store HTML entities inside JSON-LD <script>
        // tags. Because JSON-LD is valid JSON, serde happily parses
        // `{"name":"Tom &amp; Jerry"}` and stores the literal string "Tom &amp; Jerry".
        // Unconditional unescape fixes both the broken-JSON case and the
        // parses-but-has-entities case.
        let json_text = html_escape::decode_html_entities(&raw).into_owned();
        let val: serde_json::Value = match serde_json::from_str(json_text.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Flatten the value into a list of candidate objects.
        let candidates: Vec<serde_json::Value> = match &val {
            serde_json::Value::Array(arr) => arr.clone(),
            serde_json::Value::Object(obj) => {
                // @graph wrapper used by Yoast SEO and many WordPress themes.
                if let Some(serde_json::Value::Array(graph)) = obj.get("@graph") {
                    graph.clone()
                } else {
                    vec![val.clone()]
                }
            }
            _ => continue,
        };

        for candidate in candidates {
            let is_recipe = match candidate.get("@type") {
                Some(serde_json::Value::String(t)) => t == "Recipe",
                Some(serde_json::Value::Array(arr)) => {
                    arr.iter().any(|v| v.as_str() == Some("Recipe"))
                }
                _ => false,
            };
            if !is_recipe {
                continue;
            }
            if let Ok(recipe) = serde_json::from_value::<SchemaOrgRecipe>(candidate.clone()) {
                // Serialise the candidate back to a compact string for storage.
                let raw = serde_json::to_string(&candidate)
                    .unwrap_or_else(|_| json_text.trim().to_string());
                return Some((recipe, raw));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// ISO 8601 duration parsing
// ---------------------------------------------------------------------------

pub fn parse_iso_duration_to_mins(s: &str) -> Option<i64> {
    if !s.starts_with("PT") && !s.starts_with("P") {
        return None;
    }
    // Strip leading P, then handle T separator
    let s = s.trim_start_matches('P');
    let s = if let Some(t_pos) = s.find('T') {
        &s[t_pos + 1..]
    } else {
        s
    };

    let mut mins = 0i64;
    let mut current = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else {
            if let Ok(n) = current.parse::<i64>() {
                match ch {
                    'H' => mins += n * 60,
                    'M' => mins += n,
                    _ => {}
                }
            }
            current.clear();
        }
    }
    if mins > 0 {
        Some(mins)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Ingredient quantity parsing
// ---------------------------------------------------------------------------

/// Strip everything from a quantity string except digits, `.`, `/`, and spaces,
/// then collapse runs of whitespace. "1 Tbsp" → "1", "1 1/2 cups" → "1 1/2".
fn clean_quantity_str(s: &str) -> String {
    let filtered: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '/' || *c == ' ')
        .collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse SchemaOrgIngredient list into ParsedIngredient list
pub fn parse_ingredients(raw: &[SchemaOrgIngredient]) -> Vec<ParsedIngredient> {
    raw.iter()
        .enumerate()
        .filter_map(|(i, ing)| {
            let name = ing.name.as_deref()?.trim().to_lowercase();
            if name.is_empty() {
                return None;
            }
            let (num, den) = ing
                .quantity
                .as_deref()
                .map(clean_quantity_str)
                .as_deref()
                .and_then(|s| if s.is_empty() { None } else { parse_fraction_str(s) })
                .unwrap_or((None, None));
            Some(ParsedIngredient {
                name,
                qty_numerator: num,
                qty_denominator: den,
                unit: ing.unit.as_deref().filter(|s| !s.trim().is_empty()).map(str::to_string),
                preparation: ing.preparation.as_deref().filter(|s| !s.trim().is_empty()).map(str::to_string),
                sort_order: i as i64,
            })
        })
        .collect()
}

/// Parse "1/2", "3/4", "2", "1 1/2" into (numerator, denominator).
/// Also accepts Unicode fraction characters (½, ¾, ⅛ …) by normalising them first.
pub fn parse_fraction_str(s: &str) -> Option<(Option<i64>, Option<i64>)> {
    let normalised;
    let s = if s.chars().any(|c| "½⅓⅔¼¾⅕⅖⅗⅘⅙⅚⅛⅜⅝⅞".contains(c)) {
        normalised = normalise_fractions(s.trim());
        normalised.as_str()
    } else {
        s.trim()
    };
    if s.is_empty() {
        return None;
    }

    // Handle "1 1/2" (mixed number)
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 {
        if let (Ok(whole), Some((Some(n), Some(d)))) =
            (parts[0].parse::<i64>(), parse_fraction_str(parts[1]))
        {
            return Some((Some(whole * d + n), Some(d)));
        }
    }

    // Handle "1/2"
    if let Some(slash) = s.find('/') {
        let n: i64 = s[..slash].trim().parse().ok()?;
        let d: i64 = s[slash + 1..].trim().parse().ok()?;
        if d == 0 {
            return None;
        }
        let g = gcd(n.abs(), d.abs());
        return Some((Some(n / g), Some(d / g)));
    }

    // Handle whole number
    if let Ok(n) = s.parse::<i64>() {
        return Some((Some(n), Some(1)));
    }

    None
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}
