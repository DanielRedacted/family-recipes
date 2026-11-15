# Ollama prompt design notes

## Strategy

Two-shot structured prompt with `"format": "json"` enforced at the API level.
Retry up to `config.ollama.max_retries` times on parse failure.

## Prompt evolution log

Keep this file updated as you tune the prompts.

### v1 (initial)
- System: full Schema.org field list with types
- User: raw text prefixed with "Extract the recipe from:"
- Result: good for clean recipe text, occasionally wraps output in markdown fences

### v2 (current)
- Added explicit "Do not include markdown fences or text outside the JSON" instruction
- Added ingredient normalization rules (lowercase, singular, name-only)
- Added quantity fraction string instruction
- Result: reliable JSON output on llama3.1, mistral

## Known model quirks

| Model | Issue | Workaround |
|---|---|---|
| llama3.2:1b | Often omits optional fields entirely instead of null | Accept and treat missing as null in parser |
| mistral:7b | Sometimes wraps response in ```json fences | Strip ``` before parsing |
| gemma2:9b | Good Schema.org adherence, slower than mistral | No workaround needed |

## Stripping markdown fences (safety)

```rust
pub fn clean_json_response(raw: &str) -> &str {
    let raw = raw.trim();
    let raw = raw.strip_prefix("```json").unwrap_or(raw);
    let raw = raw.strip_prefix("```").unwrap_or(raw);
    let raw = raw.strip_suffix("```").unwrap_or(raw);
    // Find first { and last } to extract just the JSON object
    let start = raw.find('{').unwrap_or(0);
    let end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
    &raw[start..end]
}
```

## Context window limits

| Model | Context | Safe text input |
|---|---|---|
| llama3.1:8b | 128k tokens | ~90k chars |
| llama3.1:70b | 128k tokens | ~90k chars |
| mistral:7b | 32k tokens | ~22k chars |
| gemma2:9b | 8k tokens | ~5k chars |

We truncate input to 6,000 chars by default — safe for all models.
Increase `config.ollama.max_input_chars` if using a large-context model.

## Validation after parsing

After deserializing the Ollama response, validate:

1. `name` is non-empty string
2. `recipeIngredient` is an array (can be empty)
3. `recipeInstructions` is an array of strings (not objects)
4. Each ingredient has at least a `name`
5. `quantity` strings parse via `parse_fraction_str()`

If validation fails → retry with correction prompt.
If second attempt also fails → return 422 with the raw Ollama output for debugging.
