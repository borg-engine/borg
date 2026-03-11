# Borg

Autonomous Work Engine. Domain-specific pipelines that research, draft, build, review, and ship work end-to-end.

## Quick Start

```bash
git clone <repo-url> borg && cd borg
just setup   # builds binary, sidecar, dashboard
```

Create `.env`:

```bash
TELEGRAM_BOT_TOKEN=<from @BotFather>
PIPELINE_REPO=/path/to/your/repo
PIPELINE_TEST_CMD="cargo test"
```

```bash
just deploy   # build + restart
```

Dashboard at `http://127.0.0.1:3131`.

## Pipelines

Built-in pipeline categories:

- **SWE** — implement, validate, lint, rebase, merge via git PR
- **Legal** — research-heavy service workflows with compliance checks and human sign-off
- **Knowledge** — general-purpose agent workflows for document processing and analysis

Tasks move through configurable phases. Each task gets its own git worktree and branch. Agents run in bubblewrap sandboxes or Docker containers (`SANDBOX_BACKEND`). Sessions persist across retries.

Custom pipelines can be created via the dashboard or the API.

## Messaging

Chat with agents from any of the supported platforms:

- **Discord** — per-user bot, responds to mentions and DMs
- **Telegram** — per-user bot via @BotFather token
- **Slack** — workspace integration via Socket Mode
- **WhatsApp** — via Baileys (QR code pairing)
- **Web** — built-in chat in the dashboard

Each conversation gets its own persistent session. Chat agents can search project documents, query the knowledge base, and create pipeline tasks.

Bot connections are managed per-user from the Connections tab in the dashboard.

## BorgSearch

Document search layer built on [Vespa](https://vespa.ai). Supports full-text, semantic (embedding-based), and hybrid search across all project documents and knowledge files.

- Embedding models: `voyage-4-large` (default), `voyage-law-2`, `voyage-finance-2`, `voyage-code-3` — selected automatically per project mode
- Coverage endpoint for exhaustive corpus review (`/api/borgsearch/coverage`)
- Agents use BorgSearch via MCP tools: `search_documents`, `check_coverage`, `read_document`

## Commands

| Just | Description |
|---|---|
| `just ship` | Dashboard + test + build + deploy |
| `just setup` | Full setup (sidecar + dashboard + build) |
| `just deploy` | Build + restart |
| `just t` | Run tests |
| `just b` | Build release binary |
| `just dash` | Build dashboard |

Requires Rust, Bun.

## License

Copyright (C) 2026 Sasha Duke and contributors.

Licensed under the GNU Affero General Public License v3.0 — see [LICENSE](LICENSE).
