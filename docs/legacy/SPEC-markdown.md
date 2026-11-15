# SPEC — canonical recipe markdown format

The vault's file format is the system's public contract: the app, Obsidian, QMD, and git
all read it, and it must round-trip to Schema.org Recipe JSON-LD. **Tolerant in, strict
out**: the ingester accepts any JSON-LD variant it can normalize; the emitter produces
exactly the layout below, always.

## Example file — `recipes/shakshuka.md`

```markdown
---
title: Shakshuka
description: Eggs poached in a spiced tomato-pepper sauce.
source: https://example.com/shakshuka
author: Yotam Ottolenghi
created: 2026-07-07
date_published: 2019-03-12
prep_time: PT15M
cook_time: PT25M
total_time: PT40M
yield: 4 servings
cuisine: Middle Eastern
category: Breakfast
tags: [eggs, tomatoes, one-pan]
diet: []
rating:
  value: 4.8
  count: 312
nutrition:
  calories: 290 kcal
  protein: 14 g
image: ../attachments/shakshuka.jpg
image_source: https://example.com/img/shakshuka-hero.jpg
video: https://example.com/shakshuka.mp4
needs_review: false
---

![Shakshuka](../attachments/shakshuka.jpg)

## Ingredients

- 2 tbsp olive oil
- 1 large onion, thinly sliced
- 1/2 tsp ground cumin
- 4 eggs
- 2 tbsp [[Preserved Lemons]], chopped

## Instructions

1. Heat the oil in a large skillet over medium heat.
2. Add onion and cook until soft, about 8 minutes.
3. Crack the eggs into wells in the sauce and cover.

## Notes

Great with crusty bread. Freezes badly.
```

## Frontmatter schema

Field names are snake_case. Every field except `title` is optional — **omit absent fields
entirely** (never write empty strings/nulls). Order as listed; unknown keys the app didn't
write are **preserved verbatim and re-emitted last** on rewrite (external editors may add
their own).

| Key | Type | JSON-LD mapping | Notes |
|---|---|---|---|
| `title` | string | `name` | required |
| `description` | string | `description` | gap-fill target if missing |
| `source` | string (URL) | `url` | duplicate-detection key |
| `author` | string | `author` (Person → name) | flatten object → name |
| `created` | ISO date | — | date imported; app-owned |
| `date_published` | ISO date | `datePublished` | |
| `prep_time` / `cook_time` / `total_time` / `perform_time` | ISO-8601 duration string (`PT25M`) | `prepTime` etc. | stored verbatim, rendered human ("25 min") |
| `yield` | string | `recipeYield` | normalize array/number → single string |
| `cooking_method` | string | `cookingMethod` | |
| `cuisine` | string | `recipeCuisine` | normalize array → first string |
| `category` | string | `recipeCategory` | normalize array → first string |
| `tags` | string list | `keywords` | normalize comma-string/array → list; gap-fill target |
| `diet` | string list | `suitableForDiet` | strip `https://schema.org/` prefix ↔ restore on emit |
| `rating` | map `{value, count}` | `aggregateRating` | numbers; coerce strings |
| `nutrition` | map of string→string | `nutrition` (NutritionInformation) | keys: calories, fat, saturated_fat, unsaturated_fat, trans_fat, carbohydrate, fiber, sugar, protein, sodium, cholesterol, serving_size |
| `image` | vault-relative path | `image` | local file (downloaded at ingest) |
| `image_source` | string (URL) | — | provenance; original remote URL |
| `video` | string (URL) | `video` (VideoObject → contentUrl) | URL only, not downloaded |
| `estimated_cost` | string | `estimatedCost` | flatten MonetaryAmount → "3.50 USD" |
| `emoji` | string (1–3 emoji) | — | deterministic keyword lookup; app-owned |
| `color` | string (palette name) | — | hash-based; app-owned |
| `needs_review` | bool | — | set true when ingest was uncertain; cleared by user |

`tool`/`supply` from JSON-LD: normalize to strings and append as a `**Equipment:**` line
under `## Notes` (rare in the wild; don't give them structure they don't have).

## Body grammar

Exactly these sections, in order. `## Ingredients` and `## Instructions` are the only
machine-parsed regions; everything else is preserved as-is.

1. Optional lead image line: `![title](../attachments/<slug>.<ext>)`.
2. Optional free prose before the first `##` (preserved, not round-tripped).
3. `## Ingredients` — flat `-` bullet list, **one ingredient per line, verbatim source
   text**. Sub-headings for ingredient groups allowed as bold lines (`- **For the sauce:**`)
   — treated as group labels, not ingredients. `[[Wiki Links]]` may appear anywhere in a
   line.
4. `## Instructions` — ordered list (`1.` `2.` …), one step per item, verbatim source text.
   JSON-LD `HowToSection`s flatten to bold intro lines (`1. **Make the sauce:** …` is NOT
   used — instead the section name becomes its own bold paragraph line between numbered
   runs; numbering restarts per section).
5. Optional `## Notes` — free-form, user-owned, never machine-written except the
   Equipment line at creation.

**Ingredient lines are strings, not structs.** Schema.org's `recipeIngredient` is a string
array; store it that way. The `{quantity, unit, name, preparation}` structure needed by
the serving scaler is derived at render time by the deterministic line parser
(`src-reference/models.rs` → `parse_raw_ingredient_line`, fraction math in
`src-reference/prompt.rs`) and is never written to disk.

## Slug & wiki-link rules

- Slug = filename stem = wiki-link target. Derived from title: lowercase, ASCII-fold
  diacritics, non-alphanumeric runs → single `-`, trim `-`, max 80 chars.
  `Crème Brûlée` → `creme-brulee.md`.
- Collision at ingest: append `-2`, `-3`, … (never overwrite; duplicate detection has
  already warned by this point).
- `[[Target]]` resolves case-insensitively against slugs AND titles; `[[target|display]]`
  supported. Unresolved links render as plain text (styled dim), not errors.
- Renaming a recipe (title change) does NOT rename the file by default — the slug is the
  stable identity. Offer explicit "rename file + update backlinks" as a deliberate action.

## Round-trip contract (enforce with property tests)

1. `emit(parse(jsonld))` is deterministic and stable — same input, byte-identical file.
2. `parse(emit(recipe)) == recipe` for every canonical file (full fidelity of parsed model).
3. Rewrite preserves: unknown frontmatter keys, `## Notes`, pre-section prose, ingredient
   group labels, `[[links]]`.
4. JSON-LD export: strip `[[` `]]` (keep display text), reattach `https://schema.org/`
   diet prefixes, `image` → `image_source` URL if present else omit, emit
   `@context`/`@type: Recipe`.
5. Test corpus: real scraped JSON-LD from actual sites (supplied in the new repo), plus
   adversarial fixtures for every tolerance case: `@graph` wrapper, top-level array,
   yield as number/string/array, author as string/object/array, image as
   URL/object/array, instructions as strings/HowToStep/HowToSection, keywords as
   comma-string/array, HTML entities in text fields.

## Amendments for cookbook (override the above where they conflict)

This SPEC came from the prior project. The following cookbook decisions supersede it:

- **Vault directory is `cookbook/`** at the repo root (not `recipes/`, which holds the
  pristine originals, read-only). Downloaded images live in `cookbook/attachments/`; the
  lead-image path convention `![title](../attachments/<slug>.<ext>)` resolves against it.
- **Folder = category (overrides the "flat vault / frontmatter categories" rule).** A
  recipe's primary category IS its parent folder: `cookbook/bread/bagels.md` → category
  "Bread". The app derives category from the path at scan time and maps it to JSON-LD
  `recipeCategory` on export. **Do not write a `category:` frontmatter key** — the folder
  is the single source of truth, so it can't drift. Uncategorized recipes sit at the vault
  root. Cross-cutting membership (a recipe belonging to several categories) is expressed
  with `tags:`, not folders.
- **Slugs are unique across the entire vault**, not per-folder — so wiki-links and the
  `[[Target]]` resolver are folder-independent and moving a file between category folders
  never breaks a link. Recategorizing = moving the file; identity (the slug) is unchanged.
- **Principle: store as much state as possible in the raw folder structure** (Obsidian-style,
  no Obsidian dependency) so the vault is fully legible in Finder / any markdown reader.

### One file = one Recipe (JSON-LD compatibility rule)

A schema.org Recipe is exactly one `recipeIngredient` array + one instruction list, so a
vault file holds exactly one recipe. When source material bundles several:

- **(A) Split** — variants a cook picks by *name* (vanilla vs chocolate pudding, three
  different sodas, distinct cookie doughs) become separate files, each complete (shared
  method duplicated into each).
- **(B) Keep one** — the same recipe at different sweetness/scale points or with a one-
  ingredient swap keeps a single canonical ingredient list; the other variants are
  preserved verbatim under `## Notes` (not machine-parsed, never exported).
- **(C) Sub-recipe** — a complete prerequisite/companion recipe (a starter, a spice blend,
  a sauce) gets its own file and is referenced from the parent with a `[[wiki-link]]`
  (in the ingredient line where it's consumed, or a `See [[...]]` Note).
- Bold ingredient-group labels are for **components of one recipe** (crust/filling) and
  optional mix-ins — never for parallel variants (that's case A or B).
- **Scale tables: the canonical size is the one the instructions describe** (fall back to
  the larger size when instructions are size-neutral). The serving scaler derives all
  other sizes.

### Export rules validated against Google Recipe docs + schema.org (2026-07-09)

- `## Instructions` items export as `HowToStep{text}` — strip list numbers; never emit
  "Step 1:" prefixes or "Directions" labels inside `text`. Bold section-label lines export
  as `HowToSection{name, itemListElement}`.
- **`tags` must not duplicate the category (folder) or `cuisine`** — Google forbids
  recipeCategory/recipeCuisine terms inside `keywords`. Tags are ingredient/descriptor
  terms (chicken, quick, holiday…).
- If `nutrition` is emitted, `yield` (recipeYield) is required alongside it.
- `image` is required for Google rich results — irrelevant on the LAN, but any future
  public JSON-LD export should treat missing images as a validation warning.
