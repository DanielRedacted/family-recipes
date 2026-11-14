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

The 259 family recipes from `home-recipes/` ship with the app (`vault-seed/`) and are copied into
the recipes folder on first launch.

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
tools/              convert.py + verify.py (conversion from home-recipes/, word-for-word check)
docs/FORMAT.md      the recipe file format
```

## Tests

```sh
python3 -I tools/verify.py --self-test && python3 -I tools/verify.py   # 259/259 originals preserved
cargo test -p recipes-core                                             # parser, vault, auth, importer
cd ui && npx vitest run                                                # scaling / unit conversion
# end-to-end (needs a running recipes-server; see the scripts for arguments)
node e2e/flows.mjs <admin-url> <vault> <outdir>
node e2e/import.mjs <admin-url> <vault> <outdir>
```

## Network sharing, in short

The app makes its own certificate authority (limited to `recipes.local` and home-network
addresses), serves HTTPS on port 443 and a device setup page on port 80, and advertises
`recipes.local` through macOS Bonjour. Each device trusts the certificate once: Settings → *Phones,
tablets and other computers* shows a QR code that opens the setup page with per-device steps.
