#!/bin/bash
# Build script for Cloudflare Pages deployment
# Set BORG_API_URL env var in Cloudflare Pages settings
set -e

API_URL="${BORG_API_URL:-}"

bun install
bun run build

# Inject API URL into built index.html
if [ -n "$API_URL" ]; then
  sed -i "s|window.__BORG_API_URL__ = \"\"|window.__BORG_API_URL__ = \"$API_URL\"|" dist/index.html
fi
