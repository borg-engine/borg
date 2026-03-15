import { useQueryClient } from "@tanstack/react-query";
import { Building2, ChevronDown, ChevronLeft, Github, GitBranch, MessageSquare, Plug, Plus, Power, Settings2, Trash2 } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { useEffect, useState } from "react";
import {
  apiBase,
  connectDiscordBot,
  connectSlackBot,
  connectTelegramBot,
  type CustomMcpServer,
  deleteCustomMcpServer,
  disconnectDiscordBot,
  disconnectGoogle,
  disconnectMs365,
  disconnectSlackBot,
  disconnectTelegramBot,
  logoutWhatsApp,
  toggleCustomMcpServer,
  upsertCustomMcpServer,
  useCustomMcpServers,
  useGoogleStatus,
  useMs365Status,
  type UserSettings,
  updateUserSettings,
  useUserSettings,
  useWhatsAppStatus,
} from "@/lib/api";
import { cn } from "@/lib/utils";

export function ConnectionsPanel() {
  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 space-y-3 p-5 pb-3">
        <div className="flex items-center gap-3">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-amber-500/10 ring-1 ring-amber-500/20">
            <Plug className="h-6 w-6 text-amber-400" />
          </div>
          <div>
            <div className="text-[16px] font-semibold text-[var(--color-text)]">Connections</div>
            <div className="text-[13px] text-[var(--color-text-tertiary)]">Connect external services to extend your workflow</div>
          </div>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5">
        <div className="mx-auto max-w-3xl space-y-4">
          <CollapsibleSection icon={<MessageSquare className="h-4 w-4" />} title="Messaging" defaultOpen>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <DiscordCard />
              <TelegramCard />
              <WhatsAppCard />
              <SlackCard />
            </div>
          </CollapsibleSection>

          <CollapsibleSection icon={<GitBranch className="h-4 w-4" />} title="Git">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <GitHubCard />
              <GitLabCard />
              <CodebergCard />
            </div>
          </CollapsibleSection>

          <CollapsibleSection icon={<Building2 className="h-4 w-4" />} title="Productivity">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <Microsoft365Card />
              <GoogleWorkspaceCard />
            </div>
          </CollapsibleSection>

          <CollapsibleSection icon={<Plug className="h-4 w-4" />} title="Integrations" defaultOpen>
            <McpServersSection />
          </CollapsibleSection>
        </div>
      </div>
    </div>
  );
}

function CollapsibleSection({
  icon,
  title,
  defaultOpen = false,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="rounded-2xl border border-[var(--color-border)] bg-[#181614]">
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full items-center gap-2.5 px-4 py-3 text-left"
      >
        <span className="text-[var(--color-text-tertiary)]">{icon}</span>
        <span className="flex-1 text-[13px] font-semibold text-[var(--color-text)]">{title}</span>
        <ChevronDown className={cn("h-4 w-4 text-[var(--color-text-tertiary)] transition-transform", open && "rotate-180")} />
      </button>
      {open && <div className="px-4 pb-4">{children}</div>}
    </div>
  );
}

// ── Discord ───────────────────────────────────────────────────────────────

function DiscordCard() {
  const { data: userSettings } = useUserSettings();
  if (!userSettings) return null;
  return (
    <BotConnectionCard
      icon={<DiscordIcon />}
      iconBg="bg-[#5865F2]/10 ring-[#5865F2]/20"
      title="Discord"
      subtitle="Chat with your agent from any Discord server or DM"
      connected={userSettings.discord_bot_connected}
      displayName={userSettings.discord_bot_username}
      connectFn={connectDiscordBot}
      disconnectFn={disconnectDiscordBot}
      tokenFields={[{ placeholder: "Paste bot token" }]}
      setupInstructions={
        <ol className="list-decimal list-inside space-y-1.5 text-[12px]">
          <li>
            Go to the <span className="font-medium text-[var(--color-text)]">Discord Developer Portal</span>
          </li>
          <li>Create a new Application, then add a Bot</li>
          <li>
            Enable <span className="font-medium text-[var(--color-text)]">Message Content Intent</span> under Privileged Gateway
            Intents
          </li>
          <li>Copy the bot token and paste it below</li>
        </ol>
      }
    />
  );
}

// ── Telegram ──────────────────────────────────────────────────────────────

function TelegramCard() {
  const { data: userSettings } = useUserSettings();
  if (!userSettings) return null;
  return (
    <BotConnectionCard
      icon={<TelegramIcon />}
      iconBg="bg-[#229ED9]/10 ring-[#229ED9]/20"
      title="Telegram"
      subtitle="Chat with your agent from any Telegram conversation"
      connected={userSettings.telegram_bot_connected}
      displayName={userSettings.telegram_bot_username}
      formatStatus={(name) => `@${name}`}
      connectFn={connectTelegramBot}
      disconnectFn={disconnectTelegramBot}
      tokenFields={[{ placeholder: "Paste bot token from @BotFather" }]}
      setupInstructions={
        <ol className="list-decimal list-inside space-y-1.5 text-[12px]">
          <li>
            Open <span className="font-medium text-[var(--color-text)]">@BotFather</span> in Telegram
          </li>
          <li>
            Send <code className="rounded bg-[var(--color-border)] px-1.5 py-0.5 text-[11px] text-amber-300">/newbot</code> and
            follow the prompts
          </li>
          <li>Copy the bot token and paste it below</li>
        </ol>
      }
    />
  );
}

// ── WhatsApp ──────────────────────────────────────────────────────────────

function WhatsAppCard() {
  const queryClient = useQueryClient();
  const { data: waStatus, isLoading } = useWhatsAppStatus();
  const [disconnecting, setDisconnecting] = useState(false);

  if (isLoading || !waStatus) return null;
  if (waStatus.disabled) return null;

  const jidLabel = waStatus.jid ? waStatus.jid.split("@")[0].split(":")[0] : undefined;

  async function handleLogout() {
    setDisconnecting(true);
    try {
      await logoutWhatsApp();
      queryClient.invalidateQueries({ queryKey: ["whatsapp-status"] });
    } finally {
      setDisconnecting(false);
    }
  }

  return (
    <Card>
      <CardHeader
        icon={<WhatsAppIcon />}
        iconBg="bg-[#25D366]/10 ring-[#25D366]/20"
        title="WhatsApp"
        subtitle="Chat with your agent from any WhatsApp conversation"
        status={waStatus.connected ? "connected" : undefined}
        statusLabel={waStatus.connected ? jidLabel : undefined}
      />

      {waStatus.connected ? (
        <div className="space-y-3">
          <div className="rounded-xl border border-emerald-500/15 bg-emerald-500/[0.04] px-4 py-3 text-[12px] text-[var(--color-text-secondary)]">
            Connected and receiving messages
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleLogout}
              disabled={disconnecting}
              className="rounded-lg border border-red-500/20 bg-red-500/[0.06] px-3 py-1.5 text-[12px] text-red-400/80 transition-colors hover:bg-red-500/[0.12] hover:text-red-400"
            >
              {disconnecting ? "Disconnecting..." : "Disconnect"}
            </button>
          </div>
        </div>
      ) : waStatus.qr ? (
        <div className="space-y-3 pt-1">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/60 px-4 py-3 text-[12px] text-[var(--color-text-secondary)] space-y-2">
            <p className="font-medium text-[var(--color-text)]">Scan to connect</p>
            <p>
              Open WhatsApp on your phone, go to <span className="text-[var(--color-text)]">Linked Devices</span>, and scan this
              QR code.
            </p>
          </div>
          <div className="flex justify-center rounded-xl border border-[var(--color-border)] bg-white p-4">
            <QRCodeSVG value={waStatus.qr} size={200} />
          </div>
        </div>
      ) : (
        <div className="space-y-3 pt-1">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/60 px-4 py-3 text-[12px] text-[var(--color-text-secondary)] space-y-2">
            <p className="font-medium text-[var(--color-text)]">Connecting...</p>
            <p>The WhatsApp bridge is starting up. A QR code will appear here shortly for you to scan.</p>
          </div>
          <div className="flex items-center gap-3 rounded-xl border border-dashed border-[var(--color-border)] px-4 py-4">
            <div className="h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-[var(--color-text-faint)] border-t-amber-400" />
            <span className="text-[12px] text-[var(--color-text-tertiary)]">Waiting for QR code...</span>
          </div>
        </div>
      )}
    </Card>
  );
}

// ── GitHub ─────────────────────────────────────────────────────────────────

function GitHubCard() {
  const { data: userSettings } = useUserSettings();
  if (!userSettings) return null;
  return (
    <PatCard
      icon={<Github className="h-4.5 w-4.5 text-[var(--color-text)]" />}
      iconBg="bg-[var(--color-text)]/8 ring-[var(--color-text)]/15"
      title="GitHub"
      subtitle="Personal access token for pushing branches, creating PRs, and cloning private repos"
      isSet={userSettings.github_token_set}
      placeholder="ghp_..."
      settingKey="github_token"
    />
  );
}

// ── GitLab ─────────────────────────────────────────────────────────────────

function GitLabIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4.5 w-4.5" fill="currentColor">
      <path d="M22.65 14.39L12 22.13 1.35 14.39a.84.84 0 0 1-.3-.94l1.22-3.78 2.44-7.51A.42.42 0 0 1 4.82 2a.43.43 0 0 1 .58 0 .42.42 0 0 1 .11.18l2.44 7.49h8.1l2.44-7.51A.42.42 0 0 1 18.6 2a.43.43 0 0 1 .58 0 .42.42 0 0 1 .11.18l2.44 7.51L23 13.45a.84.84 0 0 1-.35.94z" />
    </svg>
  );
}

function GitLabCard() {
  const { data: userSettings } = useUserSettings();
  if (!userSettings) return null;
  return (
    <PatCard
      icon={<GitLabIcon />}
      iconBg="bg-[#FC6D26]/8 ring-[#FC6D26]/15"
      title="GitLab"
      subtitle="Personal access token for cloning private GitLab repos"
      isSet={userSettings.gitlab_token_set}
      placeholder="glpat-..."
      settingKey="gitlab_token"
    />
  );
}

// ── Codeberg ───────────────────────────────────────────────────────────────

function CodebergIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4.5 w-4.5" fill="currentColor">
      <path d="M11.955.49A11.955 11.955 0 0 0 0 12.444a11.955 11.955 0 0 0 11.955 11.955 11.955 11.955 0 0 0 11.955-11.955A11.955 11.955 0 0 0 11.955.489zm0 1.64a10.315 10.315 0 0 1 10.315 10.315 10.315 10.315 0 0 1-10.315 10.315A10.315 10.315 0 0 1 1.64 12.445 10.315 10.315 0 0 1 11.955 2.13zM8.682 6.968v.002c-.43 0-.863.195-1.145.571L4.1 12.119a1.452 1.452 0 0 0 0 1.714l3.437 4.578c.564.753 1.727.753 2.291 0l.604-.804-2.833-3.774a.484.484 0 0 1 0-.572l2.833-3.772-.604-.805a1.452 1.452 0 0 0-1.146-.516zm6.636 0c-.43 0-.863.195-1.145.571l-.604.805 2.833 3.772a.484.484 0 0 1 0 .572l-2.833 3.774.604.804c.564.753 1.727.753 2.291 0l3.437-4.578a1.452 1.452 0 0 0 0-1.714l-3.437-4.578a1.452 1.452 0 0 0-1.146-.428z" />
    </svg>
  );
}

function CodebergCard() {
  const { data: userSettings } = useUserSettings();
  if (!userSettings) return null;
  return (
    <PatCard
      icon={<CodebergIcon />}
      iconBg="bg-[#2185D0]/8 ring-[#2185D0]/15"
      title="Codeberg"
      subtitle="Personal access token for cloning private Codeberg repos"
      isSet={userSettings.codeberg_token_set}
      placeholder="codeberg PAT..."
      settingKey="codeberg_token"
    />
  );
}

// ── Shared PAT card ────────────────────────────────────────────────────────

function PatCard({
  icon,
  iconBg,
  title,
  subtitle,
  isSet,
  placeholder,
  settingKey,
}: {
  icon: React.ReactNode;
  iconBg: string;
  title: string;
  subtitle: string;
  isSet: boolean;
  placeholder: string;
  settingKey: string;
}) {
  const { refetch } = useUserSettings();
  const [editing, setEditing] = useState(false);
  const [token, setToken] = useState("");
  const [saving, setSaving] = useState(false);

  async function handleSave() {
    setSaving(true);
    try {
      await updateUserSettings({ [settingKey]: token } as Partial<UserSettings>);
      setToken("");
      setEditing(false);
      await refetch();
    } finally {
      setSaving(false);
    }
  }

  async function handleClear() {
    setSaving(true);
    try {
      await updateUserSettings({ [settingKey]: "" } as Partial<UserSettings>);
      await refetch();
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card>
      <CardHeader
        icon={icon}
        iconBg={iconBg}
        title={title}
        subtitle={subtitle}
        status={isSet ? "connected" : undefined}
        statusLabel={isSet ? "Token configured" : undefined}
      />
      {isSet && !editing ? (
        <div className="flex items-center gap-2 pt-1">
          <button
            onClick={() => setEditing(true)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)]"
          >
            Update Token
          </button>
          <button
            onClick={handleClear}
            disabled={saving}
            className="rounded-lg border border-red-500/20 bg-red-500/[0.06] px-3 py-1.5 text-[12px] text-red-400/80 transition-colors hover:bg-red-500/[0.12] hover:text-red-400"
          >
            Remove
          </button>
        </div>
      ) : (
        <div className="space-y-2 pt-1">
          <div className="flex items-center gap-2">
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSave()}
              placeholder={placeholder}
              autoFocus={editing}
              className="flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
            />
            <button
              onClick={handleSave}
              disabled={saving || !token.trim()}
              className={cn(
                "rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20",
                (saving || !token.trim()) && "opacity-40 cursor-not-allowed",
              )}
            >
              Save
            </button>
            {isSet && (
              <button
                onClick={() => {
                  setEditing(false);
                  setToken("");
                }}
                className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text)]"
              >
                Cancel
              </button>
            )}
          </div>
        </div>
      )}
    </Card>
  );
}

// ── Slack ──────────────────────────────────────────────────────────────────

function SlackCard() {
  const { data: userSettings } = useUserSettings();
  if (!userSettings) return null;
  return (
    <BotConnectionCard
      icon={<SlackIcon />}
      iconBg="bg-[#E01E5A]/8 ring-[#E01E5A]/15"
      title="Slack"
      subtitle="Chat with your agent from any Slack channel"
      connected={userSettings.slack_bot_connected}
      displayName={userSettings.slack_bot_name}
      formatStatus={(name) => `@${name}`}
      connectFn={connectSlackBot}
      disconnectFn={disconnectSlackBot}
      tokenFields={[
        { placeholder: "Bot Token (xoxb-...)" },
        { placeholder: "App-Level Token (xapp-...)" },
      ]}
      setupInstructions={
        <ol className="list-decimal list-inside space-y-1.5 text-[12px]">
          <li>
            Go to <span className="font-medium text-[var(--color-text)]">api.slack.com/apps</span> and create a new app
          </li>
          <li>
            Enable <span className="font-medium text-[var(--color-text)]">Socket Mode</span> and generate an App-Level Token{" "}
            <code className="rounded bg-[var(--color-border)] px-1.5 py-0.5 text-[11px] text-amber-300">xapp-...</code>
          </li>
          <li>
            Add <span className="font-medium text-[var(--color-text)]">Bot Token Scopes</span>: chat:write, app_mentions:read,
            im:history, channels:history
          </li>
          <li>Install to your workspace and copy the Bot Token</li>
        </ol>
      }
    />
  );
}

// ── Microsoft 365 card ────────────────────────────────────────────────────

function Microsoft365Card() {
  const queryClient = useQueryClient();
  const { data: status } = useMs365Status();
  const [disconnecting, setDisconnecting] = useState(false);

  // Handle OAuth callback
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const hash = window.location.hash;
    const hashParams = new URLSearchParams(hash.split("?")[1] || "");

    if (hashParams.get("ms365_connected") || params.get("ms365_connected")) {
      queryClient.invalidateQueries({ queryKey: ["ms365-status"] });
      window.history.replaceState({}, "", window.location.pathname + window.location.hash.split("?")[0]);
    }
  }, [queryClient]);

  if (!status) return null;

  async function handleDisconnect() {
    setDisconnecting(true);
    try {
      await disconnectMs365();
      queryClient.invalidateQueries({ queryKey: ["ms365-status"] });
    } finally {
      setDisconnecting(false);
    }
  }

  function handleConnect() {
    window.location.href = `${apiBase()}/api/user/microsoft/auth`;
  }

  return (
    <Card>
      <CardHeader
        icon={<MicrosoftIcon />}
        iconBg=""
        title="Microsoft 365"
        subtitle="Email, calendar, Teams, SharePoint, and OneDrive via Graph API"
        status={status.connected ? "connected" : undefined}
        statusLabel={status.connected ? status.account_email : undefined}
        customIconStyle={{ background: "rgb(0 120 212 / 0.1)", boxShadow: "inset 0 0 0 1px rgb(0 120 212 / 0.2)" }}
      />

      {status.connected ? (
        <div className="flex items-center gap-2 pt-1">
          <button
            onClick={handleConnect}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)]"
          >
            Reconnect
          </button>
          <button
            onClick={handleDisconnect}
            disabled={disconnecting}
            className="rounded-lg border border-red-500/20 bg-red-500/[0.06] px-3 py-1.5 text-[12px] text-red-400/80 transition-colors hover:bg-red-500/[0.12] hover:text-red-400"
          >
            Disconnect
          </button>
        </div>
      ) : (
        <div className="space-y-3 pt-1">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/60 px-4 py-3 text-[12px] text-[var(--color-text-secondary)] space-y-2">
            <p className="font-medium text-[var(--color-text)]">What you get</p>
            <ul className="list-disc list-inside space-y-1 text-[12px]">
              <li>Agents can read and send emails via Outlook</li>
              <li>Manage calendar events and meetings</li>
              <li>Access Teams messages and channels</li>
              <li>Browse SharePoint sites and documents</li>
            </ul>
          </div>
          <button
            onClick={handleConnect}
            className="rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20"
          >
            Connect with Microsoft
          </button>
        </div>
      )}
    </Card>
  );
}

// ── Google Workspace card ─────────────────────────────────────────────────

function GoogleWorkspaceCard() {
  const queryClient = useQueryClient();
  const { data: status } = useGoogleStatus();
  const [disconnecting, setDisconnecting] = useState(false);

  useEffect(() => {
    const hash = window.location.hash;
    const hashParams = new URLSearchParams(hash.split("?")[1] || "");
    if (hashParams.get("google_connected")) {
      queryClient.invalidateQueries({ queryKey: ["google-status"] });
      window.history.replaceState({}, "", window.location.pathname + window.location.hash.split("?")[0]);
    }
  }, [queryClient]);

  if (!status) return null;

  async function handleDisconnect() {
    setDisconnecting(true);
    try {
      await disconnectGoogle();
      queryClient.invalidateQueries({ queryKey: ["google-status"] });
    } finally {
      setDisconnecting(false);
    }
  }

  return (
    <Card>
      <CardHeader
        icon={<GoogleIcon />}
        iconBg=""
        title="Google Workspace"
        subtitle="Gmail, Calendar, Drive, Docs, and Sheets"
        status={status.connected ? "connected" : undefined}
        statusLabel={status.connected ? status.account_email : undefined}
        customIconStyle={{ background: "rgb(66 133 244 / 0.1)", boxShadow: "inset 0 0 0 1px rgb(66 133 244 / 0.2)" }}
      />
      {status.connected ? (
        <div className="flex items-center gap-2 pt-1">
          <button
            onClick={() => { window.location.href = `${apiBase()}/api/user/google/auth`; }}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)]"
          >
            Reconnect
          </button>
          <button
            onClick={handleDisconnect}
            disabled={disconnecting}
            className="rounded-lg border border-red-500/20 bg-red-500/[0.06] px-3 py-1.5 text-[12px] text-red-400/80 transition-colors hover:bg-red-500/[0.12] hover:text-red-400"
          >
            Disconnect
          </button>
        </div>
      ) : (
        <div className="space-y-3 pt-1">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/60 px-4 py-3 text-[12px] text-[var(--color-text-secondary)] space-y-2">
            <p className="font-medium text-[var(--color-text)]">What you get</p>
            <ul className="list-disc list-inside space-y-1 text-[12px]">
              <li>Agents can read and send emails via Gmail</li>
              <li>Manage calendar events</li>
              <li>Access Google Drive, Docs, and Sheets</li>
            </ul>
          </div>
          <button
            onClick={() => { window.location.href = `${apiBase()}/api/user/google/auth`; }}
            className="rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20"
          >
            Connect with Google
          </button>
        </div>
      )}
    </Card>
  );
}

// ── Generic bot connection card ───────────────────────────────────────────

function BotConnectionCard({
  icon,
  iconBg,
  title,
  subtitle,
  setupInstructions,
  tokenFields,
  connected,
  displayName,
  formatStatus = (n) => n,
  connectFn,
  disconnectFn,
}: {
  icon: React.ReactNode;
  iconBg: string;
  title: string;
  subtitle: string;
  setupInstructions: React.ReactNode;
  tokenFields: { placeholder: string }[];
  connected: boolean;
  displayName?: string;
  formatStatus?: (name: string) => string;
  connectFn: (...tokens: string[]) => Promise<unknown>;
  disconnectFn: () => Promise<unknown>;
}) {
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [tokens, setTokens] = useState<string[]>(() => tokenFields.map(() => ""));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const allFilled = tokens.every((t) => t.trim());

  async function handleConnect() {
    setSaving(true);
    setError("");
    try {
      await connectFn(...tokens);
      setTokens(tokenFields.map(() => ""));
      setEditing(false);
      queryClient.invalidateQueries({ queryKey: ["user-settings"] });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to connect");
    } finally {
      setSaving(false);
    }
  }

  async function handleDisconnect() {
    setSaving(true);
    try {
      await disconnectFn();
      queryClient.invalidateQueries({ queryKey: ["user-settings"] });
    } finally {
      setSaving(false);
    }
  }

  function handleCancel() {
    setEditing(false);
    setTokens(tokenFields.map(() => ""));
    setError("");
  }

  const statusLabel = connected && displayName ? formatStatus(displayName) : undefined;

  return (
    <Card>
      <CardHeader
        icon={icon}
        iconBg={iconBg}
        title={title}
        subtitle={subtitle}
        status={connected ? "connected" : undefined}
        statusLabel={statusLabel}
      />

      {connected && !editing ? (
        <div className="flex items-center gap-2 pt-1">
          <button
            onClick={() => setEditing(true)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)]"
          >
            Change Bot
          </button>
          <button
            onClick={handleDisconnect}
            disabled={saving}
            className="rounded-lg border border-red-500/20 bg-red-500/[0.06] px-3 py-1.5 text-[12px] text-red-400/80 transition-colors hover:bg-red-500/[0.12] hover:text-red-400"
          >
            Disconnect
          </button>
        </div>
      ) : (
        <div className="space-y-3 pt-1">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/60 px-4 py-3 text-[12px] text-[var(--color-text-secondary)] space-y-2">
            <p className="font-medium text-[var(--color-text)]">Setup</p>
            {setupInstructions}
          </div>
          {tokenFields.length === 1 ? (
            <div className="flex items-center gap-2">
              <input
                type="password"
                value={tokens[0]}
                onChange={(e) => setTokens([e.target.value])}
                placeholder={tokenFields[0].placeholder}
                className="flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
                autoFocus
              />
              <ConnectButton saving={saving} disabled={!allFilled} onClick={handleConnect} />
              {connected && <CancelButton onClick={handleCancel} />}
            </div>
          ) : (
            <div className="space-y-2">
              {tokenFields.map((field, i) => (
                <input
                  key={i}
                  type="password"
                  value={tokens[i]}
                  onChange={(e) => {
                    const next = [...tokens];
                    next[i] = e.target.value;
                    setTokens(next);
                  }}
                  placeholder={field.placeholder}
                  className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
                  autoFocus={i === 0}
                />
              ))}
              <div className="flex items-center gap-2">
                <ConnectButton saving={saving} disabled={!allFilled} onClick={handleConnect} />
                {connected && <CancelButton onClick={handleCancel} />}
              </div>
            </div>
          )}
          {error && <div className="text-[12px] text-red-400">{error}</div>}
        </div>
      )}
    </Card>
  );
}

function ConnectButton({ saving, disabled, onClick }: { saving: boolean; disabled: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      disabled={saving || disabled}
      className={cn(
        "rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20",
        (saving || disabled) && "opacity-40 cursor-not-allowed",
      )}
    >
      {saving ? "Verifying..." : "Connect"}
    </button>
  );
}

function CancelButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text)]"
    >
      Cancel
    </button>
  );
}

// ── Integration Templates ────────────────────────────────────────────────

interface McpTemplate {
  name: string;
  label: string;
  description: string;
  command: string;
  args: string[];
  credentials: { envVar: string; label: string; placeholder: string }[];
  color: string;
  icon: React.ReactNode;
  setupUrl?: string;
}

const MCP_TEMPLATES: McpTemplate[] = [
  {
    name: "notion",
    label: "Notion",
    description: "Search pages, read content, create and update databases",
    command: "npx",
    args: ["-y", "@notionhq/notion-mcp-server"],
    credentials: [{ envVar: "NOTION_TOKEN", label: "Integration Token", placeholder: "ntn_..." }],
    color: "#FFFFFF",
    icon: <NotionIcon />,
    setupUrl: "https://www.notion.so/my-integrations",
  },
  {
    name: "figma",
    label: "Figma",
    description: "Get design context, components, and layout info from Figma files",
    command: "npx",
    args: ["-y", "figma-developer-mcp", "--stdio"],
    credentials: [{ envVar: "FIGMA_API_KEY", label: "Personal Access Token", placeholder: "figd_..." }],
    color: "#A259FF",
    icon: <FigmaIcon />,
    setupUrl: "https://www.figma.com/developers/api#access-tokens",
  },
  {
    name: "airtable",
    label: "Airtable",
    description: "Read and write records, manage bases and tables",
    command: "npx",
    args: ["-y", "airtable-mcp-server"],
    credentials: [{ envVar: "AIRTABLE_API_KEY", label: "Personal Access Token", placeholder: "pat..." }],
    color: "#18BFFF",
    icon: <AirtableIcon />,
    setupUrl: "https://airtable.com/create/tokens",
  },
  {
    name: "hubspot",
    label: "HubSpot",
    description: "Manage contacts, deals, companies, and tickets",
    command: "npx",
    args: ["-y", "@hubspot/mcp-server"],
    credentials: [{ envVar: "PRIVATE_APP_ACCESS_TOKEN", label: "Private App Token", placeholder: "pat-..." }],
    color: "#FF7A59",
    icon: <HubSpotIcon />,
    setupUrl: "https://developers.hubspot.com/docs/api/private-apps",
  },
  {
    name: "mongodb",
    label: "MongoDB",
    description: "Query collections, manage databases, run aggregations",
    command: "npx",
    args: ["-y", "mongodb-mcp-server"],
    credentials: [{ envVar: "MDB_MCP_CONNECTION_STRING", label: "Connection String", placeholder: "mongodb+srv://user:pass@cluster.mongodb.net/db" }],
    color: "#00ED64",
    icon: <MongoIcon />,
  },
  {
    name: "linear",
    label: "Linear",
    description: "Manage issues, projects, and teams",
    command: "npx",
    args: ["-y", "mcp-server-linear"],
    credentials: [{ envVar: "LINEAR_API_KEY", label: "API Key", placeholder: "lin_api_..." }],
    color: "#5E6AD2",
    icon: <LinearIcon />,
    setupUrl: "https://linear.app/settings/api",
  },
  {
    name: "slack-tools",
    label: "Slack (Agent Tools)",
    description: "Search messages, read channels, manage conversations",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-slack"],
    credentials: [{ envVar: "SLACK_BOT_TOKEN", label: "Bot Token", placeholder: "xoxb-..." }],
    color: "#E01E5A",
    icon: <SlackIcon />,
  },
  {
    name: "github-tools",
    label: "GitHub (Agent Tools)",
    description: "Search code, manage issues and PRs, browse repos",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
    credentials: [{ envVar: "GITHUB_PERSONAL_ACCESS_TOKEN", label: "Personal Access Token", placeholder: "ghp_..." }],
    color: "#e8e0d4",
    icon: <Github className="h-4 w-4 text-[var(--color-text)]" />,
  },
  {
    name: "playwright",
    label: "Playwright",
    description: "Browser automation for web scraping, testing, and interaction",
    command: "npx",
    args: ["-y", "@playwright/mcp@latest"],
    credentials: [],
    color: "#2EAD33",
    icon: <PlaywrightIcon />,
  },
  {
    name: "stripe",
    label: "Stripe",
    description: "Manage customers, subscriptions, invoices, and payments",
    command: "npx",
    args: ["-y", "@stripe/mcp", "--tools=all"],
    credentials: [{ envVar: "STRIPE_SECRET_KEY", label: "Secret Key", placeholder: "sk_..." }],
    color: "#635BFF",
    icon: <StripeIcon />,
  },
  {
    name: "sentry",
    label: "Sentry",
    description: "Browse issues, errors, and performance data",
    command: "npx",
    args: ["-y", "@sentry/mcp-server"],
    credentials: [{ envVar: "SENTRY_AUTH_TOKEN", label: "Auth Token", placeholder: "sntrys_..." }],
    color: "#362D59",
    icon: <SentryIcon />,
  },
  {
    name: "jira",
    label: "Jira",
    description: "Search and manage issues, projects, and boards",
    command: "npx",
    args: ["-y", "@aashari/mcp-server-atlassian-jira"],
    credentials: [
      { envVar: "ATLASSIAN_SITE_NAME", label: "Site Name", placeholder: "yourteam" },
      { envVar: "ATLASSIAN_USER_EMAIL", label: "Email", placeholder: "you@company.com" },
      { envVar: "ATLASSIAN_API_TOKEN", label: "API Token", placeholder: "ATATT3..." },
    ],
    color: "#0052CC",
    icon: <JiraIcon />,
    setupUrl: "https://id.atlassian.com/manage-profile/security/api-tokens",
  },
  {
    name: "n8n",
    label: "n8n",
    description: "Trigger workflows and access 400+ app integrations",
    command: "npx",
    args: ["-y", "@cmwen/min-n8n-mcp"],
    credentials: [
      { envVar: "N8N_BASE_URL", label: "Instance URL", placeholder: "https://n8n.example.com" },
      { envVar: "N8N_API_KEY", label: "API Key", placeholder: "n8n_api_..." },
    ],
    color: "#EA4B71",
    icon: <N8nIcon />,
  },
  {
    name: "supabase",
    label: "Supabase",
    description: "Manage tables, query data, and configure your project",
    command: "npx",
    args: ["-y", "@supabase/mcp-server-supabase"],
    credentials: [{ envVar: "SUPABASE_ACCESS_TOKEN", label: "Access Token", placeholder: "sbp_..." }],
    color: "#3ECF8E",
    icon: <SupabaseIcon />,
  },
  {
    name: "twilio",
    label: "Twilio",
    description: "Send SMS, make calls, and manage communications",
    command: "npx",
    args: ["-y", "@twilio-alpha/mcp"],
    credentials: [
      { envVar: "TWILIO_ACCOUNT_SID", label: "Account SID", placeholder: "AC..." },
      { envVar: "TWILIO_AUTH_TOKEN", label: "Auth Token", placeholder: "" },
    ],
    color: "#F22F46",
    icon: <TwilioIcon />,
  },
  {
    name: "firebase",
    label: "Firebase",
    description: "Manage Firestore, Auth, Storage, and Cloud Functions",
    command: "npx",
    args: ["-y", "@gannonh/firebase-mcp"],
    credentials: [{ envVar: "SERVICE_ACCOUNT_KEY", label: "Service Account JSON", placeholder: '{"type":"service_account",...}' }],
    color: "#FFCA28",
    icon: <FirebaseIcon />,
  },
];

// ── MCP Servers Section ─────────────────────────────────────────────────

function McpServersSection() {
  const queryClient = useQueryClient();
  const { data: servers, isLoading } = useCustomMcpServers();
  const [view, setView] = useState<"list" | "catalog" | "custom">("list");
  const [selectedTemplate, setSelectedTemplate] = useState<McpTemplate | null>(null);

  if (isLoading) return null;

  const configuredNames = new Set(servers?.map((s) => s.name) ?? []);
  const availableTemplates = MCP_TEMPLATES.filter((t) => !configuredNames.has(t.name));

  if (selectedTemplate) {
    return (
      <TemplateSetupCard
        template={selectedTemplate}
        onSave={async (data) => {
          await upsertCustomMcpServer(data);
          queryClient.invalidateQueries({ queryKey: ["custom-mcp-servers"] });
          setSelectedTemplate(null);
          setView("list");
        }}
        onBack={() => setSelectedTemplate(null)}
      />
    );
  }

  if (view === "custom") {
    return (
      <div className="space-y-3">
        <button
          onClick={() => setView("catalog")}
          className="inline-flex items-center gap-1.5 text-[12px] text-[var(--color-text-secondary)] hover:text-[var(--color-text)] transition-colors"
        >
          <ChevronLeft className="h-3.5 w-3.5" />
          Back
        </button>
        <CustomMcpForm
          onSave={async (data) => {
            await upsertCustomMcpServer(data);
            queryClient.invalidateQueries({ queryKey: ["custom-mcp-servers"] });
            setView("list");
          }}
          onCancel={() => setView("catalog")}
        />
      </div>
    );
  }

  if (view === "catalog") {
    return (
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <button
            onClick={() => setView("list")}
            className="inline-flex items-center gap-1.5 text-[12px] text-[var(--color-text-secondary)] hover:text-[var(--color-text)] transition-colors"
          >
            <ChevronLeft className="h-3.5 w-3.5" />
            Back
          </button>
          <button
            onClick={() => setView("custom")}
            className="inline-flex items-center gap-1.5 text-[12px] text-[var(--color-text-secondary)] hover:text-[var(--color-text)] transition-colors"
          >
            <Settings2 className="h-3.5 w-3.5" />
            Custom Server
          </button>
        </div>
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
          {availableTemplates.map((template) => (
            <button
              key={template.name}
              onClick={() => setSelectedTemplate(template)}
              className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-3.5 text-left transition-colors hover:border-[#3a3530] hover:bg-[#1a1815]"
            >
              <div
                className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ring-1"
                style={{ background: `${template.color}10`, borderColor: `${template.color}30` }}
              >
                {template.icon}
              </div>
              <div className="min-w-0">
                <div className="text-[13px] font-medium text-[var(--color-text)]">{template.label}</div>
                <div className="text-[11px] text-[var(--color-text-tertiary)] leading-tight truncate">{template.description}</div>
              </div>
            </button>
          ))}
          {availableTemplates.length === 0 && (
            <div className="col-span-2 py-6 text-center text-[13px] text-[var(--color-text-tertiary)]">
              All available integrations are already configured.
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-[14px] font-semibold text-[var(--color-text)]">Integrations</div>
          <div className="text-[12px] text-[var(--color-text-tertiary)]">Tools your agents can use during tasks and chat</div>
        </div>
        <button
          onClick={() => setView("catalog")}
          className="inline-flex items-center gap-1.5 rounded-lg bg-amber-500/15 px-3 py-1.5 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20"
        >
          <Plus className="h-3.5 w-3.5" />
          Add
        </button>
      </div>

      {servers && servers.length > 0 ? (
        <div className="space-y-2">
          {servers.map((server) => (
            <McpServerCard key={server.id} server={server} />
          ))}
        </div>
      ) : (
        <div className="rounded-2xl border border-dashed border-[var(--color-border)] px-5 py-8 text-center">
          <div className="text-[13px] text-[var(--color-text-tertiary)]">
            No integrations configured yet.
          </div>
          <button
            onClick={() => setView("catalog")}
            className="mt-3 inline-flex items-center gap-1.5 rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20"
          >
            Browse integrations
          </button>
        </div>
      )}
    </div>
  );
}

function TemplateSetupCard({
  template,
  onSave,
  onBack,
}: {
  template: McpTemplate;
  onSave: (data: McpFormData) => Promise<void>;
  onBack: () => void;
}) {
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const allFilled = template.credentials.every((c) => (values[c.envVar] ?? "").trim());

  async function handleConnect() {
    setSaving(true);
    setError("");
    try {
      const env: Record<string, string> = {};
      for (const cred of template.credentials) {
        env[cred.envVar] = values[cred.envVar] ?? "";
      }
      await onSave({
        name: template.name,
        label: template.label,
        command: template.command,
        args: template.args,
        env,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to connect");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-3">
      <button
        onClick={onBack}
        className="inline-flex items-center gap-1.5 text-[12px] text-[var(--color-text-secondary)] hover:text-[var(--color-text)] transition-colors"
      >
        <ChevronLeft className="h-3.5 w-3.5" />
        Back
      </button>
      <Card>
        <CardHeader
          icon={template.icon}
          iconBg=""
          title={template.label}
          subtitle={template.description}
          customIconStyle={{ background: `${template.color}10`, boxShadow: `inset 0 0 0 1px ${template.color}30` }}
        />
        <div className="space-y-3 pt-1">
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-card)]/60 px-4 py-3 text-[12px] text-[var(--color-text-secondary)] space-y-2">
            <p className="font-medium text-[var(--color-text)]">Setup</p>
            <ol className="list-decimal list-inside space-y-1.5 text-[12px]">
              {template.setupUrl ? (
                <li>
                  Go to{" "}
                  <span className="font-medium text-[var(--color-text)]">{template.label}</span> and create an API token
                </li>
              ) : (
                <li>Create an API token in your <span className="font-medium text-[var(--color-text)]">{template.label}</span> settings</li>
              )}
              <li>Paste your credentials below</li>
            </ol>
          </div>
          <div className="space-y-2">
            {template.credentials.map((cred) => (
              <div key={cred.envVar} className="space-y-1">
                <label className="text-[11px] font-medium text-[var(--color-text-secondary)]">{cred.label}</label>
                <input
                  type={cred.envVar.toLowerCase().includes("url") || cred.envVar.toLowerCase().includes("email") ? "text" : "password"}
                  value={values[cred.envVar] ?? ""}
                  onChange={(e) => setValues({ ...values, [cred.envVar]: e.target.value })}
                  placeholder={cred.placeholder}
                  className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
                />
              </div>
            ))}
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={handleConnect}
              disabled={saving || !allFilled}
              className={cn(
                "rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20",
                (saving || !allFilled) && "opacity-40 cursor-not-allowed",
              )}
            >
              {saving ? "Connecting..." : "Connect"}
            </button>
          </div>
          {error && <div className="text-[12px] text-red-400">{error}</div>}
        </div>
      </Card>
    </div>
  );
}

function McpServerCard({ server }: { server: CustomMcpServer }) {
  const queryClient = useQueryClient();
  const [toggling, setToggling] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const template = MCP_TEMPLATES.find((t) => t.name === server.name);

  async function handleToggle() {
    setToggling(true);
    try {
      await toggleCustomMcpServer(server.id, !server.enabled);
      queryClient.invalidateQueries({ queryKey: ["custom-mcp-servers"] });
    } finally {
      setToggling(false);
    }
  }

  async function handleDelete() {
    setDeleting(true);
    try {
      await deleteCustomMcpServer(server.id);
      queryClient.invalidateQueries({ queryKey: ["custom-mcp-servers"] });
    } finally {
      setDeleting(false);
    }
  }

  return (
    <div className="flex items-center justify-between rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-4 py-3">
      <div className="flex items-center gap-3">
        <div
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ring-1"
          style={template
            ? { background: `${template.color}10`, borderColor: `${template.color}30` }
            : { background: "rgb(139 92 246 / 0.1)", borderColor: "rgb(139 92 246 / 0.2)" }
          }
        >
          {template?.icon ?? <McpIcon />}
        </div>
        <div>
          <div className="flex items-center gap-2">
            <span className="text-[13px] font-medium text-[var(--color-text)]">{server.label || server.name}</span>
            {server.enabled ? (
              <span className="inline-flex items-center gap-1 rounded-full border border-emerald-500/25 bg-emerald-500/[0.08] px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                <span className="h-1 w-1 rounded-full bg-emerald-400" />
                Connected
              </span>
            ) : (
              <span className="inline-flex items-center gap-1 rounded-full border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-0.5 text-[10px] font-medium text-[var(--color-text-tertiary)]">
                Disabled
              </span>
            )}
          </div>
          <div className="text-[11px] text-[var(--color-text-tertiary)]">
            {template?.description ?? `${server.command} ${JSON.parse(server.args_json || "[]").join(" ")}`}
          </div>
        </div>
      </div>
      <div className="flex items-center gap-1.5">
        <button
          onClick={handleToggle}
          disabled={toggling}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)]"
          title={server.enabled ? "Disable" : "Enable"}
        >
          <Power className="h-3.5 w-3.5" />
        </button>
        <button
          onClick={handleDelete}
          disabled={deleting}
          className="rounded-lg border border-red-500/20 bg-red-500/[0.06] p-1.5 text-red-400/80 transition-colors hover:bg-red-500/[0.12] hover:text-red-400"
          title="Remove"
        >
          <Trash2 className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}

interface McpFormData {
  name: string;
  label?: string;
  command: string;
  args?: string[];
  env?: Record<string, string>;
  enabled?: boolean;
}

function CustomMcpForm({
  onSave,
  onCancel,
}: {
  onSave: (data: McpFormData) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [label, setLabel] = useState("");
  const [command, setCommand] = useState("npx");
  const [argsStr, setArgsStr] = useState("-y ");
  const [envPairs, setEnvPairs] = useState<{ key: string; value: string }[]>([{ key: "", value: "" }]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  async function handleSave() {
    setSaving(true);
    setError("");
    try {
      const args = argsStr.trim() ? argsStr.trim().split(/\s+/) : [];
      const env: Record<string, string> = {};
      for (const pair of envPairs) {
        const k = pair.key.trim();
        if (k) env[k] = pair.value;
      }
      await onSave({
        name: name.trim(),
        label: label.trim() || undefined,
        command: command.trim(),
        args,
        env: Object.keys(env).length > 0 ? env : undefined,
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to save");
    } finally {
      setSaving(false);
    }
  }

  const isValid = name.trim() && command.trim() && /^[a-zA-Z0-9_-]+$/.test(name.trim());

  return (
    <div className="rounded-2xl border border-amber-500/20 bg-[var(--color-bg-secondary)] p-5 space-y-4">
      <div className="text-[13px] font-medium text-[var(--color-text)]">Custom MCP Server</div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <label className="text-[11px] font-medium text-[var(--color-text-secondary)]">Name</label>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="my-server"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
          />
        </div>
        <div className="space-y-1.5">
          <label className="text-[11px] font-medium text-[var(--color-text-secondary)]">Display Name</label>
          <input
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="My Server"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
          />
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-1.5">
          <label className="text-[11px] font-medium text-[var(--color-text-secondary)]">Command</label>
          <input
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="npx"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] font-mono outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
          />
        </div>
        <div className="col-span-2 space-y-1.5">
          <label className="text-[11px] font-medium text-[var(--color-text-secondary)]">Arguments</label>
          <input
            value={argsStr}
            onChange={(e) => setArgsStr(e.target.value)}
            placeholder="-y @some/mcp-server"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] font-mono outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
          />
        </div>
      </div>
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="text-[11px] font-medium text-[var(--color-text-secondary)]">Environment Variables</label>
          <button
            onClick={() => setEnvPairs([...envPairs, { key: "", value: "" }])}
            className="text-[11px] text-amber-400/70 hover:text-amber-300 transition-colors"
          >
            + Add
          </button>
        </div>
        {envPairs.map((pair, i) => (
          <div key={i} className="flex items-center gap-2">
            <input
              value={pair.key}
              onChange={(e) => setEnvPairs(envPairs.map((p, j) => (j === i ? { ...p, key: e.target.value } : p)))}
              placeholder="KEY"
              className="w-2/5 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text)] font-mono outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
            />
            <input
              type="password"
              value={pair.value}
              onChange={(e) => setEnvPairs(envPairs.map((p, j) => (j === i ? { ...p, value: e.target.value } : p)))}
              placeholder="value"
              className="flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text)] font-mono outline-none transition-colors focus:border-amber-500/30 placeholder:text-[var(--color-text-faint)]"
            />
            {envPairs.length > 1 && (
              <button
                onClick={() => setEnvPairs(envPairs.filter((_, j) => j !== i))}
                className="shrink-0 rounded-lg p-1.5 text-[var(--color-text-tertiary)] hover:text-red-400 transition-colors"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        ))}
      </div>
      <div className="flex items-center gap-2 pt-1">
        <button
          onClick={handleSave}
          disabled={saving || !isValid}
          className={cn(
            "rounded-lg bg-amber-500/15 px-4 py-2 text-[12px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20",
            (saving || !isValid) && "opacity-40 cursor-not-allowed",
          )}
        >
          {saving ? "Saving..." : "Add Server"}
        </button>
        <button
          onClick={onCancel}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text)]"
        >
          Cancel
        </button>
      </div>
      {error && <div className="text-[12px] text-red-400">{error}</div>}
    </div>
  );
}

function McpIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="h-4 w-4 text-violet-400">
      <path d="M12 2L2 7l10 5 10-5-10-5z" />
      <path d="M2 17l10 5 10-5" />
      <path d="M2 12l10 5 10-5" />
    </svg>
  );
}

function AirtableIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#18BFFF"><path d="M11.5 3.02L3.63 6.2a.86.86 0 000 1.58l7.89 3.2a2.29 2.29 0 001.72 0l7.89-3.2a.86.86 0 000-1.58L13.22 3.02a2.29 2.29 0 00-1.72 0z" /><path d="M12.6 12.15v8.27a.43.43 0 00.6.4l8.54-3.47a.86.86 0 00.52-.79v-8.27a.43.43 0 00-.6-.4L13.12 11.36a.86.86 0 00-.52.79z" opacity="0.7" /><path d="M11.14 12.3L3.47 9.2a.43.43 0 00-.63.38v8.12a.86.86 0 00.48.77l7.67 3.83a.43.43 0 00.63-.38v-8.85a.86.86 0 00-.48-.77z" opacity="0.5" /></svg>;
}

function NotionIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#e8e0d4"><path d="M4.46 4.18l9.68-.71c1.19-.1 1.49-.04 2.24.53l3.07 2.14c.52.38.68.48.68.89v12.27c0 .71-.26 1.12-1.18 1.18l-11.26.66c-.68.04-.97-.07-1.32-.49l-2.1-2.72c-.38-.52-.52-.89-.52-1.34V5.34c0-.57.26-1.08 1-1.16h-.29zm10.09 2.5v9.1c0 .49-.2.72-.63.75l-7.8.46c-.43.02-.63-.21-.63-.68V6.97c0-.47.21-.73.63-.75l7.8-.28c.46-.02.63.22.63.74z" /></svg>;
}

function FigmaIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#A259FF"><path d="M8 24c2.2 0 4-1.8 4-4v-4H8c-2.2 0-4 1.8-4 4s1.8 4 4 4z" /><path d="M4 12c0-2.2 1.8-4 4-4h4v8H8c-2.2 0-4-1.8-4-4z" opacity="0.8" /><path d="M4 4c0-2.2 1.8-4 4-4h4v8H8C5.8 8 4 6.2 4 4z" opacity="0.6" /><path d="M12 0h4c2.2 0 4 1.8 4 4s-1.8 4-4 4h-4V0z" opacity="0.4" /><circle cx="16" cy="12" r="4" opacity="0.6" /></svg>;
}

function HubSpotIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#FF7A59"><circle cx="12" cy="12" r="3" /><circle cx="18" cy="15" r="2" /><circle cx="6" cy="15" r="2" /><circle cx="12" cy="5" r="2" /><path d="M12 8v1m4.5 4.5l-1.5-1m-6 0l-1.5 1" stroke="#FF7A59" strokeWidth="1.5" fill="none" /></svg>;
}

function MongoIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#00ED64"><path d="M12.5 2.1c-.3-.5-.5-.9-.5-1.1 0 0-.2.4-.5 1.1C9.4 6.7 5 9.2 5 14c0 3.9 3.1 7 7 7s7-3.1 7-7c0-4.8-4.4-7.3-6.5-11.9zM12 19c-.6 0-1-.2-1-.5v-4c0-.3.4-.5 1-.5s1 .2 1 .5v4c0 .3-.4.5-1 .5z" /></svg>;
}

function LinearIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#5E6AD2"><path d="M3.36 7.56a10.2 10.2 0 0013.08 13.08L3.36 7.56zm.91-1.95l14.12 14.12A10.2 10.2 0 004.27 5.61zm2.12-1.7L19.7 17.22a10.2 10.2 0 00-13.31-13.3z" /></svg>;
}

function GoogleIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4.5 w-4.5">
      <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 01-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z" fill="#4285F4" />
      <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853" />
      <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05" />
      <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335" />
    </svg>
  );
}

function PlaywrightIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#2EAD33"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 14.5v-9l7 4.5-7 4.5z" /></svg>;
}

function StripeIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#635BFF"><path d="M13.98 11.02c0-1.07-.45-1.47-1.32-1.47-.87 0-1.44.4-1.44 1.47v1.96c0 1.07.57 1.47 1.44 1.47.87 0 1.32-.4 1.32-1.47v-1.96zM24 4v16a4 4 0 01-4 4H4a4 4 0 01-4-4V4a4 4 0 014-4h16a4 4 0 014 4zm-7.55 7.02c0-2.35-1.18-3.62-3.22-3.62-1.15 0-1.97.42-2.6 1.16V7.5H8.5v9h2.13v-.92c.63.7 1.41 1.07 2.56 1.07 2.08 0 3.26-1.32 3.26-3.67v-1.96z" /></svg>;
}

function SentryIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#362D59"><path d="M13.93 2.18a1.86 1.86 0 00-3.22 0l-2.4 4.15a10.64 10.64 0 015.17 8.94h-2.31a8.33 8.33 0 00-4.05-7.01l-2.92 5.05A4.54 4.54 0 016.32 17h-2.3a1.86 1.86 0 01-1.61-2.79L7.56 5.2l.48-.83a1.86 1.86 0 013.22 0L13.93 9l2.67-4.61.48-.83 2.67 4.62 2.67 4.61A1.86 1.86 0 0120.81 15h-1.15" /></svg>;
}

function JiraIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#0052CC"><path d="M11.53 2c0 4.97-4.03 9-9 9a.47.47 0 000 .94c4.97 0 9 4.03 9 9a.47.47 0 00.94 0c0-4.97 4.03-9 9-9a.47.47 0 000-.94c-4.97 0-9-4.03-9-9a.47.47 0 00-.94 0z" /></svg>;
}

function N8nIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#EA4B71"><circle cx="6" cy="12" r="3" /><circle cx="18" cy="7" r="3" /><circle cx="18" cy="17" r="3" /><path d="M9 12h3m0 0l3-5m-3 5l3 5" stroke="#EA4B71" strokeWidth="1.5" fill="none" /></svg>;
}

function SupabaseIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#3ECF8E"><path d="M13.36 21.89c-.53.67-1.6.13-1.56-.78l.57-12.11H21c1.26 0 1.95 1.47 1.15 2.44L13.36 21.9z" opacity="0.7" /><path d="M10.64 2.11c.53-.67 1.6-.13 1.56.78L11.63 15H3c-1.26 0-1.95-1.47-1.15-2.44L10.64 2.1z" /></svg>;
}

function TwilioIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4" fill="#F22F46"><path d="M12 0C5.37 0 0 5.37 0 12s5.37 12 12 12 12-5.37 12-12S18.63 0 12 0zm0 20.4c-4.64 0-8.4-3.76-8.4-8.4S7.36 3.6 12 3.6s8.4 3.76 8.4 8.4-3.76 8.4-8.4 8.4zm3.54-11.94a2.1 2.1 0 110 4.2 2.1 2.1 0 010-4.2zm0 5.28a2.1 2.1 0 110 4.2 2.1 2.1 0 010-4.2zm-7.08-5.28a2.1 2.1 0 110 4.2 2.1 2.1 0 010-4.2zm0 5.28a2.1 2.1 0 110 4.2 2.1 2.1 0 010-4.2z" /></svg>;
}

function FirebaseIcon() {
  return <svg viewBox="0 0 24 24" className="h-4 w-4"><path d="M4.53 20.26L6.23 3.1a.5.5 0 01.93-.18L9 7.1l1.61-3.07a.5.5 0 01.89 0L19.47 20.26" fill="#FFA000" /><path d="M4.53 20.26L9 7.1l2.52 4.2z" fill="#F57F17" opacity="0.7" /><path d="M4.53 20.26h14.94L11.52 11.3z" fill="#FFCA28" /></svg>;
}

// ── Shared UI ─────────────────────────────────────────────────────────────

function Card({ children }: { children: React.ReactNode }) {
  return <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-5 space-y-3">{children}</div>;
}

function CardHeader({
  icon,
  iconBg,
  title,
  subtitle,
  status,
  statusLabel,
  customIconStyle,
}: {
  icon: React.ReactNode;
  iconBg: string;
  title: string;
  subtitle: string;
  status?: "connected";
  statusLabel?: string;
  customIconStyle?: React.CSSProperties;
}) {
  return (
    <div className="flex items-start gap-3.5">
      <div className={cn("flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ring-1", iconBg)} style={customIconStyle}>{icon}</div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2.5">
          <span className="text-[14px] font-semibold text-[var(--color-text)]">{title}</span>
          {status && (
            <span className="inline-flex items-center gap-1.5 rounded-full border border-emerald-500/25 bg-emerald-500/[0.08] px-2.5 py-0.5 text-[11px] font-medium text-emerald-400">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
              {statusLabel ?? "Connected"}
            </span>
          )}
        </div>
        <p className="mt-0.5 text-[12px] leading-relaxed text-[var(--color-text-tertiary)]">{subtitle}</p>
      </div>
    </div>
  );
}

function TelegramIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className="h-4.5 w-4.5 text-[#229ED9]">
      <path d="M11.944 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0a12 12 0 0 0-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 0 1 .171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.48.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z" />
    </svg>
  );
}

function DiscordIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className="h-4.5 w-4.5 text-[#5865F2]">
      <path d="M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189z" />
    </svg>
  );
}

function WhatsAppIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className="h-4.5 w-4.5 text-[#25D366]">
      <path d="M17.472 14.382c-.297-.149-1.758-.867-2.03-.967-.273-.099-.471-.148-.67.15-.197.297-.767.966-.94 1.164-.173.199-.347.223-.644.075-.297-.15-1.255-.463-2.39-1.475-.883-.788-1.48-1.761-1.653-2.059-.173-.297-.018-.458.13-.606.134-.133.298-.347.446-.52.149-.174.198-.298.298-.497.099-.198.05-.371-.025-.52-.075-.149-.669-1.612-.916-2.207-.242-.579-.487-.5-.669-.51-.173-.008-.371-.01-.57-.01-.198 0-.52.074-.792.372-.272.297-1.04 1.016-1.04 2.479 0 1.462 1.065 2.875 1.213 3.074.149.198 2.096 3.2 5.077 4.487.709.306 1.262.489 1.694.625.712.227 1.36.195 1.871.118.571-.085 1.758-.719 2.006-1.413.248-.694.248-1.289.173-1.413-.074-.124-.272-.198-.57-.347m-5.421 7.403h-.004a9.87 9.87 0 01-5.031-1.378l-.361-.214-3.741.982.998-3.648-.235-.374a9.86 9.86 0 01-1.51-5.26c.001-5.45 4.436-9.884 9.888-9.884 2.64 0 5.122 1.03 6.988 2.898a9.825 9.825 0 012.893 6.994c-.003 5.45-4.437 9.884-9.885 9.884m8.413-18.297A11.815 11.815 0 0012.05 0C5.495 0 .16 5.335.157 11.892c0 2.096.547 4.142 1.588 5.945L.057 24l6.305-1.654a11.882 11.882 0 005.683 1.448h.005c6.554 0 11.89-5.335 11.893-11.893a11.821 11.821 0 00-3.48-8.413z" />
    </svg>
  );
}

function SlackIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className="h-4.5 w-4.5">
      <path
        d="M5.042 15.165a2.528 2.528 0 0 1-2.52 2.523A2.528 2.528 0 0 1 0 15.165a2.527 2.527 0 0 1 2.522-2.52h2.52v2.52zm1.271 0a2.527 2.527 0 0 1 2.521-2.52 2.527 2.527 0 0 1 2.521 2.52v6.313A2.528 2.528 0 0 1 8.834 24a2.528 2.528 0 0 1-2.521-2.522v-6.313zM8.834 5.042a2.528 2.528 0 0 1-2.521-2.52A2.528 2.528 0 0 1 8.834 0a2.528 2.528 0 0 1 2.521 2.522v2.52H8.834zm0 1.271a2.528 2.528 0 0 1 2.521 2.521 2.528 2.528 0 0 1-2.521 2.521H2.522A2.528 2.528 0 0 1 0 8.834a2.528 2.528 0 0 1 2.522-2.521h6.312zm10.122 2.521a2.528 2.528 0 0 1 2.522-2.521A2.528 2.528 0 0 1 24 8.834a2.528 2.528 0 0 1-2.522 2.521h-2.522V8.834zm-1.268 0a2.528 2.528 0 0 1-2.523 2.521 2.527 2.527 0 0 1-2.52-2.521V2.522A2.527 2.527 0 0 1 15.165 0a2.528 2.528 0 0 1 2.523 2.522v6.312zm-2.523 10.122a2.528 2.528 0 0 1 2.523 2.522A2.528 2.528 0 0 1 15.165 24a2.527 2.527 0 0 1-2.52-2.522v-2.522h2.52zm0-1.268a2.527 2.527 0 0 1-2.52-2.523 2.526 2.526 0 0 1 2.52-2.52h6.313A2.527 2.527 0 0 1 24 15.165a2.528 2.528 0 0 1-2.522 2.523h-6.313z"
        fill="#E01E5A"
      />
    </svg>
  );
}

function MicrosoftIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4.5 w-4.5">
      <rect x="1" y="1" width="10" height="10" fill="#F25022" />
      <rect x="13" y="1" width="10" height="10" fill="#7FBA00" />
      <rect x="1" y="13" width="10" height="10" fill="#00A4EF" />
      <rect x="13" y="13" width="10" height="10" fill="#FFB900" />
    </svg>
  );
}
