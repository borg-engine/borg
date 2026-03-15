#!/bin/bash
# Build script for Cloudflare Pages deployment
# Set API_BASE_URL env var in Cloudflare Pages settings
set -e

API_URL="${API_BASE_URL:-}"

bun install
bun run build

# Inject API URL into built index.html
if [ -n "$API_URL" ]; then
  sed -i "s|window.__API_BASE_URL__ = \"\"|window.__API_BASE_URL__ = \"$API_URL\"|" dist/index.html
fi
