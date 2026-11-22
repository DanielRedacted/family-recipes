---
title: Server API
summary: Every HTTP route the server answers, who may call it, and how authentication, the CSRF guard and the activity log work.
audience: admin
order: 85
---

The desktop window, phones and tablets all use the same HTTP API, served by one axum router (`server::router` in `crates/core/src/server.rs`, plus `extra_routes` in `crates/core/src/lib.rs`). The UI calls it through `api()` in `ui/src/lib/api.ts`.

## Roles

Every request gets a role from `auth::identify`. Roles are ordered, so a higher one may do everything a lower one may.

| Role | Who |
|---|---|
| Admin | the desktop window: the loopback listener plus the per-launch `recipes_admin` cookie |
| Writer | a device signed in as a person **while a family PIN is set** |
| Reader | everyone else who reaches a listener: signed-in devices while no PIN is set, and devices not signed in |

Separately from the role, the request carries **who** is asking (`Who`): the desktop window is the built-in "Admin" (or the person chosen as Editing as), a signed-in device is its person, anyone else is nobody. The whiteboard and the activity log use this.

Reading needs no sign-in at the API level; the UI shows its sign-in screen first. A role check that fails answers `403` with "only the desktop app can do this" or "editing needs the family PIN: set one in the Recipes app on the Mac".

## Sessions

| Cookie | Set by | Listener |
|---|---|---|
| `recipes_admin` | `GET /auth?k=<launch token>` | loopback only; the token changes at every launch |
| `__Host-recipes` | `POST /api/session` | HTTPS (Secure, HttpOnly, SameSite=Strict, 400 days) |
| `recipes_session` | `POST /api/session` | plain HTTP (HttpOnly, SameSite=Strict, 400 days) |

A session cookie is `<expiry>.<PIN generation>.<person id>.<HMAC-SHA256 signature>`, signed with `cookie.key` from the data folder. It stops working when it expires, when the PIN is changed or removed (the generation goes up), or when the person is removed. Wrong PINs are rate limited per address and in total (`429`). Details are on [Home network](#/settings/help/home-network).

## The guard

`auth::guard` wraps every route, in this order:

1. **Host check.** The `Host` (or HTTP/2 `:authority`) must be `127.0.0.1:<port>` / `localhost:<port>` on loopback, or `recipes.local` / one of the Mac's private IPs on the home network. Otherwise `421 unknown host`. This blocks DNS rebinding.
2. **CSRF check**, for every method except GET, HEAD and OPTIONS:
   - the header `X-Recipes: 1` must be present, else `403 missing X-Recipes header`;
   - if an `Origin` header is sent, its host must equal the request's host, else `403 cross-origin request refused`.
3. **Role and person** are attached to the request (`Caller`, `Who`).
4. After the handler: a successful write by a known person goes to the activity log (below), and every response gets `X-Content-Type-Options: nosniff` and `Referrer-Policy: same-origin`.

Request bodies are limited to 30 MB, responses are compressed (gzip, brotli), and errors are JSON `{"error": "…"}` except for the guard's and the role check's plain-text answers.

## Routes

"Anyone" means anyone who passes the host check. "Signed in" means the request names a person: a signed-in device, or the desktop window.

### Session and sign-in

| Method | Path | Purpose | Who |
|---|---|---|---|
| GET | `/api/ping` | Answers `ok` (used to probe for HTTPS) | anyone |
| GET | `/api/session` | Role, person, whether a PIN is set, AI on, shell available, version, whether the first-start setup is pending | anyone |
| POST | `/api/session` | Sign in `{name, full_name, pin}`: adds the person if the name is new (then `full_name` is required), checks the PIN when one is set, records the device, sets the session cookie | anyone |
| DELETE | `/api/session` | Sign out (clears the cookie) | anyone |
| GET | `/auth?k=<token>` | Exchanges the launch token for the admin cookie and redirects to `/` | loopback only |

### Recipes folder

| Method | Path | Purpose | Who |
|---|---|---|---|
| GET | `/api/vault` | The whole index (files, folders, folder icons and colours) as JSON, with an ETag (`304` when unchanged) | Reader |
| GET | `/api/events` | Server-sent events: `vault` (new index ETag), `messages` (whiteboard changed), `made` (a meal was logged) | Reader |
| GET | `/api/files/{path}` | One file, with an ETag. Files that aren't notes, images or PDFs, and SVGs, are sent as downloads with `Content-Security-Policy: sandbox` | Reader |
| PUT | `/api/files/{path}` | Write a file. Needs `If-Match: "<etag>"` (overwrite) or `If-None-Match: *` (create only), else `428`; a mismatch is `409` with the current ETag. `?unique=true` picks a free name. Notes are limited to 2 MB. The old version is kept in `.trash/versions/` first | Writer |
| DELETE | `/api/files/{path}` | Move a file or folder to the trash; returns its `trash_id` | Writer |
| POST | `/api/fs/move` | Rename or move `{from, to}`; `409` if the target exists (a change of letter case only is allowed) | Writer |
| POST | `/api/fs/mkdir` | Create a folder `{path}` | Writer |
| PUT | `/api/folders` | Replace `.folders.yaml` (folder icons and colours) with the body, up to 256 KB | Writer |
| GET | `/api/trash` | List the trash | Writer |
| POST | `/api/trash/restore` | Restore `{id}`; returns the restored path | Writer |
| DELETE | `/api/trash` | Empty the trash | Admin |
| GET | `/api/ingredients` | The ingredient catalogue and the known units, with an ETag | Reader |

Paths are relative to the recipes folder, `/`-separated, without `..` or dot components (`VaultPath`).

### Import

Import runs as a job: start it, poll its status, then save. Nothing is written until save, except a file kept as a view-only recipe. See [Importing](#/settings/help/importing).

| Method | Path | Purpose | Who |
|---|---|---|---|
| POST | `/api/import/url` | Start importing a web page `{url}`; returns `202 {job}` | Writer |
| POST | `/api/import/file` | Upload a file (multipart): starts a job, or with mode `keep` saves a PDF or photo as a view-only recipe | Writer |
| POST | `/api/import/from-vault` | Convert a PDF already in the recipes folder `{path}` into an editable draft | Writer |
| POST | `/api/import/check` | Check a draft `{markdown}` and return warnings (`issues`) | Writer |
| GET | `/api/import/{job}` | Job status: running, failed with an error, or the draft | Writer |
| GET | `/api/import/{job}/image/{i}` | One candidate image of the draft | Writer |
| POST | `/api/import/{job}/save` | Save the draft `{folder, name, markdown, images, cover}` with its images and original | Writer |
| DELETE | `/api/import/{job}` | Cancel and forget the job | Writer |

### Settings (desktop only)

| Method | Path | Purpose | Who |
|---|---|---|---|
| GET | `/api/settings` | Recipes folder, PIN set, AI settings and whether a key is saved, sharing and its status, keep awake, open at login, GitHub status, Editing as | Admin |
| PUT | `/api/settings` | Change any of `ai`, `lan`, `keep_awake`, `vault`, `setup_done`, `editing_as` (`""` for nobody); starts or stops sharing as needed; returns the settings | Admin |
| PUT | `/api/settings/pin` | Set `{pin}` (6 characters or more), or remove it with `""`; signs every device out | Admin |
| PUT | `/api/settings/ai-key` | Save `{key}` in the Keychain, or delete it with `""` | Admin |
| POST | `/api/setup/trust-local` | Trust the local CA in this Mac's login keychain (macOS asks for the password) | Admin |

### People

| Method | Path | Purpose | Who |
|---|---|---|---|
| GET | `/api/users` | Everyone's names, for the sign-in page; the desktop app also gets each person's devices | anyone |
| POST | `/api/users` | Add a person `{name, full_name}` | Admin |
| PUT | `/api/users/{id}` | Rename `{name, full_name}` | Admin |
| DELETE | `/api/users/{id}` | Remove a person (signs out their devices) | Admin |
| GET | `/api/users/{id}/activity` | Their latest 200 changes, newest first | Admin |

See [People](#/settings/help/people).

### Whiteboard and meals

| Method | Path | Purpose | Who |
|---|---|---|---|
| GET | `/api/messages` | The whiteboard notes still showing, newest first | anyone |
| POST | `/api/messages` | Add a note `{text, expires}` (up to 500 characters; `expires` in Unix seconds, or none for until removed) | signed in |
| DELETE | `/api/messages/{id}` | Remove any note | signed in |
| GET | `/api/made` | The log of meals made | anyone |
| POST | `/api/made` | Log the recipe `{path}` as made now; returns the whole log | anyone |
| POST | `/api/feedback` | Append `{kind, text, context}` to `FEEDBACK.md` in the recipes folder (`kind`: Bug, Idea or Other; text up to 5000 characters, context up to 2000) | signed in |

### AI

All answer `404` while AI is off and `502` when the provider fails. See [AI assistant](#/settings/help/ai-assistant).

| Method | Path | Purpose | Who |
|---|---|---|---|
| POST | `/api/ai/fix` | Corrected import draft `{markdown, source_text}` → `{markdown}` | Writer |
| POST | `/api/ai/tidy` | Formatting proposal for a recipe `{markdown}` → `{markdown}` | Writer |
| POST | `/api/ai/ask` | Answer `{markdown, question}` → `{answer}` | Writer |

### GitHub

See [GitHub sync](#/settings/help/github-sync).

| Method | Path | Purpose | Who |
|---|---|---|---|
| POST | `/api/git/connect` | Connect `{url, token}`, run a first sync; returns the sync status | Admin |
| POST | `/api/git/sync` | Sync now; returns the status | Admin |
| DELETE | `/api/git` | Stop syncing and delete the token | Admin |

### Desktop shell

`POST /api/shell/{action}`, Admin only, and only in `Recipes.app`: the headless server answers `501 not running in the desktop app`.

| Action | Body | Does |
|---|---|---|
| `reveal` | `{path}` (`""` = the recipes folder) | Show in Finder |
| `choose-vault` | none | Folder picker; returns `{path}` |
| `print` | none | Print the window |
| `open-url` | `{url}` (`http://` or `https://` only) | Open in the default browser |
| `restart` | none | Restart the app |
| `autostart` | `{enabled}` (optional) | Set or read Open at login; returns `{enabled}` |
| `update-check` | none | Returns `{version, notes}`, or nulls when up to date |
| `update-install` | none | Download, install and restart |

### Outside the API

| Method | Path | Purpose | Who |
|---|---|---|---|
| GET | `/ca.crt` | The local CA certificate as `Recipes-CA.crt` (404 before sharing has started) | anyone |
| GET | `/setup` | The device setup page; plain-HTTP listener only | anyone |
| GET | anything else | The embedded UI (`index.html` for unknown paths); unknown `/api/…` paths get `404 no such endpoint` | anyone |

## The activity log

`activity.log` in the data folder: one JSON line per change, appended by the guard after a **successful** write (any method but GET, HEAD and OPTIONS) by a known person, desktop included:

```json
{"t": 1767225600, "user": "<person id>", "name": "Ann", "method": "PUT", "path": "/api/files/Pies/Pumpkin%20Pie.md"}
```

- Not logged: `/api/session`, `/api/shell/…`, `/api/ai/…`, and import steps other than `/api/import/{job}/save`, since they change nothing.
- Moves and import saves record the endpoint, not the recipe's name (that is in the request body).
- The file is created readable by the owner only and is never rotated. Settings → People shows each person's latest changes through `GET /api/users/{id}/activity`.
