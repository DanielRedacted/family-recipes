# Recipes

Preserving my mom and grandma's recipes.

The family cookbook as a folder of plain-text recipes, with an app to browse, search, read, cook
from, edit, import and manage them.

- **On the Mac:** `Recipes.app` (Tauri). Closing the window keeps it running in the menu bar. With
  *Open Recipes at login* on, it starts in the menu bar without a window.
- **On phones, tablets and other computers:** `http://recipes.local` in any browser, with nothing to
  set up. Each device signs in once with a name (no password) and the family PIN set on the Mac; without
  a PIN they can read but not edit. The Mac's Settings → People lists everyone, their devices and
  what each of them changed. Each person also gives a full name, which the recipes record: who
  imported a recipe, who wrote it, and a dated history of changes with a comment for each
  ([docs/FORMAT.md](docs/FORMAT.md)). On the Mac, "Editing as" chooses whose name its changes carry. After a one-time certificate step it is
  also installable from `https://recipes.local` as a full-screen app (PWA) that keeps working for
  reading when the Mac is asleep.
- **The files:** `~/Recipes` (changeable in Settings). Markdown, one file per recipe, folder = category.
  See [docs/FORMAT.md](docs/FORMAT.md).
- **Sync:** optional, with a private GitHub repository: every save is uploaded, other Macs' changes
  are downloaded (see below). The app keeps `README.md` in the recipes folder as a contents list, so
  the repository reads well on GitHub.
- **Recipe book:** Settings → General saves every recipe as one self-contained web page, or as a PDF
  (straight to Downloads on the Mac, the print dialog elsewhere): title page, contents, a chapter per folder.
- **AI:** optional, off by default, your own Anthropic or OpenAI-compatible API key (stored in the
  macOS Keychain), or a model running on the Mac (Ollama, LM Studio at `http://localhost:…/v1`, no
  key). Only ever proposes changes you accept.

What it does: a folder column that follows the open recipe and cards (with emoji and colours you
can change), bookmarks for everyone and each person's own favorites (kept with their name, on every device), a
People page and a page for everyone the recipes credit (account or not: what they wrote, added and edited,
their favorites and an optional photo or emoji), home page sections that fold away (remembered per device), a
kitchen whiteboard on the home page (notes that last today, until a chosen date or until removed),
a suggested dinner on the home page with Random beside it (filters: kind of recipe, default Dinner;
protein; folder; bookmarked; Random shows every match as a shuffled grid), "Made today" under each
recipe and in cook mode (any device can log a meal; after a dinner with a protein, dinners with that
protein aren't suggested for 3 days unless it's chosen in the Protein filter; a recipe left open 20 minutes
without it is logged as a guess), a Calendar page (week or month) of what was cooked, added and changed each
day, with the Mac's Calendar.app events when turned on in Settings (read-only; today's events also show on the
home page when there are any), each recipe's author under
its title and the website it came from, ⌘K search (recipes, folders, ingredients), a reader with serving scaler (every set of
ingredients scales; yields such as "2 dozen" count as 24), US or metric units (chosen in Settings; units are written one
way, in lower case, with amounts in bold: `docs/FORMAT.md`) and cook mode, ingredient
tables with alternative portions (Single / Double…: one portion at a time, with its multiplier), a
formatted editor in the style of Apple Notes (Markdown marks are never shown; text types, lists,
checklists and real tables with an editor in each cell; the file stays plain
Markdown and is saved only when you press Save), `[[links]]` and Markdown links between recipes with backlinks (an ingredient named like another
recipe links to it), an
ingredient catalogue with pictures that also feeds suggestions while typing, a Nutrition Facts label and package-style ingredient list for each recipe
worked out from its ingredients with USDA FoodData Central values (recipes used as ingredients are worked out from their own
ingredients; it follows the scaler; an optional USDA key looks up ingredients the shipped table lacks), tabs on wider screens (each
with its own page and Back button, ⌘[),
links to web pages marked ↗ and opened in the browser, and an importer for web pages (schema.org, WP Recipe Maker incl. its
REST data, nutrition and equipment), PDFs, Word files, photos and pasted text — several at once,
each with a step log, in-app feedback (search "feedback" or ⇧⌘F; appended to `FEEDBACK.md` in the recipes folder, so it syncs to GitHub), and a Help section in Settings (a searchable user guide for everyone, plus technical
pages for admins; the pages are `docs/help/*.md`).

## The recipes are not in this repository

The app ships empty and works on whatever folder of recipes you give it. The family recipes live in
a private repository of their own (`home-recipes`), and on the Mac `~/Recipes` is a clone of it.

**GitHub sync (in the app):** in the first-start setup or Settings → Recipes folder → GitHub, enter
the repository (`owner/name`) and a fine-grained token (only that repository, *Contents: Read and
write*; kept in the Keychain). An empty recipes folder is filled from GitHub; a folder that is already
a clone of it is reused. After that every save is committed and pushed (kept and retried while
offline), other Macs' changes are pulled when the app opens, every 5 minutes and on **Sync now**.
When two Macs change the same file before syncing, nothing is lost: the other version is saved
beside it as `Name (from other Mac).md`. Git is built into the app (libgit2); nothing to install.

Without the app, the same by hand:

```sh
git clone https://github.com/DanielRedacted/home-recipes.git ~/Recipes   # first time, on a new Mac
cd ~/Recipes && git pull                                                  # get changes
cd ~/Recipes && git add -A && git commit -m "…" && git push               # share your edits
```

The app sees pulled changes at once. `.trash/` stays out of git. `main` holds only recipes and
settings. The files recipes were made from (the family's Word documents, PDFs and anything imported
since) are in `attachments/originals/`, each linked from its recipe; the conversion and checking
tools are on that repository's `tools` branch.

## Download and updates

Download `Recipes_X.Y.Z_universal.dmg` from
[Releases](https://github.com/DanielRedacted/family-recipes/releases/latest) and drag Recipes into
**Applications**. Run it from there: updates can't be installed into a copy that runs from the disk
image or the Downloads folder.

The app isn't notarized by Apple, so the first launch needs one extra step: right-click Recipes in
Applications → **Open** → **Open**. (Or, in Terminal: `xattr -cr /Applications/Recipes.app`.)
The first start opens a short setup: recipes folder, open at login, sharing with phones and
tablets, and an optional family PIN. All of it can be changed later in Settings.

When a new version is out, the app says so a few seconds after it opens: **Update and restart**
downloads it, asks about any unsaved edit, and restarts into the new version. Settings → App →
**Check for updates** looks on demand. Each update is a new ad-hoc signed build, so macOS may ask
once more for Keychain access (the AI key) and for incoming network connections: choose **Always
Allow** / **Allow**.

## Release

```sh
node scripts/release.mjs 0.2.0 "What changed, one line per item"
```

It refuses a dirty tree, an existing tag or a version that isn't newer, and stops if any test fails:
the Rust tests, the parse of every recipe in `~/Recipes`, the UI tests and type check, and a smoke
check that loads the app on a copy of the recipes at desktop and phone width (`ui/e2e/smoke.mjs`).
Then it builds a universal `Recipes.app` on this Mac and signs the update. It starts the built app
on a copy of the recipes (its window appears for a few seconds) and stops if the window doesn't show
the folders; that copy never shares on the network or touches the installed app's settings or login
item. Then it commits the version, tags `v0.2.0` (the message becomes
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
docs/help/          the in-app Help pages (Settings → Help)
```

## Tests

```sh
cargo test -p recipes-core                                   # parser, vault, auth, importer
VAULT=~/Recipes TECHNIQUES="$PWD/../home-recipes-tools/.tools/techniques.txt" cargo test -p recipes-core -- --ignored   # every note in a real recipes folder parses
cd ui && npx vitest run                                      # scaling / unit conversion
# end-to-end (needs a running recipes-server; see the scripts for arguments)
node e2e/flows.mjs <admin-url> <vault> <outdir>        # navigation, save, menus, icons, links
node e2e/editor.mjs <admin-url> <vault> <outdir>       # formatted editor
node e2e/ingredients.mjs <admin-url> <vault> <outdir>  # catalogue and suggestions
node e2e/import.mjs <admin-url> <vault> <outdir>       # importer (needs internet)
```

## Network sharing, in short

The app serves itself over plain HTTP on port 80, so any device on the home network opens
`http://recipes.local` with no setup (the PIN, names and the session cookie then cross the Wi-Fi
unencrypted, as with a home file share). It also makes its own certificate authority (limited to
`recipes.local` and home-network addresses), serves HTTPS on port 443, and advertises
`recipes.local` through macOS Bonjour. The HTTPS address is only needed for the installable offline
app. A device trusts the certificate once through the setup page at `http://recipes.local/setup`
(Settings → This device → *Install as an app*), and after that `http://` moves it to `https://` by
itself. Settings → *Phones & tablets* on the Mac shows a QR code that opens the app. Only one Mac
shares: a Mac that starts while another already serves `recipes.local` leaves the network to it
(its own window still shows its own recipes folder) and takes over within a minute if that Mac stops.
No Docker or Caddy is involved; see `deploy/linux/README.md` for what a Raspberry Pi version
would need.
