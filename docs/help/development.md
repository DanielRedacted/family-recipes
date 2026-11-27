---
title: Development
summary: Building and running the app, the tests, releasing, the code graph, and how these Help pages are kept current.
audience: admin
order: 90
---

For coding agents, `AGENTS.md` at the top of the repository is the full set of rules; this page collects the commands. How the pieces fit is on [Architecture](#/settings/help/architecture).

## Requirements

Rust (stable), Node 20+, macOS 12+. `scripts/normalize-units.ts` needs Node 23.6+ (built-in TypeScript).

## Build

```sh
cd ui && npm install
npm run app            # builds target/release/bundle/macos/Recipes.app (ad-hoc signed)
```

The server embeds `ui/dist`, so build the UI before compiling a server you want to test:

```sh
cd ui && npm run build
cargo build --bin recipes-server    # from the repository root
```

## Run without the desktop shell

```sh
cargo run -p recipes-core --bin recipes-server -- --vault /tmp/vault --data /tmp/data [--lan]
cd ui && npm run dev   # optional: Vite dev server, proxies /api to :8640
```

The server prints an `http://127.0.0.1:8640/auth?k=…` link that opens the app with full rights.

- Always pass `--vault` and `--data`: without them the server uses `~/Recipes` and the default data folder.
- `--port N` changes the port (`--port 0` picks a free one). `--lan` also shares on the home network; see [Home network](#/settings/help/home-network).
- GitHub sync runs in a dev server too if the data folder's settings have it on. Never point a server with sync on at `~/Recipes`; copy recipes for testing without `.git`. See [GitHub sync](#/settings/help/github-sync).
- `npm run dev` proxies `/api` and `/auth` to port 8640, so start the server on its default port.

## Unit tests

```sh
cargo test -p recipes-core
VAULT=~/Recipes cargo test -p recipes-core -- --ignored --skip dump_vault   # the real recipes parse
cd ui && npx vitest run && npm run check
```

When you change ingredient detection in `recipe.rs`, dump every recipe's parse before and after and review every line that moves between ingredients and steps:

```sh
VAULT=~/Recipes DUMP=/tmp/x.json cargo test -p recipes-core dump_vault -- --ignored
```

## End-to-end tests

Playwright scripts, `ui/e2e/*.mjs <admin-url> <vault> <outdir>`. They edit, rename and favorite recipes, so always run them on a copy:

```sh
cd ui && npm run build && cd .. && cargo build --bin recipes-server
rsync -a --exclude .git --exclude .trash ~/Recipes/ /tmp/e2e-vault/
target/debug/recipes-server --vault /tmp/e2e-vault --data /tmp/e2e-data --port 0   # prints the admin URL
cd ui && node e2e/flows.mjs "<admin-url>" /tmp/e2e-vault /tmp/e2e-out
node e2e/editor.mjs … && node e2e/ingredients.mjs … && node e2e/accounts.mjs …  # fresh vault copy each
DATA=/tmp/e2e-data node e2e/cookbooks.mjs "<admin-url>" /tmp/e2e-vault /tmp/e2e-out /tmp/e2e-mine  # a second cookbook (a folder of another name, outside the first)
```

Once the server runs and before the scripts, skip the first-start setup page (it covers the window): with the cookie from opening the admin URL, send `PUT /api/settings {"setup_done": true}` with the header `X-Recipes: 1`.

| Script | Covers |
|---|---|
| `flows.mjs` | navigation, save, menus, icons, links |
| `editor.mjs` | the formatted editor |
| `ingredients.mjs` | the ingredient catalogue and editor suggestions |
| `accounts.mjs` | sign-in, the write log, the whiteboard and bookmarks, with a second browser as a family member |
| `import.mjs` | import flows (needs the internet) |

`e2e/import.mjs` needs the internet and two source files from the recipes repository (`IMPORT_SRC`, default `/tmp/import-src`):

```sh
mkdir -p /tmp/import-src && cp ~/Recipes/attachments/originals/{"Einkorn Pizza Dough Recipe _ GrillGirl.pdf","Cookies - Macaroons.docx"} /tmp/import-src/
```

A headless browser does not reproduce the desktop webview (WKWebView). Behaviour that depends on it (opening links, downloads, print) needs a manual check in `Recipes.app`.

## Releasing

Only when the owner asks: it pushes and publishes.

```sh
node scripts/release.mjs 0.2.0 "What changed, one line per item"
```

`scripts/release.mjs` refuses a dirty tree, an existing tag or a version that isn't newer. It stops before changing anything if a test fails: `cargo test`, the parse of every recipe in `~/Recipes`, `vitest`, `npm run check`, and `ui/e2e/smoke.mjs`, which starts `recipes-server` on a copy of the recipes and loads the app at desktop and phone width with no script errors. It then sets the version in `Cargo.toml` and `Cargo.lock`, builds a universal `Recipes.app` on this Mac and signs the update. Next it runs the built app with `RECIPES_SMOKE=<temp dir>` (see `src-tauri/src/main.rs`): on a copy of the recipes, with sharing off, no single-instance lock and no login item, it makes the sharing certificate and exits 0 once its window shows the folders, or 1 after 45 s; a failure puts the version back and publishes nothing. Then it commits the version, tags `vX.Y.Z` (the message becomes the release notes, shown in the app under *What's new*), pushes, and publishes the GitHub release with the disk image, the update bundle and `latest.json`. Nothing is committed or published if the build fails.

- Needs `gh` signed in and the Rust target `x86_64-apple-darwin` (`rustup target add x86_64-apple-darwin`).
- Updates are signed with `~/.tauri/family-recipes.key` and its password in `~/.tauri/family-recipes.key.password`; the public half is in `src-tauri/tauri.conf.json`. Keep backups of both files. If they're lost, installed apps refuse every future update. Never commit them.
- Installed apps look for `latest.json` on the latest GitHub release.

## The code graph

`graphify-out/` holds a knowledge graph of the code and docs. Ask it before reading source:

```sh
graphify query "how does the guard check CSRF"
graphify explain "git.rs"
graphify path "guard()" "identify()"
```

- Git hooks (`post-commit`, `post-checkout`, `post-merge`, `post-rewrite`) rebuild the code part.
- After editing code, run `graphify update .` (AST only, free).
- After a new Help page or a rewritten section, run `/graphify . --update` in Claude Code so the graph reads the new text. Skip it for small wording fixes.
- A large diff to `graphify-out/graph.json` is expected: it is rewritten whole on every build.

## Writing and keeping these Help pages current

The pages are `docs/help/*.md`, one per topic, plus `docs/FORMAT.md` (shown as [Recipe file format](#/settings/help/file-format)). `ui/src/lib/help.ts` bundles them into the UI at build time and indexes them for the search box; they are shown in Settings → Help and rendered with the same Markdown renderer as recipes.

Each page starts with frontmatter and has no `# Title` line (the title is the heading):

```yaml
---
title: Server API
summary: One plain sentence for the contents list.
audience: admin     # or everyone
order: 85           # position in the contents list
---
```

- **Pages for everyone** are written for the family: plain words, what to tap and what happens, no file or code names.
- **Admin pages** may name files, endpoints and commands. They are hidden from everyone but the desktop app, in the list and in search, but their text ships in the UI bundle like the rest: never put a secret in one.
- Link pages with `[text](#/settings/help/<file name without .md>)`. A page for everyone must never link to an admin page.

The rule (`AGENTS.md`, "Keeping docs and the graph current"): any change to behaviour, settings, the UI, the API, the file format or setup updates the matching page in the same change. A new feature gets a new page or section, and a removed feature has its text removed. Find every page that mentions the area with `graphify query`, `graphify explain` and `grep docs/help`. Also update `docs/FORMAT.md` when the file format changes, the README's feature summary when a user-visible feature changes, and `DESIGN.md` / `VOICE.md` as `AGENTS.md` says. End every change by saying which docs were updated, or "no docs affected" and why.

Two checks help:

- **The test.** `cd ui && npx vitest run src/lib/help.test.ts` (`ui/src/lib/help.test.ts`) checks that every page has a title, summary, valid audience and an order, that no body starts with a `# ` heading, that every `#/settings/help/…` link points at a real page, that pages for everyone don't link to admin pages, and that admin pages are hidden from others in the list and in search.
- **The Stop hook.** In Claude Code, `.claude/settings.json` runs `scripts/docs-check.sh` whenever the agent stops. If code under `ui/src`, `crates`, `src-tauri/src` or `docs/FORMAT.md` changed since the graph was built, it runs `graphify update .`. If that code also changed after the newest Help page (or the last "no docs affected" answer), it blocks the stop once and asks for the matching `docs/help/` page to be updated or for a reason why none is affected; the next stop goes through and is remembered in `.claude/docs-checked`. Other agents follow the rule by hand.
