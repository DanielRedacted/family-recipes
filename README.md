# Recipes

The family cookbook as a folder of plain-text recipes, with an app to browse, search, read, cook
from, edit, import and manage them.

- **On the Mac:** `Recipes.app` (Tauri). Closing the window keeps it running in the menu bar.
- **On phones, tablets and other computers:** `https://recipes.local`, installable as a full-screen
  app (PWA) that keeps working for reading when the Mac is asleep. Read-only until the PIN set on the
  Mac is entered.
- **The files:** `~/Recipes` (changeable in Settings). Markdown, one file per recipe, folder = category.
  See [docs/FORMAT.md](docs/FORMAT.md).
- **AI:** optional, off by default, your own Anthropic or OpenAI-compatible API key (stored in the
  macOS Keychain). Only ever proposes changes you accept.

What it does: a folder column that follows the open recipe and cards (with emoji and colours you
can change), favorites, ⌘K search (recipes, folders, ingredients), a reader with serving scaler (every set of
ingredients scales; yields such as "2 dozen" count as 24), unit conversion and cook mode, ingredient
tables with alternative portions (Single / Double…: one portion at a time, with its multiplier), a
formatted editor in the style of Apple Notes (Markdown marks are never shown; text types, lists,
checklists and real tables with an editor in each cell; the file stays plain
Markdown and is saved only when you press Save), `[[links]]` between recipes with backlinks, an
ingredient catalogue with pictures that also feeds suggestions while typing, and an importer for web pages (schema.org, WP Recipe Maker incl. its
REST data, nutrition and equipment), PDFs, Word files, photos and pasted text — several at once,
each with a step log.

The 259 family recipes from `home-recipes/` ship with the app (`vault-seed/`, 257 files: two were
exact duplicates) and are copied into the recipes folder on first launch. When a later version
corrects or renames a bundled recipe, the app updates it in your folder only if you have not
changed that file (the old copy goes to `.trash/versions`); edited, moved or deleted files are left
alone. File names and titles were tidied (typos, ALL CAPS, "Recipe" suffixes) by
`tools/names.tsv`; old names stay searchable as `aliases`. The text is in standard Markdown without
a word added or dropped: ingredient tables in one layout (amounts first, name last; see
`docs/FORMAT.md`), yields and times in properties, flattened Word tables rebuilt.

## Build and run

Requirements: Rust (stable), Node 20+, macOS 12+.

```sh
cd ui && npm install
npm run app            # builds target/release/bundle/macos/Recipes.app (ad-hoc signed)
```

Development without the desktop shell:

```sh
cargo run -p recipes-core --bin recipes-server -- --vault /tmp/vault --data /tmp/data [--lan]
# prints an http://127.0.0.1:8640/auth?k=… link that opens the app with full rights
cd ui && npm run dev   # optional: Vite dev server, proxies /api to :8640
```

## Layout

```
crates/core/        server library: vault, recipe parser, HTTP API, LAN/TLS, importer, AI
  src/bin/recipes-server.rs   headless server
src-tauri/          desktop shell (window, menu-bar item, Finder/dialog/print hooks)
ui/                 Svelte 5 + Vite frontend (embedded into the binary), PWA
vault-seed/         the converted family recipes
tools/              convert.py + verify.py (conversion from home-recipes/, word-for-word check),
                    overrides.json (structure-only fixes), names.tsv (renames and titles),
                    ingredient_tables.txt (tables rewritten in the standard layout),
                    techniques.txt, seed-etags.py
deploy/linux/       Linux / Raspberry Pi notes and the old projects' deploy files (not used on macOS)
docs/FORMAT.md      the recipe file format
```

## Tests

```sh
python3 -I tools/verify.py --self-test && python3 -I tools/verify.py   # 259/259 originals preserved (or proven duplicates)
cargo test -p recipes-core                                             # parser, vault, auth, importer
cd ui && npx vitest run                                                # scaling / unit conversion
# end-to-end (needs a running recipes-server; see the scripts for arguments)
node e2e/flows.mjs <admin-url> <vault> <outdir>        # navigation, save, menus, icons, links
node e2e/editor.mjs <admin-url> <vault> <outdir>       # formatted editor
node e2e/ingredients.mjs <admin-url> <vault> <outdir>  # catalogue and suggestions
node e2e/import.mjs <admin-url> <vault> <outdir>       # importer (needs internet)
```

## Network sharing, in short

The app makes its own certificate authority (limited to `recipes.local` and home-network
addresses), serves HTTPS on port 443 and a device setup page on port 80, and advertises
`recipes.local` through macOS Bonjour. Each device trusts the certificate once: Settings → *Phones,
tablets and other computers* shows a QR code that opens the setup page with per-device steps.
No Docker or Caddy is involved; see `deploy/linux/README.md` for what a Raspberry Pi version
would need.
