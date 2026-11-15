# AGENTS.md

Notes for coding agents working in this repository. The README covers what the app does, how to
release it and how to run it; this file covers what you need to change it safely.

## What this is
dont 
A family cookbook app: a folder of Markdown recipes, browsed and edited in a Tauri desktop app
(`Recipes.app`) that also serves the same UI to phones and tablets on the home network.

- `crates/core/`: the whole server as a library. `vault.rs` (folder index, file writes, trash),
  `recipe.rs` (recipe structure parser), `server.rs` (axum HTTP API and the embedded UI), `auth.rs`,
  `ingredient.rs` (ingredient catalogue), `import.rs` / `extract.rs` / `schema.rs` (importer),
  `ai.rs`, `lan.rs` (TLS, Bonjour), `git.rs` (GitHub sync through bundled libgit2). `src/bin/recipes-server.rs` is the headless server for
  development and tests.
- `src-tauri/`: the desktop shell only (window, menu-bar item, the `Shell` trait for Finder,
  dialogs, print, opening URLs, updates).
- `ui/`: Svelte 5 (runes) + Vite. Compiled into `ui/dist` and embedded into the Rust binary with
  rust-embed (`#[folder = "../../ui/dist"]` in `server.rs`).
- `docs/FORMAT.md`: the recipe file format and how the app reads it.
- `DESIGN.md`, `VOICE.md`: the visual design and the timers/voice plan (see "Design documents").
- `docs/legacy/`: code and a backlog of ideas kept from the two earlier projects. Nothing there is
  built or tested; check its README before building something it lists.
- `scripts/`: `release.mjs` (build, sign, publish) and `normalize-units.ts` (one-off maintenance of a
  recipes folder).

## The recipes are not here

The app ships empty. The family recipes live in a separate private repository; on the owner's Mac
`~/Recipes` is a clone of it, with the original conversions on its `original-markdown` branch and the
conversion and checking tools in `~/Recipes/.tools/` (`verify.py`, `convert.py`, `techniques.txt`).

- Never copy recipes into this repository.
- `~/Recipes` is the owner's live data. Don't edit it unless asked; if asked, commit there locally
  and leave pushing to the owner.
- The sibling folder `home-recipes/` next to this repository is read-only.

## Rules for recipe text

- **No information added or removed** without the owner's explicit permission. Formatting changes
  (list markers, table layout, unit spellings, moving a value into a property) are fine; rewording,
  dropping words or inventing values are not.
- Prove it after any change to recipe files:
  ```sh
  python3 -I ~/Recipes/.tools/verify.py /tmp/originals/recipes ~/Recipes   # every original word kept
  python3 -I ~/Recipes/.tools/verify.py --since HEAD ~/Recipes             # nothing but spelling changed
  ```
  (`git -C ~/Recipes archive origin/original-markdown | tar -x -C /tmp/originals` gets the originals.)
  If a legitimate formatting change fails the check, extend `verify.py` with a narrow, explicit
  equivalence and a self-test that would catch a real change; don't loosen it generally.
- The app never re-serialises a note body. Frontmatter is edited only through `setProp`
  (`ui/src/components/Properties.svelte`), which uses yaml's `Document` API to keep comments, order
  and block style.
- Units are written one way (`tsp`, `Tbsp`, `cup`/`cups`, `oz`, `lb`, `350°F`, see `docs/FORMAT.md`).
  `ui/src/lib/units.ts` is the single normalizer for the reader, the importer and
  `scripts/normalize-units.ts`; keep it free of imports so Node can run it directly.

## Product principles

- Obsidian is the UX reference (folder column, `[[links]]`, properties); the editor follows Apple
  Notes (Markdown marks never shown, files stay plain Markdown, saved only on Save).
- AI is optional and off by default; it only ever proposes changes the user accepts.
- Before hand-rolling something sizeable, look for a maintained open-source package (the formatted
  editor's tables use `codemirror-markdown-tables`, for example).
- Phones are first-class: check layouts at phone width (≤899px switches to the drawer and bottom bar).

## Design documents

- **`DESIGN.md`** is the visual design in Google's DESIGN.md format: YAML tokens (colours, type,
  radii, spacing, components) followed by prose on how they're used. The CSS is the implementation
  (`ui/src/app.css` custom properties and the component styles); DESIGN.md describes it, it doesn't
  generate it.
  - Before styling UI, read it and reuse an existing token or component (`var(--accent)`, not a
    new hex value; an existing button or chip style, not a new one).
  - When you change a colour, font, radius, spacing value or component style in the CSS, change
    DESIGN.md in the same commit, light and dark (`-dark` tokens). A new component gets a
    `components:` entry and a line in the Components section.
  - Check it with `npx @google/design.md lint DESIGN.md` (ask the owner before running `npx`). It
    flags broken references and low contrast; the Do's and Don'ts section lists the known
    contrast issues.
- **`VOICE.md`** is the agreed plan for cooking timers, unit and temperature conversion, and a
  kitchen voice assistant. None of it is built.
  - Read it before building timers, conversions or anything voice related. Its decisions are
    settled (the app owns timers and conversions; no AI in the voice path; openWakeWord, faster-whisper,
    Piper); raise a change with the owner rather than working around one.
  - Build timers and conversions in the app first; they're useful without voice.
  - As parts get built, update VOICE.md to say what exists and where, and FORMAT.md or the README
    if what users see changes.

## Skills

Skills live in `.agents/skills/<name>/SKILL.md`. Read a skill's `SKILL.md` before doing the task it
covers, and follow it together with this file; where they disagree, this file and DESIGN.md win.

- **`unslop`**: visual polish passes on UI that looks generic or AI-generated. Here the direction
  comes from DESIGN.md, not a new one: use its polish mode, reuse existing tokens, and update
  DESIGN.md with any token or component change as described above.

## Things that bite

- **GitHub sync** (`git.rs`) commits and pushes everything in the recipes folder after each change once
  it's connected (`settings.git_sync`), and pulls at startup. Never point a dev or test server with sync
  on at `~/Recipes`; test copies must exclude `.git` (the e2e `rsync` already does). Its test uses a
  local bare repository, no network. `vendored-openssl` is there only because libgit2-sys initialises
  OpenSSL on macOS too; check `otool -L` shows no `/opt/homebrew` library after touching git2.
- **Build order.** The server embeds `ui/dist`, so build the UI before compiling a server you want to
  test: `cd ui && npm run build`, then `cargo build --bin recipes-server`.
- **API mutations** need the header `X-Recipes: 1` and a same-origin `Origin` (CSRF guard in
  `auth.rs`); the UI's `api()` adds it. Shell actions (`/api/shell/*`) are admin-only and need the
  desktop app.
- **Parser line numbers** (`recipe.rs`) are 0-based within the whole file, frontmatter included; the
  reader maps them onto the raw text (`scaledRaw` in `ui/src/lib/markdown.ts`). Ingredient detection
  is heuristic: when you change it, dump every recipe's parse before and after
  (`VAULT=~/Recipes DUMP=/tmp/x.json cargo test -p recipes-core dump_vault -- --ignored`) and review
  every line that moves between ingredients and steps.
- **Editor tables** rely on private annotation values of `codemirror-markdown-tables`
  (`"table.format"`, `"table.*"`) in `Editor.svelte` and `livePreview.ts`, so that re-padding a table
  is not counted as an edit or kept in undo history. Recheck both when upgrading the package.
- **Routing** is hash based (`#/n/<path>`, `#/f/<folder>`, `#/settings/<section>`). Navigate with
  `go()` from `state.svelte.ts`; it respects the unsaved-changes guard and the Back history counter.
  All link clicks go through one handler in `App.svelte` (app pages in the app, web pages in the
  browser).
- **Node:** `scripts/normalize-units.ts` relies on Node's built-in TypeScript support (Node 23.6+).

## Tests

```sh
cargo test -p recipes-core
VAULT=~/Recipes cargo test -p recipes-core -- --ignored --skip dump_vault   # the real recipes parse
cd ui && npx vitest run && npm run check
```

End-to-end (Playwright, `ui/e2e/*.mjs <admin-url> <vault> <outdir>`). The scripts edit, rename and
favorite recipes, so always run them on a copy:

```sh
cd ui && npm run build && cd .. && cargo build --bin recipes-server
rsync -a --exclude .git --exclude .trash ~/Recipes/ /tmp/e2e-vault/
target/debug/recipes-server --vault /tmp/e2e-vault --data /tmp/e2e-data --port 0   # prints the admin URL
# skip the first-start setup page (it covers the window): with the cookie from opening the admin URL,
# PUT /api/settings {"setup_done": true} with header X-Recipes: 1
cd ui && node e2e/flows.mjs "<admin-url>" /tmp/e2e-vault /tmp/e2e-out
node e2e/editor.mjs … && node e2e/ingredients.mjs …                         # fresh vault copy each
```

`e2e/import.mjs` needs the internet and two source files from the recipes repository
(`IMPORT_SRC`, default `/tmp/import-src`):

```sh
mkdir -p /tmp/import-src && git -C ~/Recipes archive origin/originals \
  "recipes/PDFs/Einkorn Pizza Dough Recipe _ GrillGirl.pdf" "recipes/Cookies - Macaroons.docx" \
  | tar -x -C /tmp/import-src --strip-components=1
```

A headless browser does not reproduce the desktop webview
(WKWebView); behaviour that depends on it (opening links, downloads, print) needs a manual check in
`Recipes.app`.

## Changes and commits

- Update `docs/FORMAT.md` when the file format or the way recipes are read changes, and the README's
  feature summary when a user-visible feature changes.
- Commit only when asked. Commit messages are one plain sentence or two describing the behaviour
  change, not the files touched.
- `node scripts/release.mjs` pushes and publishes a GitHub release: run it only when asked. Never
  commit the signing key (`~/.tauri/family-recipes.key*`).
