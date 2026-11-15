# Recipes

Preserving my mom and grandma's recipes.

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
ingredient catalogue with pictures that also feeds suggestions while typing, a Back button (⌘[),
links to web pages marked ↗ and opened in the browser, and an importer for web pages (schema.org, WP Recipe Maker incl. its
REST data, nutrition and equipment), PDFs, Word files, photos and pasted text — several at once,
each with a step log.

## The recipes are not in this repository

The app ships empty and works on whatever folder of recipes you give it. The family recipes live in
a private repository of their own (`home-recipes`), and on the Mac `~/Recipes` is a clone of it:

```sh
git clone https://github.com/DanielRedacted/home-recipes.git ~/Recipes   # first time, on a new Mac
cd ~/Recipes && git pull                                                  # get changes
cd ~/Recipes && git add -A && git commit -m "…" && git push               # share your edits
```

The app sees pulled changes at once. `.trash/` stays out of git; `.git/` and `.tools/` (the
conversion scripts) are hidden from the app. The original conversions from the family's Word
documents are kept on that repository's `original-markdown` branch.

## Download and updates

Download `Recipes_X.Y.Z_universal.dmg` from
[Releases](https://github.com/DanielRedacted/family-recipes/releases/latest) and drag Recipes into
**Applications**. Run it from there: updates can't be installed into a copy that runs from the disk
image or the Downloads folder.

The app isn't notarized by Apple, so the first launch needs one extra step: right-click Recipes in
Applications → **Open** → **Open**. (Or, in Terminal: `xattr -cr /Applications/Recipes.app`.)

When a new version is out, the app says so a few seconds after it opens: **Update and restart**
downloads it, asks about any unsaved edit, and restarts into the new version. Settings → App →
**Check for updates** looks on demand. Each update is a new ad-hoc signed build, so macOS may ask
once more for Keychain access (the AI key) and for incoming network connections: choose **Always
Allow** / **Allow**.

## Release

```sh
node scripts/release.mjs 0.2.0 "What changed, one line per item"
```

It refuses a dirty tree, an existing tag or a version that isn't newer, then builds a universal
`Recipes.app` on this Mac, signs the update, commits the version, tags `v0.2.0` (the message becomes
the release notes, shown in the app under *What's new*), pushes, and publishes the GitHub release
with the disk image, the update bundle and `latest.json`. Nothing is committed or published if the
build fails. Needs `gh` signed in and the Rust target `x86_64-apple-darwin`
(`rustup target add x86_64-apple-darwin`).

**Back up the signing key.** Updates are signed with `~/.tauri/family-recipes.key` and its password
in `~/.tauri/family-recipes.key.password`; the public half is in `src-tauri/tauri.conf.json`. Keep
copies of both files somewhere safe (a password manager). If they're lost, installed apps will
refuse every future update and need a manual reinstall from a build with a new key. Never commit them.

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
scripts/release.mjs build, sign and publish a release (see Release)
deploy/linux/       Linux / Raspberry Pi notes and the old projects' deploy files (not used on macOS)
docs/FORMAT.md      the recipe file format
```

## Tests

```sh
cargo test -p recipes-core                                   # parser, vault, auth, importer
VAULT=~/Recipes cargo test -p recipes-core -- --ignored       # every note in a real recipes folder parses
cd ui && npx vitest run                                      # scaling / unit conversion
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
