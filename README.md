# Borg

Autonomous Work Engine for domain-specific pipelines that research, draft, build, review, and ship work end-to-end. Dashboard and chat integration across web, Telegram, Discord, and WhatsApp.

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

Built-in modes include:
- `sweborg` — software engineering (implement, validate, lint, rebase, merge)
- `lawborg` — research-heavy service workflows with review and human sign-off

Tasks move through configurable phases. Each task gets its own git branch. Agents run in bubblewrap sandboxes or Docker containers (`SANDBOX_BACKEND`). Sessions persist across retries.

Custom pipelines can be created via the dashboard mode creator or the API.

## Chat

Mention the bot in a registered Telegram, Discord, or WhatsApp group. Each group gets its own persistent session. Chat agents can search project documents and create pipeline tasks.

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
