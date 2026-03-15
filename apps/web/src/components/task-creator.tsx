import { useQueryClient } from "@tanstack/react-query";
import { ChevronRight, Plus, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { createTask, useModes, useStatus } from "@/lib/api";
import { useDashboardMode } from "@/lib/dashboard-mode";
import { type PipelineMode, repoName } from "@/lib/types";

const LEGAL_TASK_TYPES = [
  { value: "", label: "General legal task" },
  { value: "research_memo", label: "Research memo" },
  { value: "contract_analysis", label: "Contract analysis" },
  { value: "motion_draft", label: "Motion draft" },
  { value: "brief", label: "Brief" },
];

interface TaskCreatorProps {
  defaultMode?: string;
  hideModePicker?: boolean;
  projectId?: number;
  projectMode?: string;
  buttonLabel?: string;
}

function isLegalModeName(mode?: string): boolean {
  return mode === "lawborg" || mode === "legal";
}

export function TaskCreator({
  defaultMode,
  hideModePicker = false,
  projectId,
  projectMode,
  buttonLabel = "New Task",
}: TaskCreatorProps = {}) {
  const { isLegal } = useDashboardMode();
  const initialMode = projectMode || defaultMode || (isLegal ? "lawborg" : "sweborg");
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [mode, setMode] = useState(initialMode);
  const [taskType, setTaskType] = useState("");
  const [requiresExhaustiveCorpusReview, setRequiresExhaustiveCorpusReview] = useState(false);
  const [repoPath, setRepoPath] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");
  const queryClient = useQueryClient();
  const { data: modes } = useModes();
  const { data: status } = useStatus();

  const repos = status?.watched_repos ?? [];
  const legalWorkflow = isLegal || isLegalModeName(projectMode) || isLegalModeName(mode);
  const inheritedProjectWorkflow = !!projectId && !!projectMode;
  const effectiveHideModePicker = hideModePicker || legalWorkflow;

  const selectedMode = modes?.find((m) => m.name === mode);
  const phases = selectedMode?.phases?.slice().sort((a, b) => a.priority - b.priority) ?? [];

  const groupedModes = useMemo(() => {
    if (!modes) return [];
    const groups: { category: string; modes: PipelineMode[] }[] = [];
    const seen = new Map<string, number>();
    for (const m of modes) {
      const cat = m.category || "Other";
      if (seen.has(cat)) {
        groups[seen.get(cat)!].modes.push(m);
      } else {
        seen.set(cat, groups.length);
        groups.push({ category: cat, modes: [m] });
      }
    }
    groups.sort((a, b) => a.category.localeCompare(b.category));
    return groups;
  }, [modes]);

  useEffect(() => {
    const nextMode = projectMode || defaultMode || (isLegal ? "lawborg" : "sweborg");
    setMode(nextMode);
  }, [defaultMode, isLegal, projectMode]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim()) return;
    setSubmitting(true);
    setError("");
    try {
      const modeForRequest = inheritedProjectWorkflow ? undefined : mode;
      await createTask(
        title.trim(),
        description.trim(),
        modeForRequest,
        repoPath || undefined,
        projectId,
        taskType || undefined,
        requiresExhaustiveCorpusReview,
      );
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      setTitle("");
      setDescription("");
      setTaskType("");
      setRequiresExhaustiveCorpusReview(false);
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create task");
    } finally {
      setSubmitting(false);
    }
  }

  if (!open) {
    return (
      <button
        onClick={() => setOpen(true)}
        className="inline-flex items-center gap-1.5 rounded-lg bg-amber-500/15 px-3.5 py-2 min-h-[44px] text-[12px] font-medium text-amber-400 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/25"
      >
        <Plus className="h-3.5 w-3.5" />
        <span className="hidden md:inline">{buttonLabel}</span>
        <span className="md:hidden">New</span>
      </button>
    );
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/70 backdrop-blur-sm pt-[5vh] md:pt-[15vh] overflow-y-auto"
      onClick={() => setOpen(false)}
    >
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={handleSubmit}
        className="w-full max-w-lg rounded-2xl border border-[#2a2520] bg-[#1c1a17] p-4 md:p-6 mx-3 md:mx-0 shadow-2xl mb-[5vh]"
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-[16px] font-semibold text-zinc-100">Create Task</h2>
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="rounded-lg p-1.5 text-zinc-500 transition-colors hover:bg-white/[0.06] hover:text-zinc-300"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-[12px] font-medium text-zinc-400">Title</label>
            <input
              autoFocus
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="What needs to be done?"
              className="w-full rounded-xl border border-[#2a2520] bg-[#232019] px-4 py-2.5 text-[14px] text-zinc-100 placeholder-zinc-600 outline-none transition-colors focus:border-amber-500/30"
            />
          </div>

          <div>
            <label className="mb-1.5 block text-[12px] font-medium text-zinc-400">
              Description <span className="text-zinc-600">optional</span>
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Additional context or instructions..."
              rows={3}
              className="w-full rounded-xl border border-[#2a2520] bg-[#232019] px-4 py-2.5 text-[14px] text-zinc-100 placeholder-zinc-600 outline-none transition-colors focus:border-amber-500/30 resize-none"
            />
          </div>

          <div className="flex gap-3">
            {!effectiveHideModePicker && (
              <div className="flex-1">
                <label className="mb-1.5 block text-[12px] font-medium text-zinc-400">Mode</label>
                <select
                  value={mode}
                  onChange={(e) => setMode(e.target.value)}
                  className="w-full rounded-xl border border-[#2a2520] bg-[#232019] px-4 py-2.5 text-[13px] text-zinc-200 outline-none transition-colors focus:border-amber-500/30"
                >
                  {groupedModes.length > 0 ? (
                    groupedModes.map((g) => (
                      <optgroup key={g.category} label={g.category}>
                        {g.modes.map((m) => (
                          <option key={m.name} value={m.name}>
                            {m.experimental ? `${m.label} (experimental)` : m.label}
                          </option>
                        ))}
                      </optgroup>
                    ))
                  ) : (
                    <option value="sweborg">Software Engineering</option>
                  )}
                </select>
              </div>
            )}
            {effectiveHideModePicker && (
              <div className="flex-1">
                <label className="mb-1.5 block text-[12px] font-medium text-zinc-400">Workflow</label>
                <div className="rounded-xl border border-[#2a2520] bg-[#232019] px-4 py-3 text-[13px] text-zinc-200">
                  <div className="font-medium text-zinc-100">{selectedMode?.label ?? mode}</div>
                  <div className="mt-1 text-[11px] text-zinc-500">
                    {inheritedProjectWorkflow
                      ? "Inherited automatically from this matter."
                      : legalWorkflow
                        ? "Applied automatically for legal work."
                        : "Applied automatically."}
                  </div>
                </div>
              </div>
            )}

            {repos.length > 1 && !projectId && (
              <div className="flex-1">
                <label className="mb-1.5 block text-[12px] font-medium text-zinc-400">Repository</label>
                <select
                  value={repoPath}
                  onChange={(e) => setRepoPath(e.target.value)}
                  className="w-full rounded-xl border border-[#2a2520] bg-[#232019] px-4 py-2.5 text-[13px] text-zinc-200 outline-none transition-colors focus:border-amber-500/30"
                >
                  <option value="">Default</option>
                  {repos.map((r) => (
                    <option key={r.path} value={r.path}>
                      {repoName(r.path)}
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>

          {phases.length > 0 && (
            <div>
              <label className="mb-2 block text-[12px] font-medium text-zinc-400">
                {effectiveHideModePicker ? "Workflow" : "Pipeline"}
              </label>
              <div className="flex flex-wrap items-center gap-1.5">
                {phases.map((p, i) => (
                  <span key={p.name} className="flex items-center">
                    <span className="rounded-lg bg-[#232019] px-2.5 py-1 text-[12px] text-[#9c9486] ring-1 ring-inset ring-[#2a2520]">
                      {p.label}
                    </span>
                    {i < phases.length - 1 && <ChevronRight className="mx-1 h-3 w-3 text-zinc-700" />}
                  </span>
                ))}
              </div>
            </div>
          )}

          {legalWorkflow && (
            <div>
              <label className="mb-1.5 block text-[12px] font-medium text-zinc-400">Task Type</label>
              <select
                value={taskType}
                onChange={(e) => setTaskType(e.target.value)}
                className="w-full rounded-xl border border-[#2a2520] bg-[#232019] px-4 py-2.5 text-[13px] text-zinc-200 outline-none transition-colors focus:border-amber-500/30"
              >
                {LEGAL_TASK_TYPES.map((t) => (
                  <option key={t.value} value={t.value}>
                    {t.label}
                  </option>
                ))}
              </select>
            </div>
          )}

          {legalWorkflow && !!projectId && (
            <label className="block rounded-xl border border-[#2a2520] bg-[#151412] px-4 py-3">
              <div className="flex items-start gap-3">
                <input
                  type="checkbox"
                  checked={requiresExhaustiveCorpusReview}
                  onChange={(e) => setRequiresExhaustiveCorpusReview(e.target.checked)}
                  className="mt-0.5 h-4 w-4 rounded border-[#3a342d] bg-[#232019] text-amber-500 focus:ring-amber-500/30"
                />
                <div>
                  <div className="text-[13px] font-medium text-zinc-100">Exhaustive corpus review</div>
                  <div className="mt-1 text-[11px] text-zinc-500">
                    Require the agent to inventory the full matter, run multiple BorgSearch passes, check coverage, and
                    inspect full documents before making corpus-wide conclusions.
                  </div>
                </div>
              </div>
            </label>
          )}
        </div>

        {error && (
          <div className="mt-3 rounded-xl border border-red-500/20 bg-red-500/[0.06] px-4 py-2.5 text-[13px] text-red-400">
            {error}
          </div>
        )}

        <div className="mt-6 flex justify-end gap-3">
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="rounded-lg px-4 py-2.5 text-[13px] font-medium text-zinc-400 transition-colors hover:text-zinc-200"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={submitting || !title.trim()}
            className="rounded-lg bg-amber-500 px-5 py-2.5 text-[13px] font-medium text-white transition-colors hover:bg-amber-400 disabled:opacity-50 shadow-lg shadow-amber-500/20"
          >
            {submitting ? "Creating..." : "Create Task"}
          </button>
        </div>
      </form>
    </div>
  );
}
