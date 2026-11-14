## LAN hosting (https://recipes.local)

Expose the app to your local network over HTTPS using the included Docker Compose setup (Ollama + Caddy). The app binary must run natively — not in Docker — so that mDNS service registration works.

### Start

```bash
# Start Ollama and Caddy
docker compose up -d

# Pull your model into Ollama (first time only)
docker compose exec ollama ollama pull llama3.2:latest

# Run the app (registers recipes.local on your LAN via mDNS)
cargo run
# or: ./target/release/recipe-app
```

### Trust the TLS certificate

Caddy uses its own local CA (`tls internal`). Each device needs to trust it once before the browser accepts `https://recipes.local` without a warning.

**This machine (macOS):**
```bash
docker cp $(docker compose ps -q caddy):/data/caddy/pki/authorities/local/root.crt caddy-root.crt
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain caddy-root.crt
```
Restart your browser after running this.

**Other devices:**
Copy `caddy-root.crt` from the host and install it:

| Platform | How |
|---|---|
| macOS | Double-click → Keychain Access → mark as "Always Trust" |
| iOS | Settings → General → VPN & Device Management → install profile → Certificate Trust Settings → enable |
| Windows | Double-click → Install Certificate → Local Machine → Trusted Root CAs |
| Android | Settings → Security → Install from storage |
| Linux | `sudo cp caddy-root.crt /usr/local/share/ca-certificates/ && sudo update-ca-certificates` |

### Linux-specific setup

Docker Engine (non-Desktop) doesn't inject `host.docker.internal` automatically — `docker-compose.yml` already includes the required `extra_hosts` mapping.

mDNS `.local` resolution requires Avahi:
```bash
sudo apt install avahi-daemon libnss-mdns
sudo systemctl enable --now avahi-daemon
```

Verify it's working: `avahi-resolve -n recipes.local` should return your machine's IP.

