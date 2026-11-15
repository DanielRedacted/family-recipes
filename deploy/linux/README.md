# Linux / Raspberry Pi: kept for later

Nothing in this folder is used by the macOS app. It keeps the Linux and Raspberry Pi
deployment work from the two earlier projects, so a Pi version can start from it.

## Docker and Caddy are not needed

The earlier projects used Caddy for HTTPS and Docker to run Caddy and a local Ollama model.
This app does both jobs itself:

- **HTTPS:** it creates its own certificate authority and serves `https://recipes.local`
  on port 443 (`crates/core/src/lan.rs`).
- **AI:** it uses an API key, or a local model (Ollama, LM Studio) through its OpenAI-compatible
  address on the same machine. No container is needed for either.

A Pi build needs neither Docker nor Caddy. It needs the native `recipes-server` binary run
by systemd, with Avahi publishing `recipes.local`.

## What is here

| File | From | What it was |
|---|---|---|
| `from-recipes/Dockerfile`, `.dockerignore` | `recipes/` (commit bdb3151, its last) | Two-stage Rust build on `debian:bookworm-slim`. Usable as a pattern if a container is ever wanted; drop `SQLX_OFFLINE`. |
| `from-recipes/docker-compose.yml` | `recipes/` | App + Ollama + Caddy containers. |
| `from-recipes/Caddyfile` | `recipes/` | Reverse proxy for `recipes.local`. |
| `from-recipes/deploy.sh` | `recipes/` | Ubuntu/Debian bootstrap: installs Docker CE, starts the stack. |
| `from-recipes/config.toml` | `recipes/` | Old app config (Ollama URL, ports). |
| `from-recipes/README-lan-hosting.md` | `recipes/README.md` | Per-device certificate trust steps (Linux clients need `avahi-daemon` + `libnss-mdns`; `update-ca-certificates`). |
| `from-recipes/mdns.rs`, `from-cookbook/mdns.rs` | both projects | mDNS registration with the pure-Rust `mdns-sd` crate (works on Linux without Avahi's tools). |
| `from-cookbook/NOTES.md` | `cookbook/handoff/deploy/` (commit afb372b) | The deployment decisions for a Pi (native binary under systemd, TLS on) and a list of defects in the files above. |

Known defects in the copied files (from `NOTES.md`; don't reuse them as they are):

- The Caddyfile serves plain HTTP although the README describes HTTPS.
- Commit bdb3151 put the app on the host network (`network_mode: host`) and pointed Caddy at
  `host.docker.internal` with an `extra_hosts` entry, which fixed mDNS and the proxy. `NOTES.md`
  was written before that and still lists them as broken.

The rest of what the two projects left behind (code, prompts, product ideas) is in
[docs/legacy/](../../docs/legacy/README.md).

## Porting checklist for a Pi build

These are the macOS-only parts of the current code and what would replace each on Linux.

| Area | macOS today | Linux replacement |
|---|---|---|
| Run as a service | Tauri app with a menu-bar icon | `recipes-server --vault … --data … --lan` under a systemd unit (see the sketch below) |
| Ports 443/80 | allowed for normal users on macOS | `AmbientCapabilities=CAP_NET_BIND_SERVICE` in the unit, or use 8443/8080 (the code already falls back) |
| `recipes.local` | `dns-sd -P` child process (`lan.rs` `advertise`) | `avahi-publish -a recipes.local <ip>` as a child process, or the `mdns-sd` crate (see `mdns.rs` here) |
| Keep awake | `caffeinate` (`lan.rs` `set_keep_awake`) | not needed on a Pi; `systemd-inhibit` if ever needed |
| Trust the CA on the host | `security add-trusted-cert` (`lan.rs` `trust_on_this_mac`) | `cp ca.crt /usr/local/share/ca-certificates/recipes.crt && update-ca-certificates` |
| AI key | macOS Keychain (`keyring` with `apple-native`, `ai.rs`) | `keyring` with the `linux-native` feature (Secret Service), or a key file with 0600 permissions in the data folder |
| `.doc` / `.rtf` / `.odt` import | `/usr/bin/textutil` (`import.rs` `textutil_html`) | `libreoffice --headless --convert-to html`, or `pandoc` |
| Build | `cargo build` on the Mac | `cargo build --release` on the Pi, or `cross build --release --target aarch64-unknown-linux-gnu` |

systemd unit sketch (untested):

```ini
[Unit]
Description=Recipes
After=network-online.target avahi-daemon.service
Wants=network-online.target

[Service]
User=recipes
ExecStart=/usr/local/bin/recipes-server --vault /srv/recipes --data /var/lib/recipes --lan
AmbientCapabilities=CAP_NET_BIND_SERVICE
Restart=on-failure

[Install]
WantedBy=multi-user.target
```
