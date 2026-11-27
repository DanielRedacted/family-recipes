# Parts kept from the earlier projects

Before this app there were two others: `recipes` (Rust + SQLite + a local Ollama model, served
through Docker and Caddy) and `cookbook` (a Rust rewrite around a folder of Markdown, abandoned
after its first phases). Both were deleted on 2026-11-15. This folder keeps the parts worth
integrating later and a backlog of their ideas that this app doesn't have.

Nothing here is built or tested. The files are copied unchanged, so they still refer to their old
crates and paths. Deployment files for Linux are in [deploy/linux/](../../deploy/linux/README.md).
The earlier projects' full history is in the private GitHub repositories `recipes` and
`cookbook`; `cookbook`'s converted recipes are on the `cookbook` branch of the recipes repository.

## Files

| Here | From | What it is |
| --- | --- | --- |
| `jsonld-export/` | `cookbook/src/recipe/` | The recipe model of `cookbook`, with **schema.org JSON-LD export** (`jsonld.rs`) and its tests (`roundtrip_tests.rs`, including a proptest). The other files are the types it needs. |
| `SPEC-markdown.md` | `cookbook/docs/` | `cookbook`'s file format: extra frontmatter fields and the export rules checked against Google's recipe documentation. |
| `fraction.rs` | `cookbook/src/` | Exact fractions, kitchen rounding, fifths and sixths. |
| `slug.rs` | `cookbook/src/vault/` | File-name slugs that fold accents ("Crème Brûlée" → `creme-brulee`). |
| `ai-extraction/prompts.rs` | `recipes/src/ollama/prompt.rs` | Per-field extraction prompts (metadata, ingredients, review, instructions, gap fill, correction), each insisting on verbatim copying, with worked examples. Also a schema.org reader and fraction parsing. |
| `ai-extraction/pipeline.rs` | `recipes/src/ai/mod.rs` | The multi-pass pipeline that used those prompts, with retries, JSON mode, and Ollama / OpenAI / Anthropic request shapes. |
| `ai-extraction/ollama-prompt.md` | `recipes/docs/` | The first single-prompt design and a table of small local models' quirks. |
| `backup-export.rs` | `recipes/src/api/data.rs` | One-click export zip, restore, and "erase all data". |
| `cuisines-categories.sql` | `recipes/migrations/009_…` | Fixed lists of 42 cuisines and 30 categories. |
| `food-emoji-reference.txt` | `recipes/src/data/` | 131 food emoji with what each suits. |
| `fixtures/` | `recipes/tests/fixtures/` | Made-up test pages: with JSON-LD, without it, plain text, and a mock AI answer. Never used by a test. |

## Backlog

Ideas the earlier projects had, built or planned, that this app doesn't have. None is promised.

### Cooking

- ~~Temperature conversion~~ and ~~cups ↔ grams per ingredient~~: built (2026-11-26). Metric mode
  shows °C and weighs solids with the USDA table (`toCelsius`, `solidGramsPerMl` in `ui/src/lib/scale.ts`).
- Timers and a kitchen voice assistant: shelved (see "Decided against, on purpose").

### Recipes and links

- **Unlinked mentions.** Notice when a recipe's text names another recipe ("pie crust") and offer
  to turn it into a `[[link]]` with one click.
- **Rename updates links.** Renaming a recipe could rewrite `[[links]]` to it in other recipes.
- **Tags everywhere.** Rename or delete a tag across all recipes. `recipes` had this.
- **Cuisine and category suggestions** from the fixed lists in `cuisines-categories.sql`, offered
  while typing tags.
- **Random recipe** button. `recipes` had one.
- **Table view.** A sortable, filterable table of recipes by their properties (times, servings,
  tags), like Obsidian Bases.

### Import and export

- **JSON-LD export** for sharing or printing with structured data. `jsonld-export/` has working
  code and the rules: steps as `HowToStep` without "Step 1:" prefixes, bold labels as
  `HowToSection`, tags that don't repeat the category or cuisine, nutrition only with a yield.
  This app's files may hold several ingredient sets and portion tables, which one schema.org
  `Recipe` can't express, so export needs a rule for those first.
- **Backup zip.** Download every recipe, attachment and setting as one file, and restore from it.
  The folder plus git covers this for the owner; a zip is for anyone else. See `backup-export.rs`.
- **Import history** that survives a restart: what was imported, from where, and when. Today only
  the `source:` property remains.
- **Imported images:** resize to about 1600 px and keep the image's original address.
- **Fields the importer drops:** rating, date published, cost, video address. Cuisine, category
  and diet become tags rather than separate fields.
- **Test pages:** a collection of real, awkward schema.org pages (`@graph`, top-level arrays,
  yields as number, text or list, authors and images in every shape, instructions as text,
  `HowToStep` or `HowToSection`) as regression tests. food.com is a good source of doubly encoded
  HTML entities. `fixtures/` is a start.

### Small gaps

- Fraction glyphs ⅕ ⅖ ⅗ ⅘ ⅙ ⅚ are not read as amounts (`units.ts`, `scale.ts`, `recipe.rs`,
  `import.rs` know halves, thirds, quarters and eighths). No recipe uses them today.
  `fraction.rs` has the full set.
- Not from the earlier projects, but found while checking scaling: "1 - 15 oz can of chickpeas"
  and "3 - 15 oz cans" read as the ranges 1–15 and 3–15, so they scale wrongly (`QTY_RX` in
  `scale.ts`; the server's `ingredient::parse_line` reads them right, for nutrition). So does "1 8 oz package cream cheese" ("½ 8 oz package" at ×½).
- `import.rs` `slug()` doesn't fold accents for attachment names. `slug.rs` does.
- No property-based tests; `cookbook` used `proptest`.
- Card colours come from a hash of the name. `recipes` picked them by food (red for tomato and
  berries, orange for pumpkin and citrus, brown for chocolate and bread; see `ENRICHMENT_BASE` in
  `ai-extraction/prompts.rs`), which could become a word table like the emoji one.
- A Raspberry Pi build: see `deploy/linux/README.md`.

### AI extraction

The app sends one prompt that tidies a whole Markdown file. `recipes` made one request per field
instead and checked each answer. If extraction is ever moved to AI, start from
`ai-extraction/prompts.rs`, in particular the ingredient rules ("quantity contains a NUMBER only",
size words go in the preparation, lines starting "Or" are alternatives, not ingredients) and the
instruction rules ("do not reword, shorten, combine… if a recipe has no instructions, do not
generate any"). `recipes` also had the AI write descriptions and choose emoji; writing a
description breaks the rule that no information is added to a recipe, so it was not kept.

### Lessons

- Cutting a string at a byte count (`&s[..n]`) panics in the middle of "½" or "é".
- A contentless SQLite FTS5 table (`content=''`) returns empty columns, so every search found
  nothing.
- Small local models leave fields out instead of writing null, and wrap JSON in code fences.
- The docs of `recipes` described a shared chat session that the code never had. Docs change in
  the same commit as the code.

### Decided against, on purpose

So these aren't reopened by accident: cloud AI is allowed (optional, off by default); imports
accept PDFs, Word files, photos and text, not only JSON-LD; editing needs a PIN on other devices;
the app never rewrites a recipe into a standard layout; one file may hold several ingredient sets
and portion tables; times are written for people (`1 hr 20 min`), not ISO 8601; files are named
by title, not slug; the UI has a build step (Svelte); no cooking timers or voice assistant in the
app, since phones' Clock and Siri already do both and keep ringing while the phone is locked
(shelved 2026-11-26, see [VOICE.md](../../VOICE.md)).
