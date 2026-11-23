---
title: AI assistant
summary: The optional AI features, where the key is kept, what AI is used for and why it never changes a recipe by itself.
audience: admin
order: 78
---

AI is optional and **off by default**. Nothing in the app needs it. When it's on, it only ever makes a proposal that someone reviews and accepts; the AI code (`crates/core/src/ai.rs`) never writes to the recipes folder.

## Turning it on

Settings → AI assistant on the Mac (admin only):

| Setting | What it does |
|---|---|
| Enable AI features | `settings.ai.enabled`. While off, every AI endpoint answers `404` and the AI buttons and menu items are hidden. |
| Provider | **Anthropic (Claude)** or **OpenAI-compatible** |
| Model | Anthropic: empty means `claude-opus-5-5`. OpenAI-compatible: required, for example the model name your server lists. |
| API base URL | OpenAI-compatible only. Empty means `https://api.openai.com/v1`. |
| API key | saved with **Save key**; shown only as "(saved)" afterwards |

## Providers

- **Anthropic**: the Messages API at `https://api.anthropic.com/v1/messages`, called over plain HTTP (there's no official Rust SDK). Needs a key.
- **OpenAI-compatible**: `POST <base URL>/chat/completions`, with the key as a bearer token. The base URL must start with `https://`, or be `http://` on this Mac's loopback address (`localhost`, `127.0.0.1`, `[::1]`).
- **A model running on the Mac** (Ollama, LM Studio): choose OpenAI-compatible and enter its local address, for example `http://localhost:11434/v1`. No key is needed for a loopback address; one is sent if saved.

## Where the key is kept

In the macOS Keychain, service `family.recipes.desktop`, account `ai-api-key` (`PUT /api/settings/ai-key`; an empty key deletes it). It is never written to `settings.json`, the recipes folder or git, and never sent to phones or tablets: requests to the provider are made by the Mac. Each update is a new ad-hoc signed build, so macOS may ask once more for Keychain access after updating; choose **Always Allow**.

## What AI is used for

| Where | What it does | Endpoint |
|---|---|---|
| Import, **Fix with AI** | Corrects an imported draft against the text taken from the source (web page, PDF, Word file), using it to restore anything the draft lost or garbled. | `POST /api/ai/fix` |
| A recipe's **More** menu, **Tidy formatting with AI…** | Reformats an existing recipe into the app's layout, formatting only. | `POST /api/ai/tidy` |
| A recipe's **More** menu, **Ask AI about this recipe…** | Answers a question about the open recipe in a few sentences; if the recipe doesn't say, the answer says so and marks general advice as a suggestion. | `POST /api/ai/ask` |

Fix and tidy share one set of instructions: keep the recipe's own words, quantities, temperatures and times exactly; don't invent, paraphrase, summarise or convert units; only reorder lines into sections, split or join lines, and remove website clutter and leftover HTML; keep existing frontmatter, images and links.

All three need editing rights (the desktop app, or a device signed in with the family PIN). The More menu items show only on Markdown notes, and only when AI is on and the person can edit.

## It only proposes

- **Fix** and **Tidy** open a side-by-side comparison: the current text on the left, the AI suggestion on the right, editable. Nothing changes until **Use suggestion** (import) or **Save suggestion** (recipe). Saving a tidied recipe records "Accepted an AI suggestion" in its history.
- **Ask** shows the answer under the question and never touches the file.
- AI only ever sees and proposes text. The file a recipe was imported from is kept either way, and a tidied recipe keeps its `original` link even if the suggestion dropped it.
- AI requests are not written to the activity log, since they change nothing.

## When it fails

Errors from the provider come back as `502` with its message, for example "no API key saved — add one in Settings on the Mac", "the AI declined this request", "the AI's answer was cut off (too long)" or "choose a model in Settings". Requests time out after 5 minutes.
