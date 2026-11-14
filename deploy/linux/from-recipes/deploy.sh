#!/usr/bin/env bash
set -euo pipefail

# ─── Helpers ──────────────────────────────────────────────────────────────────
log()  { echo "[deploy] $*"; }
warn() { echo "[deploy] WARNING: $*" >&2; }
die()  { echo "[deploy] ERROR: $*" >&2; exit 1; }

require_root() {
    if [[ $EUID -ne 0 ]]; then
        die "Docker installation requires root. Re-run with: sudo $0"
    fi
}

# Prefer 'docker compose' (plugin); fall back to 'docker-compose' (standalone)
compose() {
    if docker compose version &>/dev/null 2>&1; then
        docker compose "$@"
    elif command -v docker-compose &>/dev/null; then
        docker-compose "$@"
    else
        die "Neither 'docker compose' nor 'docker-compose' found."
    fi
}

# ─── Docker installation (Ubuntu/Debian) ──────────────────────────────────────
install_docker() {
    log "Installing Docker..."
    require_root

    apt-get update -qq
    apt-get install -y -qq ca-certificates curl gnupg

    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg \
        | gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    chmod a+r /etc/apt/keyrings/docker.gpg

    local codename
    codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-$(echo "$VERSION_ID" | cut -d. -f1)}")

    echo \
        "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
https://download.docker.com/linux/ubuntu ${codename} stable" \
        | tee /etc/apt/sources.list.d/docker.list > /dev/null

    apt-get update -qq
    apt-get install -y -qq \
        docker-ce docker-ce-cli containerd.io \
        docker-buildx-plugin docker-compose-plugin

    log "Docker installed."
}

# ─── Main ─────────────────────────────────────────────────────────────────────
cd "$(dirname "$0")"

FIRST_RUN=false

# 1. Install Docker if missing
if ! command -v docker &>/dev/null; then
    FIRST_RUN=true
    install_docker
else
    log "Docker $(docker --version | awk '{print $3}' | tr -d ,) already installed."
fi

# 2. Enable Docker to start on boot (idempotent)
if command -v systemctl &>/dev/null; then
    systemctl enable docker &>/dev/null || true
    systemctl start  docker &>/dev/null || true
fi

# 3. Ensure required data files/dirs exist before Docker tries to mount them
if [[ ! -f recipes.db ]]; then
    log "Creating empty recipes.db..."
    touch recipes.db
    FIRST_RUN=true
fi
mkdir -p uploads

# 4. Build and deploy
if [[ "$FIRST_RUN" == true ]]; then
    log "First run — building image and starting all services..."
else
    log "Updating — rebuilding app image and restarting services..."
fi

compose up -d --build

log ""
if [[ "$FIRST_RUN" == true ]]; then
    log "Deploy complete. App will restart automatically on reboot."
    log "Access at: https://recipes.local (or http://<host-ip>)"
    log ""
    log "Pull an Ollama model if you haven't yet:"
    log "  docker exec -it \$(docker compose ps -q ollama) ollama pull llama3.2"
else
    log "Update complete. Running containers:"
    compose ps
fi
