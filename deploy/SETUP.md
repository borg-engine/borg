# Borg Deployment — app.borg.legal

Hetzner VPS + Cloudflare Tunnel + Cloudflare Access (SSO).

## Architecture

```
  Browser → app.borg.legal
               │
         Cloudflare Edge
         ┌─────────────────┐
         │  Access (SSO)   │  ← Google/GitHub login
         │  Tunnel         │  ← encrypted tunnel to VPS
         └────────┬────────┘
                  │
         Hetzner VPS (CAX21)
         ┌────────┴────────┐
         │  cloudflared     │  ← tunnel daemon
         │  borg-server     │  ← API + dashboard on :3131
         │  (docker agents) │  ← pipeline spawns agent containers
         └─────────────────┘
```

No ports exposed. No Caddy/nginx. No TLS config. Cloudflare handles everything.

**Cost**: ~€6.49/mo (Hetzner) + free (Cloudflare). Domain: ~$7/yr (Porkbun).

---

## Step 1: Cloudflare Setup (~10 min)

### 1.1 Add zone

1. [dash.cloudflare.com](https://dash.cloudflare.com) → **Add a Site** → `borg.legal` → Free plan
2. Copy the two nameservers Cloudflare assigns

### 1.2 Update nameservers at Porkbun

1. [porkbun.com](https://porkbun.com) → `borg.legal` → Nameservers
2. Replace Porkbun defaults with Cloudflare's two nameservers
3. Disable DNSSEC if enabled (conflicts with Cloudflare)
4. Wait for propagation (~30 min)

### 1.3 Create Tunnel

1. Cloudflare dashboard → **Zero Trust** → **Networks** → **Tunnels** → **Create**
2. Name: `borg`
3. Choose **Cloudflared** connector
4. Copy the tunnel token (starts with `eyJ...`)
5. Add public hostname:
   - Subdomain: `app`, Domain: `borg.legal`
   - Service: `http://borg:3131`

### 1.4 Add Access Policy (SSO)

1. **Zero Trust** → **Access** → **Applications** → **Add an Application**
2. Type: **Self-hosted**
3. Application domain: `app.borg.legal`
4. Name: `Borg Dashboard`
5. Identity providers: add **Google** and/or **GitHub** (follow the prompts — just needs an OAuth client ID/secret from each provider)
6. Policy: **Allow** → Include → **Emails** → add your email(s)

---

## Step 2: Hetzner VPS (~5 min)

### 2.1 Install hcloud CLI

```bash
sudo pacman -S hcloud-cli   # Arch
# Or: curl -sSL https://github.com/hetznercloud/cli/releases/latest/download/hcloud-linux-amd64.tar.gz | tar xz && sudo mv hcloud /usr/local/bin/
hcloud context create borg   # Paste API token from console.hetzner.cloud
```

### 2.2 Provision

```bash
# Upload SSH key
hcloud ssh-key create --name borg-key --public-key-from-file ~/.ssh/id_ed25519.pub

# Create firewall (only SSH — tunnel handles the rest)
hcloud firewall create --name borg-fw
hcloud firewall add-rule borg-fw --direction in --protocol tcp --port 22 --source-ips 0.0.0.0/0 --source-ips ::/0

# Create server
hcloud server create \
  --name borg \
  --type cax21 \
  --image ubuntu-24.04 \
  --location nbg1 \
  --ssh-key borg-key \
  --firewall borg-fw \
  --user-data-from-file deploy/cloud-init.yml
```

Wait ~3 min for cloud-init to finish installing Docker.

---

## Step 3: Deploy (~5 min)

```bash
VPS_IP=$(hcloud server ip borg)
ssh root@$VPS_IP   # or deploy@ if cloud-init user created

# Clone repo
git clone https://github.com/neuralcollective/borg.git /opt/borg
cd /opt/borg/deploy

# Create .env
cat > .env << 'EOF'
CLAUDE_CODE_OAUTH_TOKEN=your-claude-oauth-token
CLOUDFLARE_TUNNEL_TOKEN=eyJ...your-tunnel-token
SANDBOX_BACKEND=docker
CONTAINER_IMAGE=borg-agent
CONTINUOUS_MODE=true
PIPELINE_REPO=/app
DATA_DIR=/app/store
DASHBOARD_DIST_DIR=/app/dashboard/dist
MODEL=claude-sonnet-4-6
EOF

# Build agent image
cd /opt/borg && docker build -t borg-agent -f container/Dockerfile container/

# Build and start
cd deploy && docker compose up -d --build

# Check logs
docker compose logs -f borg
```

### Getting your Claude OAuth token

Copy from your local machine:
```bash
cat ~/.claude/.credentials.json | jq -r '.oauthToken'
# Or: set CLAUDE_CODE_OAUTH_TOKEN in .env
```

---

## Updating

```bash
# From your local machine:
BORG_HOST=root@$(hcloud server ip borg) bash deploy/deploy.sh

# Or manually on the VPS:
cd /opt/borg && git pull && cd deploy && docker compose up -d --build
```

## Useful Commands

```bash
# Logs
ssh root@$(hcloud server ip borg) 'cd /opt/borg/deploy && docker compose logs -f borg'

# Shell into borg container
ssh root@$(hcloud server ip borg) 'cd /opt/borg/deploy && docker compose exec borg bash'

# Restart
ssh root@$(hcloud server ip borg) 'cd /opt/borg/deploy && docker compose restart borg'

# DB backup
ssh root@$(hcloud server ip borg) 'docker compose -f /opt/borg/deploy/docker-compose.yml exec borg sqlite3 /app/store/borg.db ".backup /tmp/backup.db" && docker compose -f /opt/borg/deploy/docker-compose.yml cp borg:/tmp/backup.db .'
```
