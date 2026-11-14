//! Optional AI assistant (off by default). External API key only, kept in the macOS Keychain.
//! Every result is a proposal the user reviews; nothing here writes to the vault.
//!
//! Anthropic: raw HTTP to the Messages API (no official Rust SDK). OpenAI-compatible
//! providers: /chat/completions.

use crate::auth::{Caller, Role};
use crate::server::{AppState, err};
use anyhow::{Result, anyhow, bail};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{post, put};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "family.recipes.desktop";
const KEYCHAIN_USER: &str = "ai-api-key";
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";

type St = State<Arc<AppState>>;
type Res = Result<Response, Response>;

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(Duration::from_secs(300)).connect_timeout(Duration::from_secs(10)).build().expect("http client")
});

fn entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)?)
}

pub fn key_set() -> bool {
    entry().and_then(|e| Ok(e.get_password()?)).is_ok_and(|k| !k.is_empty())
}

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/settings/ai-key", put(set_key))
        .route("/api/ai/fix", post(fix))
        .route("/api/ai/tidy", post(tidy))
        .route("/api/ai/ask", post(ask))
}

#[derive(Deserialize)]
struct KeyBody {
    key: String,
}

async fn set_key(caller: Caller, Json(b): Json<KeyBody>) -> Res {
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
    Ok(StatusCode::NO_CONTENT.into_response())
}

const FORMAT_RULES: &str = "\
You format recipes as plain-text Markdown files for a family cookbook app.

Rules:
- Keep the recipe's own words, quantities, temperatures and times exactly as written. Do not invent, \
paraphrase, summarise, convert units or add anything that is not in the source.
- You may: reorder lines into the right sections, split lines that were merged, join lines that were split, \
remove website clutter (ads, navigation, \"jump to recipe\", ratings, comments, newsletter prompts) and \
remove leftover HTML.
- Layout: optional YAML frontmatter first (keep every existing key and value; you may fill yield, \
prep_time, cook_time, total_time, source or author only from values stated in the source), then \
`# Title`, an optional short description, `## Ingredients` as a `- ` list (use `### Group` subheadings \
only when the source groups ingredients), `## Directions` as a numbered list (with `### Section` \
subheadings when the source has sections), then `## Notes` for remaining tips or variations.
- Keep existing image lines (`![...](...)`) and links unchanged.
- Reply with the complete Markdown file only: no explanation, no code fences.";

#[derive(Deserialize)]
struct FixBody {
    markdown: String,
    #[serde(default)]
    source_text: String,
}

/// Imported draft -> corrected draft (shown to the user as a diff).
async fn fix(State(s): St, caller: Caller, Json(b): Json<FixBody>) -> Res {
    caller.need(Role::Writer)?;
    enabled(&s)?;
    let mut user = String::new();
    if !b.source_text.trim().is_empty() {
        user.push_str(&format!("<source_text>\n{}\n</source_text>\n\n", b.source_text.trim()));
    }
    user.push_str(&format!(
        "<draft>\n{}\n</draft>\n\nFix the draft so it follows the rules, using the source text to restore anything the draft lost or garbled.",
        b.markdown
    ));
    let out = complete(&s, FORMAT_RULES, &user, "medium").await.map_err(ai_err)?;
    Ok(Json(json!({ "markdown": strip_fences(&out) })).into_response())
}

#[derive(Deserialize)]
struct TidyBody {
    markdown: String,
}

/// Existing note -> tidier formatting proposal.
async fn tidy(State(s): St, caller: Caller, Json(b): Json<TidyBody>) -> Res {
    caller.need(Role::Writer)?;
    enabled(&s)?;
    let user = format!("<recipe>\n{}\n</recipe>\n\nReformat this recipe file so it follows the rules. Change formatting only.", b.markdown);
    let out = complete(&s, FORMAT_RULES, &user, "medium").await.map_err(ai_err)?;
    Ok(Json(json!({ "markdown": strip_fences(&out) })).into_response())
}

#[derive(Deserialize)]
struct AskBody {
    markdown: String,
    question: String,
}

async fn ask(State(s): St, caller: Caller, Json(b): Json<AskBody>) -> Res {
    caller.need(Role::Writer)?;
    enabled(&s)?;
    let system = "You help a home cook with one recipe from their family cookbook. Answer the question \
about the recipe below briefly and practically (a few sentences or a short list). If the recipe \
doesn't say, say so and give your best general cooking advice, clearly marked as a suggestion.";
    let user = format!("<recipe>\n{}\n</recipe>\n\n{}", b.markdown, b.question.trim());
    let out = complete(&s, system, &user, "low").await.map_err(ai_err)?;
    Ok(Json(json!({ "answer": out.trim() })).into_response())
}

/// AI endpoints behave as absent while AI is off.
fn enabled(s: &AppState) -> Result<(), Response> {
    if s.settings.read().unwrap().ai.enabled { Ok(()) } else { Err(err(StatusCode::NOT_FOUND, "AI features are turned off (Settings on the Mac)")) }
}

fn ai_err(e: anyhow::Error) -> Response {
    err(StatusCode::BAD_GATEWAY, format!("{e:#}"))
}

fn strip_fences(s: &str) -> String {
    let t = s.trim();
    let t = t.strip_prefix("```markdown").or_else(|| t.strip_prefix("```md")).or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t);
    t.trim().to_string() + "\n"
}

/// One-shot completion with the configured provider.
async fn complete(s: &AppState, system: &str, user: &str, effort: &str) -> Result<String> {
    let ai = s.settings.read().unwrap().ai.clone();
    if !ai.enabled {
        bail!("AI features are turned off (Settings on the Mac)");
    }
    let key = tokio::task::spawn_blocking(|| entry().and_then(|e| Ok(e.get_password()?))).await?.map_err(|_| anyhow!("no API key saved — add one in Settings on the Mac"))?;
    match ai.provider.as_str() {
        "openai" => openai(&ai.base_url, &ai.model, &key, system, user).await,
        _ => anthropic(if ai.model.trim().is_empty() { DEFAULT_ANTHROPIC_MODEL } else { ai.model.trim() }, &key, system, user, effort).await,
    }
}

async fn anthropic(model: &str, key: &str, system: &str, user: &str, effort: &str) -> Result<String> {
    let body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": system,
        "output_config": { "effort": effort },
        // on a safety decline, the API re-runs the request on a fallback model it picks
        "fallbacks": "default",
        "messages": [{ "role": "user", "content": user }],
    });
    let res = HTTP
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2026-10-21")
        .header("anthropic-beta", "server-side-fallback-2026-07-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("could not reach Anthropic: {e}"))?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = v["error"]["message"].as_str().unwrap_or("request failed");
        bail!("Anthropic answered {status}: {msg}");
    }
    match v["stop_reason"].as_str() {
        Some("refusal") => bail!("the AI declined this request"),
        Some("max_tokens") => bail!("the AI's answer was cut off (too long)"),
        _ => {}
    }
    let text: String = v["content"]
        .as_array()
        .map(|blocks| blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if text.trim().is_empty() {
        bail!("the AI returned an empty answer");
    }
    Ok(text)
}

async fn openai(base: &str, model: &str, key: &str, system: &str, user: &str) -> Result<String> {
    let base = if base.trim().is_empty() { "https://api.openai.com/v1" } else { base.trim().trim_end_matches('/') };
    if !base.starts_with("https://") {
        bail!("the API base URL must start with https://");
    }
    if model.trim().is_empty() {
        bail!("choose a model in Settings");
    }
    let body = json!({ "model": model.trim(), "messages": [{ "role": "system", "content": system }, { "role": "user", "content": user }] });
    let res = HTTP.post(format!("{base}/chat/completions")).bearer_auth(key).json(&body).send().await.map_err(|e| anyhow!("could not reach {base}: {e}"))?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("{base} answered {status}: {}", v["error"]["message"].as_str().unwrap_or("request failed"));
    }
    v["choices"][0]["message"]["content"].as_str().map(str::to_string).filter(|t| !t.trim().is_empty()).ok_or_else(|| anyhow!("the AI returned an empty answer"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn fences_stripped() {
        assert_eq!(super::strip_fences("```markdown\n# A\n```"), "# A\n");
        assert_eq!(super::strip_fences("# A"), "# A\n");
    }
}
