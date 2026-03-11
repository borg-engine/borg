# Setting Up Borg

Step-by-step setup for a fresh machine.

## Prerequisites

- Linux (tested on Arch, should work on Ubuntu/Debian)
- Rust toolchain (`curl https://sh.rustup.rs -sSf | sh`)
- Docker daemon running
- Bun (`curl -fsSL https://bun.sh/install | bash`)
- Claude Code CLI (`bun install -g @anthropic-ai/claude-code`)
- Claude OAuth credentials (run `claude` once to log in)

## 1. Clone and Build

```bash
git clone <repo-url> borg && cd borg
just setup
```

This builds the binary, Docker agent image, sidecar deps, and dashboard.

## 2. Configure

Copy the example and fill in your values:

```bash
cp .env.example .env
```

Minimum required config:

```bash
# At least one messaging backend
TELEGRAM_BOT_TOKEN=<token from @BotFather>

# Bot identity
ASSISTANT_NAME=Borg

# Pipeline (optional — enables autonomous engineering)
PIPELINE_REPO=/absolute/path/to/target/repo
PIPELINE_TEST_CMD=cargo test
```

See `.env.example` for the full list of options.

## 3. Run

```bash
just r
```

Or with systemd (user service):

```bash
mkdir -p ~/.config/systemd/user
cp borg.service ~/.config/systemd/user/borg.service
# Edit the paths in the service file to match your install location
systemctl --user daemon-reload
systemctl --user enable --now borg
journalctl --user -u borg -f
```

## 4. Register a Chat

In your Telegram group (or Discord channel), send `/register` to the bot. Then mention it by name (e.g. `@Borg`) to trigger a response.

## 5. Create Pipeline Tasks

Send `/task Fix the login bug` in a registered chat, or let the auto-seeder discover tasks when the pipeline is idle.

## Verify It Works

- `just status` returns JSON with version and uptime
- `/ping` in Telegram responds with `pong`
- Dashboard at `http://127.0.0.1:3131`

## Discord Bot Setup

1. Go to https://discord.com/developers/applications
2. Create application → Bot → copy token
3. Enable **Message Content Intent** under Bot settings
4. Invite: OAuth2 → URL Generator → scopes: `bot` + `applications.commands` → permissions: Send Messages, Read Message History
5. Set `DISCORD_TOKEN=<token>` in `.env`
