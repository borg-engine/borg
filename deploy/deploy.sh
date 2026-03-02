#!/usr/bin/env bash
set -euo pipefail

HOST="${BORG_HOST:?BORG_HOST env var required (e.g. root@1.2.3.4)}"
REMOTE_DIR="/opt/borg"

echo "Deploying to $HOST..."

# Build dashboard locally (faster than on VPS)
echo "Building dashboard..."
(cd "$(dirname "$0")/../dashboard" && bun install && bun run build)

# Sync repo to VPS
echo "Syncing to VPS..."
rsync -az --delete \
  --exclude .git \
  --exclude node_modules \
  --exclude target \
  --exclude store \
  --exclude '.env' \
  "$(dirname "$0")/../" "$HOST:$REMOTE_DIR/"

# Build and restart on VPS
echo "Building and restarting..."
ssh "$HOST" "
    set -euo pipefail
    cd $REMOTE_DIR

    # Rebuild agent image if container/Dockerfile changed
    docker build -t borg-agent -f container/Dockerfile container/

    # Rebuild and restart borg server
    cd deploy
    docker compose build borg
    docker compose up -d
"

echo "Deploy complete. Checking health..."
sleep 5
ssh "$HOST" "curl -sf http://localhost:3131/api/health" && echo " OK" || echo " Health check failed (may still be starting)"
