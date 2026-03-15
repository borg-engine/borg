# CLAUDE.md

## Overview

Borg is an autonomous AI agent orchestrator written in Rust. It connects to Telegram, WhatsApp, and Discord to respond to chat messages (via Claude Code subprocess), and runs an engineering pipeline that autonomously creates, tests, and merges code changes.

## Project Structure

```
apps/
  web/                    # React + Vite + Tailwind web dashboard
  desktop/                # Tauri 2 desktop app (wraps web frontend)
  mobile/                 # React Native + Expo mobile app (iOS/Android)
packages/
  api/                    # @borg/api — shared types + framework-agnostic API client
  hooks/                  # @borg/hooks — shared React hooks (WIP)
borg-rs/                  # Rust implementation (active codebase)
  crates/
    borg-core/            # Pipeline, DB, config, agent traits, modes
    borg-agent/           # Claude + Ollama agent backends
    borg-server/          # Axum HTTP server, routes, logging
    borg-domains/         # Domain-specific pipeline modes (swe, legal, web, crew, sales, data, chef)
container/
  Dockerfile              # Pipeline agent image (bun + claude CLI)
  entrypoint.sh           # Agent entrypoint: parses JSON input, runs claude
sidecar/                  # Unified Discord+WhatsApp bridge (bun, discord.js + Baileys)
```

Monorepo managed by bun workspaces (root `package.json` defines `packages/*` and `apps/*`).

## Build & Test

```bash
just t                 # Run all unit tests
just b                 # Build release binary
just deploy            # Build and restart service
just dash              # Build web dashboard
just desktop           # Build Tauri desktop app
just mobile            # Start Expo dev server for mobile app
just mobile-check      # Verify mobile bundles (iOS + Android)
just setup             # Full setup (image + sidecar + dashboard + build)
```

Prefer `just` commands over ad hoc shell invocations when a matching recipe exists.

## Dashboard Context

The dashboard lives in `apps/web/` and can run in multiple domain modes. Check the active mode before making UI assumptions.

Key mode boundaries in the dashboard:
- `projects-panel.tsx`: Legal renders ChatBody + DocumentViewWrapper (lines ~897-956); SWE renders file manager + cloud storage (lines ~957-1440)
- `project-detail.tsx`: Legal hides Tasks/Activity tabs; SWE shows all tabs + mode badges
- `task-creator.tsx`: Legal shows a Task Type dropdown; SWE shows a mode picker
- Mode checks use `isSWE`/`isLegal` from `useDashboardMode()` — grep for these to find boundaries
- Terminology (projects vs matters) is handled by `vocabulary.ts`

## Code Style

- No slop comment prefixes (`AUDIT:`, `SECURITY:`, `NOTE:`). `TODO:` is fine.
- Use `bun` for all JavaScript and TypeScript package management and script execution. Do not use `npm`, `yarn`, or `pnpm`.
- Prefer `just <recipe>` for standard project workflows before reaching for raw commands.

## Git Commits

- Use conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `docs:`, `test:`
