---
title: GitHub sync
summary: Keeping the recipes folder in a private GitHub repository, from connecting to conflicts and disconnecting.
audience: admin
order: 76
---

Optional. Once connected, the recipes folder is a git clone of a private GitHub repository, and the app commits, pulls and pushes by itself. Git is built in (libgit2, bundled through the `git2` crate), so nothing needs installing. The code is `crates/core/src/git.rs`; the UI is `ui/src/components/GitHub.svelte`.

## Connecting

In the first-start setup (**Recipes from GitHub**) or Settings → Recipes folder → GitHub:

1. **Repository**: `owner/name`. A `https://github.com/…` address, `github.com/owner/name` or `git@github.com:owner/name` also work; all become the https clone URL.
2. **Access token**: a fine-grained token made by the repository's owner, with *Only select repositories* → the recipes repository and *Permissions* → *Contents: Read and write*. The same token works on every family Mac.

Connect (`POST /api/git/connect`, desktop only) then:

- Uses the recipes folder as it is if it's already a clone of that repository.
- Sets an empty folder up to receive it (dot files such as `.DS_Store` don't count as content). A folder with other files is refused: choose an empty folder first with **Use a different folder…**.
- Refuses a folder that is already a clone of a different repository.
- Runs a first full sync, so a wrong repository name or a bad token fails here, before the token or the setting is saved.
- Then saves the token in the macOS Keychain (service `family.recipes.desktop`, account `github-token`) and sets `git_sync: true` in `settings.json`. Leaving the token field empty reuses a token already in the Keychain.

The token is sent to GitHub as the password for user `x-access-token`. It is never written to the repository's config or to the settings file.

## When it syncs

A background loop (`git::spawn`) runs one sync round:

- right after the app starts, which pulls what changed while it was closed;
- after every change in the recipes folder, as seen by the file watcher (saves in the app, and edits made in Finder, a text editor or Obsidian);
- every 5 minutes, which also retries uploads that failed while offline;
- on **Sync now** (`POST /api/git/sync`).

Only one round runs at a time. Each round:

1. **Commit.** If anything in the folder changed, everything is staged (`git add -A`) and committed. The message is `Update <path>` for one file, `Update N files` for more. The author is the repository's configured git user, or `Recipes <recipes@localhost>`.
2. **Fetch** the current branch from `origin`.
3. **Pull.** A fast-forward when possible, otherwise a merge commit, "Merge recipe changes from GitHub". The checkout is safe: if a file was edited again since step 1 it is left alone and the round fails, to be retried later.
4. **Push.** If GitHub refuses because it got a newer commit in between, the round fetches, merges and pushes once more. If that is refused too it gives up until the next round.

The app sees pulled changes at once, through the file watcher.

## What is synced and what isn't

- **Synced:** everything else in the recipes folder, including `attachments/` (photos, kept PDFs and Word files).
- **Never uploaded:** `.trash/` (deleted files and earlier versions) and `.DS_Store`. Connecting adds both to the repository's `.git/info/exclude`, whatever its `.gitignore` says.
- **Contents:** `README.md` at the top of the recipes folder (hidden in the app) lists every recipe by folder, and the app rewrites it after every change (`crates/core/src/toc.rs`), so the repository reads well on GitHub. It holds no dates, so two Macs write the same file; when both changed it, a merge keeps this Mac's copy and makes no "(from other Mac)" copy, and the next rescan writes it afresh. A `README.md` without the app's first line is someone's own and is never touched.
- **Feedback:** `FEEDBACK.md` at the top of the recipes folder (hidden in the app) collects what people send with [Sending feedback](#/settings/help/feedback), so it reaches GitHub with everything else.
- **Never synced:** the app's data folder. People and their devices (`users.json`), the activity log (`activity.log`), the whiteboard (`messages.json`), meals made (`made.json`), settings, the cookie key and the certificates live there, not in the recipes folder, so git never sees them. See [Architecture](#/settings/help/architecture).
- **Not in git at all:** the GitHub token and the AI key, which are in the Keychain.

## Status

Settings → Recipes folder → GitHub shows (from `git::status`, also part of `GET /api/settings`):

- the repository and the time of the last successful sync;
- the last error, if the round failed;
- how many commits are waiting to upload (made here, not yet on GitHub);
- after **Sync now**, how many files the pull changed.

Messages you may see:

| Message | Meaning |
|---|---|
| GitHub refused the token. Connect again with a new one. | the token expired, was revoked or lacks access |
| GitHub can't find that repository, or the token doesn't give access to it. | wrong name, or the token isn't for that repository |
| Couldn't reach GitHub (…). Changes are kept and uploaded once the Mac is online. | offline; commits wait and are pushed later |
| The recipes folder's git repository is in the middle of a merge or rebase. Finish it in Terminal first. | a git operation was started by hand and not finished |
| The recipes folder's git repository isn't on a branch. | detached HEAD; check out a branch by hand |
| GitHub kept changing during the upload; trying again in a few minutes. | the push was refused twice |

## Conflicts

Nothing is lost when two Macs change the same file before syncing:

- **Both changed the same file:** this Mac's version keeps the name, and the other version is saved beside it as `Name (from other Mac).md` (`Name (from other Mac 2).md` and so on if that is taken). Compare the two, keep what you want, and move the extra copy to the trash.
- **One deleted it, the other changed it:** the file is kept.

## Disconnecting

Settings → Recipes folder → GitHub → **Stop syncing** (`DELETE /api/git`) sets `git_sync: false` and deletes the token from the Keychain. The recipes and their git history (`.git/`) stay in the folder. Connecting again needs the token again.

## By hand

The folder is an ordinary git clone, so `git pull`, `git commit` and `git push` in Terminal work alongside the app. The commands are in the README.

## For developers

- A dev or test server with sync on commits and pushes whatever folder it points at. Never point one at `~/Recipes`, and copy recipes for tests without `.git` (the e2e `rsync` uses `--exclude .git`).
- The test in `git.rs` uses a local bare repository, no network. `connect` accepts an absolute path as the repository for this.
- `vendored-openssl` in `crates/core/Cargo.toml` is there only because libgit2-sys initialises OpenSSL on macOS too. After touching `git2`, check that `otool -L` shows no `/opt/homebrew` library.
