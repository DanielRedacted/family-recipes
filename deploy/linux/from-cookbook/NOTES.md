# deploy NOTES

The four files here are verbatim copies of the OLD app's deploy setup. **They are
reference, not templates — the old setup was internally inconsistent** (WARTS.md #5–7).
Cookbook's target deployment is different and simpler in places:

## Target (decided)

- **cookbook binary: native** on the Pi (systemd unit) — mDNS requires host networking.
  Cross-compiled `aarch64-unknown-linux-gnu`, or built on the Pi.
- **Ollama: native** ARM64 install (`curl -fsSL https://ollama.com/install.sh | sh`
  ships a systemd unit) — not Docker. 16GB Pi → 7–8B Q4 model; 3B fallback if slow.
- **Caddy: keep, WITH TLS this time** — `https://cookbook.local`, `tls internal`,
  trust the CA once per device. Caddy native (`apt install caddy`) or Docker; if Docker,
  it's the only container left.
- **Voice daemon (P10): systemd unit** running the Python service.

## What was wrong in these copied files — fix when writing cookbook's versions

1. `docker-compose.yml` builds and runs the app in a container, contradicting the
   README's "must run natively for mDNS". Cookbook: no app container at all.
2. `docker-compose.yml` lacks the `extra_hosts: host.docker.internal:host-gateway`
   entry its own README claimed was present. If Caddy runs in Docker and proxies to the
   native binary, Caddy's the one that needs `extra_hosts` (or use the Pi's LAN IP).
3. `Caddyfile` serves plain HTTP :80, no `tls internal`, despite the documented HTTPS
   story. Cookbook's Caddyfile:
   ```
   cookbook.local {
       tls internal
       reverse_proxy localhost:3000
   }
   ```
4. `deploy.sh` is a decent Ubuntu Docker bootstrap — mostly obsolete for a native
   deployment; keep its `set -euo pipefail` discipline and first-run hints
   (model pull, CA trust) in whatever setup script replaces it.

## Still-valid references

- `Dockerfile`: good two-stage pattern (builder + slim runtime) if a container build is
  ever wanted; drop `SQLX_OFFLINE` (no sqlx in cookbook).
- CA-trust ritual per device (macOS): extract Caddy's root cert, then
  `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain caddy-root.crt`.
  Linux clients need Avahi for `.local`: `sudo apt install avahi-daemon libnss-mdns`.
- Avahi note applies to the Pi itself too (mdns-sd crate handles registration, but
  clients resolving cookbook.local need mDNS working on their end).
