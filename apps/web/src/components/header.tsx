import { Menu } from "lucide-react";
import { useState } from "react";
import { getSelectedWorkspaceId, switchWorkspace, useStatus, useWorkspaces } from "@/lib/api";
import { useDomain } from "@/lib/domain";
import { repoName } from "@/lib/types";
import { useUIMode } from "@/lib/ui-mode";
import { BorgLogo, PRODUCT_WORD } from "./borg-logo";
import { TaskCreator } from "./task-creator";

function formatUptime(seconds: number) {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

type View =
  | "tasks"
  | "projects"
  | "connections"
  | "creator"
  | "auto-tasks"
  | "proposals"
  | "logs"
  | "queue"
  | "status"
  | "chat"
  | "knowledge"
  | "settings"
  | "theme";

const VIEW_TITLES: Record<View, string> = {
  tasks: "Pipeline Tasks",
  projects: "Workspace",
  connections: "Connections",
  creator: "Pipelines",
  "auto-tasks": "Auto Tasks",
  proposals: "Proposals",
  logs: "System Logs",
  queue: "Integration Queue",
  status: "Status",
  chat: "Chat",
  knowledge: "Knowledge Base",
  settings: "Settings",
  theme: "Theme",
};

export function Header({
  connected,
  mobile,
  view,
  repoFilter,
  onRepoFilterChange,
  onMenuToggle,
}: {
  connected: boolean;
  mobile?: boolean;
  view?: View;
  repoFilter?: string | null;
  onRepoFilterChange?: (repo: string | null) => void;
  onMenuToggle?: () => void;
}) {
  const { data: status } = useStatus();
  const { data: workspaceData } = useWorkspaces();
  const { mode: uiMode } = useUIMode();
  const domain = useDomain();
  const isMinimal = uiMode === "minimal";
  const [switchingWorkspace, setSwitchingWorkspace] = useState(false);
  const selectedWorkspaceId = getSelectedWorkspaceId() ?? workspaceData?.default_workspace_id ?? 0;
  const workspaces = workspaceData?.workspaces ?? [];

  if (mobile) {
    return (
      <header className="flex h-14 shrink-0 items-center gap-2 border-b border-[var(--color-border)] bg-[var(--color-bg)] px-2">
        {onMenuToggle && (
          <button
            onClick={onMenuToggle}
            className="flex h-[44px] w-[44px] items-center justify-center rounded-xl text-[var(--color-text-tertiary)] hover:text-[var(--color-text)] transition-colors"
            aria-label="Toggle sidebar"
          >
            <Menu className="h-5 w-5" />
          </button>
        )}
        <div className="flex items-center gap-2.5">
          <div className={`borg-logo h-6 w-6 ${domain.accentBg}`}>
            <BorgLogo size="mobile" />
            <div className="borg-logo-ghost grid grid-cols-2 grid-rows-2" aria-hidden>
              {PRODUCT_WORD.split("").map((c, i) => (
                <span key={i} className="flex items-center justify-center text-[16px]">
                  {c}
                </span>
              ))}
            </div>
          </div>
          <span className="text-[14px] font-semibold tracking-tight text-[var(--color-text)]">Borg</span>
        </div>

        <div className="ml-auto flex items-center gap-3">
          <TaskCreator />
          {status?.continuous_mode && (
            <span className="flex items-center gap-1.5 text-[12px] text-[var(--color-text-secondary)]">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
              Cont
            </span>
          )}
          <span className="text-[12px] tabular-nums text-[var(--color-text-tertiary)]">
            {status ? formatUptime(status.uptime_s) : "--"}
          </span>
          <span
            className={`h-2 w-2 rounded-full ${connected ? "bg-emerald-500 shadow-[0_0_6px_rgba(200,160,80,0.3)]" : "bg-red-500"}`}
          />
        </div>
      </header>
    );
  }

  const repos = status?.watched_repos ?? [];
  const multiRepo = repos.length > 1;

  return (
    <header className="flex h-14 shrink-0 items-center gap-2 md:gap-4 border-b border-[var(--color-border)] px-3 md:px-6">
      {onMenuToggle && (
        <button
          onClick={onMenuToggle}
          className="flex h-[44px] w-[44px] items-center justify-center rounded-xl text-[var(--color-text-tertiary)] hover:text-[var(--color-text)] transition-colors lg:hidden"
          aria-label="Toggle sidebar"
        >
          <Menu className="h-5 w-5" />
        </button>
      )}
      <h1 className="text-[15px] font-semibold text-[var(--color-text)] truncate">{VIEW_TITLES[view ?? "tasks"]}</h1>

      {!isMinimal && (
        <>
          <div className="h-4 w-px bg-[var(--color-border)] hidden lg:block" />
          <div className="hidden lg:flex items-center gap-4 text-[12px] text-[var(--color-text-tertiary)]">
            {status?.continuous_mode && (
              <span className="flex items-center gap-1.5 text-[var(--color-text-secondary)]">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                Continuous
              </span>
            )}
            <span>
              Up <span className="text-[var(--color-text-secondary)]">{status ? formatUptime(status.uptime_s) : "--"}</span>
            </span>
            <span>
              Model <span className="text-[var(--color-text-secondary)]">{status?.model ?? "--"}</span>
            </span>
            <span className="h-3 w-px bg-[var(--color-border)]" />
            <span>
              Active <span className="text-blue-400 tabular-nums">{status?.active_tasks ?? 0}</span>
            </span>
            <span>
              Merged <span className="text-emerald-400 tabular-nums">{status?.merged_tasks ?? 0}</span>
            </span>
            <span>
              AI Calls <span className="text-cyan-400 tabular-nums">{status?.ai_requests ?? 0}</span>
            </span>
            <span>
              Failed <span className="text-red-400 tabular-nums">{status?.failed_tasks ?? 0}</span>
            </span>
            {status?.version && (
              <span className="rounded-full bg-amber-500/[0.06] px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-text-tertiary)]">
                {status.version}
              </span>
            )}
          </div>
        </>
      )}

      <div className="ml-auto flex items-center gap-2 md:gap-4">
        {workspaces.length > 0 && (
          <select
            value={selectedWorkspaceId || ""}
            disabled={switchingWorkspace}
            onChange={async (e) => {
              const nextId = Number(e.target.value);
              if (!nextId || nextId === selectedWorkspaceId) return;
              try {
                setSwitchingWorkspace(true);
                await switchWorkspace(nextId);
                window.location.reload();
              } finally {
                setSwitchingWorkspace(false);
              }
            }}
            className="h-8 min-h-[44px] md:min-h-0 md:h-7 max-w-[180px] md:max-w-[220px] shrink-0 rounded-lg border border-[var(--color-border)] bg-amber-500/[0.03] px-2 text-[13px] md:text-[12px] text-[var(--color-text-secondary)] outline-none"
          >
            {workspaces.map((workspace) => (
              <option key={workspace.workspace_id} value={workspace.workspace_id}>
                {workspace.name}
              </option>
            ))}
          </select>
        )}
        {multiRepo && onRepoFilterChange && (
          <select
            value={repoFilter ?? ""}
            onChange={(e) => onRepoFilterChange(e.target.value || null)}
            className="hidden md:block h-7 shrink-0 rounded-lg border border-[var(--color-border)] bg-amber-500/[0.03] px-2 text-[12px] text-[var(--color-text-secondary)] outline-none"
          >
            <option value="">All repos</option>
            {repos.map((r) => (
              <option key={r.path} value={r.path}>
                {repoName(r.path)}
                {!r.auto_merge ? " (manual)" : ""}
              </option>
            ))}
          </select>
        )}
        <TaskCreator />
      </div>
    </header>
  );
}
