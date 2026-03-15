import { GitMerge } from "lucide-react";
import { useQueue, useStatus } from "@/lib/api";
import { repoName } from "@/lib/types";
import { StatusBadge } from "./status-badge";

interface QueuePanelProps {
  repoFilter: string | null;
}

export function QueuePanel({ repoFilter }: QueuePanelProps) {
  const { data: queue } = useQueue();
  const { data: status } = useStatus();
  const multiRepo = (status?.watched_repos?.length ?? 0) > 1;

  const filtered = repoFilter ? queue?.filter((e) => e.repo_path === repoFilter) : queue;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-14 shrink-0 items-center justify-between border-b border-[var(--color-border)] px-5">
        <span className="text-[14px] font-semibold text-[var(--color-text)]">Integration Queue</span>
        <span className="rounded-full bg-amber-500/[0.06] px-2.5 py-0.5 text-[11px] tabular-nums text-[var(--color-text-secondary)]">
          {filtered?.length ?? 0}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto overscroll-contain">
        <div className="p-2">
          {filtered?.map((e) => (
            <div
              key={e.id}
              className="flex items-center gap-3 rounded-xl px-4 py-3 min-h-[44px] text-[13px] transition-colors hover:bg-[var(--color-card)] overflow-x-auto"
            >
              <StatusBadge status={e.status} />
              {multiRepo && !repoFilter && e.repo_path && (
                <span className="shrink-0 rounded-lg bg-amber-500/[0.04] px-2 py-0.5 text-[11px] font-medium text-[var(--color-text-secondary)]">
                  {repoName(e.repo_path)}
                </span>
              )}
              <span className="flex-1 truncate font-mono text-[13px] text-[var(--color-text)]">{e.branch}</span>
              <span className="shrink-0 font-mono text-[11px] text-[var(--color-text-tertiary)]">#{e.task_id}</span>
            </div>
          ))}
          {!filtered?.length && (
            <div className="flex flex-col items-center justify-center py-20 text-center">
              <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-amber-500/[0.04] ring-1 ring-amber-900/20">
                <GitMerge className="h-6 w-6 text-[var(--color-text-tertiary)]" strokeWidth={1.5} />
              </div>
              <p className="text-[14px] text-[var(--color-text-secondary)]">Queue is empty</p>
              <p className="mt-1 text-[12px] text-[var(--color-text-tertiary)]">Completed tasks will appear here for integration</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
