---
title: Architecture
summary: How the app is built, from the Rust core and the desktop shell to the Svelte UI, the two folders and the build order.
audience: admin
order: 80
---

One Rust server library does everything; the desktop app and the headless dev server are two thin ways of running it. The UI is a Svelte app compiled into the server binary.

```
  ui/ (Svelte 5 + Vite) ──npm run build──▶ ui/dist ──rust-embed──┐
                                                                  ▼
  crates/core (recipes-core: server, vault, parser, LAN, sync, importer, AI)
        │                                          │
        ▼                                          ▼
  src-tauri (Recipes.app)                 recipes-server (headless)
  window on 127.0.0.1:<random>            127.0.0.1:8640 by default
  + home network on :80 / :443            + home network only with --lan
```

## crates/core

The whole server, as the library `recipes-core`. Modules (`crates/core/src/`):

| Module | What it does |
|---|---|
| `lib.rs` | Glue: subsystem state (`Extensions`), the extra routes, `start_network` (router, sharing, IP watcher, GitHub sync) and reacting to settings changes. |
| `server.rs` | The axum router and handlers for files, folders, trash, settings and shell actions; app state, settings, the file watcher, and serving the embedded UI. |
| `auth.rs` | Roles (Reader, Writer, Admin), session cookies, PIN hashing (Argon2) and rate limiting, and the `guard` middleware (host check, CSRF check, activity log). |
| `users.rs` | People and their devices (`users.json`), sign-in, and the activity log (`activity.log`). |
| `vault.rs` | The recipes folder as a database: safe relative paths, scanning and the index, atomic writes, the trash and earlier versions. |
| `recipe.rs` | Reads a recipe's structure (title, ingredients, steps, sections) from a Markdown note; never rewrites the file. |
| `ingredient.rs` | The ingredient catalogue: every ingredient line split into quantity, unit and name, grouped across recipes. |
| `import.rs` | Import jobs for web pages, PDFs, Word files, photos and pasted text; drafts are saved only when the user saves. |
| `extract.rs` | Turns a web page or document into a draft recipe without AI (schema.org, WP Recipe Maker, microdata, readability). |
| `schema.rs` | A tolerant reader for schema.org `Recipe` data and WP Recipe Maker's REST JSON. |
| `fetch.rs` | Outbound HTTP for the importer, refusing private and loopback addresses on every hop (SSRF guard), with size caps. |
| `ai.rs` | The optional AI assistant: Anthropic or OpenAI-compatible providers, key in the Keychain. See [AI assistant](#/settings/help/ai-assistant). |
| `lan.rs` | Home-network sharing: HTTP and HTTPS listeners, the local CA and certificates, Bonjour, the `/setup` page. See [Home network](#/settings/help/home-network). |
| `git.rs` | GitHub sync through bundled libgit2. See [GitHub sync](#/settings/help/github-sync). |
| `board.rs` | The kitchen whiteboard (`messages.json`). |
| `made.rs` | Meals made, behind "Made today", guessed meals and the dinner suggestion (`made.json`). |
| `calendar.rs` | The Mac's calendar events, read-only through EventKit, for the Calendar page and the home page's Today list. |
| `setup.html` | The device setup page served at `/setup`. |
| `bin/recipes-server.rs` | The headless server (below). |

The routes are listed on [Server API](#/settings/help/server-api).

## src-tauri

The desktop shell only (`src-tauri/src/main.rs`), Tauri 2:

- Starts the core in-process and binds the loopback listener on a random port; the window opens `/auth?k=<launch token>` there, which makes it the admin.
- Implements the core's `Shell` trait (`TauriShell`): reveal in Finder, the folder picker, print, opening web pages in the default browser, open at login, restart, and checking for and installing updates.
- Keeps running in the menu bar when the window is closed, so phones and tablets stay served. Started at login with `--hidden`, it has no window and no Dock icon.
- Links to other sites open in the default browser, not in the window. Downloads go to the Downloads folder, numbered when a name is taken.
- Plugins: single instance, dialog, opener, autostart, updater (releases from GitHub, signed; see [Development](#/settings/help/development)).

## ui

Svelte 5 with runes, built by Vite (`ui/`). Notable parts:

- Hash routing (`#/n/<path>`, `#/f/<folder>`, `#/settings/<section>`), with `go()` in `ui/src/lib/state.svelte.ts`.
- `ui/src/lib/api.ts`: the fetch wrapper that adds `X-Recipes: 1` to every call.
- The formatted editor on CodeMirror 6 (with `codemirror-markdown-tables`); Markdown rendering with markdown-it and DOMPurify; search with MiniSearch; PDFs with pdf.js; Word files with mammoth.
- A PWA (`vite-plugin-pwa`) for phones and tablets: the UI, the recipe list and opened files are cached for reading offline. It's not registered inside the desktop window.
- The Help pages: `docs/help/*.md` and `docs/FORMAT.md`, bundled by `ui/src/lib/help.ts`.

**Embedding.** `npm run build` writes `ui/dist`. `server.rs` embeds that folder into the binary with rust-embed (`#[folder = "../../ui/dist"]`), and the router's fallback serves it: files under `assets/` are cached for a year (their names carry a hash), HTML gets a Content-Security-Policy, and unknown paths get `index.html`. Without a built UI the server answers "UI not built (run npm run build in ui/)". In development, `npm run dev` runs Vite with the API proxied to a headless server on port 8640.

## Two folders

The recipes and the app's own data are kept apart, so GitHub sync never sees people, logs or keys.

**The recipes folder** (default `~/Recipes`, changeable in Settings → Recipes folder; the choice is stored in `settings.json` and takes effect when Recipes restarts):

| Path | What it is |
|---|---|
| `**/*.md` | the recipes and other notes; folder = category |
| `attachments/` | images, and imported originals under `attachments/originals/` |
| `.trash/` | deleted files, and earlier versions of overwritten files under `.trash/versions/` (at most one per 10 minutes) |
| `.folders.yaml` | folder icons and colours, written by the app |
| `.git/` | when GitHub sync is on |

The app hides every dot file and dot folder, and `AGENTS.md`, `CLAUDE.md`, `FEEDBACK.md` and `README.md` at the top level. `README.md` is the contents, written by the app after each rescan (`toc.rs`) only when it changes. A file watcher (300 ms debounce) rescans when anything visible changes, so edits made in Finder, a text editor or Obsidian show up at once. The file format is in [Recipe file format](#/settings/help/file-format).

**The data folder** (the desktop app: `~/Library/Application Support/family.recipes.desktop`; `recipes-server`: `--data`, default `~/Library/Application Support/Recipes`). Files are written readable by the owner only:

| File | What it is |
|---|---|
| `settings.json` | settings: recipes folder, PIN hash, AI provider and model, sharing, keep awake, GitHub sync on/off, Editing as |
| `cookie.key` | the key that signs session cookies |
| `users.json` | people and their devices |
| `activity.log` | every change a person made, one JSON line each |
| `messages.json` | the whiteboard |
| `made.json` | meals made, with who logged them and which are guesses |
| `tls/` | the local CA and the server certificate |
| `dns-sd.pid` | the Bonjour registration's process |

Secrets that are in neither folder: the GitHub token and the AI key, in the macOS Keychain (service `family.recipes.desktop`).

## recipes-server

`crates/core/src/bin/recipes-server.rs`, for development and tests:

```sh
recipes-server --vault DIR [--data DIR] [--port N] [--lan]
```

- Listens on `127.0.0.1` (port 8640 by default; `--port 0` picks a free one) and prints the admin link, `http://127.0.0.1:<port>/auth?k=…`.
- No desktop shell: the `/api/shell/*` actions answer `501`.
- Home-network sharing is off unless `--lan` is given.
- GitHub sync still runs if the data folder's settings have it on.
- Without `--vault` it uses `~/Recipes`, and without `--data` the default data folder above. For anything but reading, always pass test copies of both.

## Build order

The server embeds `ui/dist`, so the UI is built first:

```sh
cd ui && npm install && npm run build && cd ..   # ui/dist
cargo build --bin recipes-server                 # the headless server, with the UI inside
```

or, for the whole desktop app:

```sh
cd ui && npm run app    # Tauri builds the UI itself first
```

`npm run app` runs `tauri build`, whose `beforeBuildCommand` is `npm run build`; it produces `target/release/bundle/macos/Recipes.app`, ad-hoc signed. Commands for tests and releases are on [Development](#/settings/help/development).
