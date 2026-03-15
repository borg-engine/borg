import { FileText, Plus, Scale } from "lucide-react";
import { useDashboardMode } from "@/lib/dashboard-mode";
import type { PipelineModeFull } from "@/lib/types";
import { cn } from "@/lib/utils";

export function ModeSidebar({
  builtIn,
  custom,
  activeName,
  onSelect,
  onNew,
  onDelete,
}: {
  builtIn: PipelineModeFull[];
  custom: PipelineModeFull[];
  allowExperimental: boolean;
  activeName: string;
  onSelect: (mode: PipelineModeFull, readOnly: boolean) => void;
  onNew: () => void;
  onDelete: (name: string) => void;
}) {
  const { isLegal } = useDashboardMode();
  const legalMode = isLegal ? builtIn.find((m) => m.name === "legal" || m.name === "lawborg") : null;

  return (
    <div className="flex h-full w-[250px] shrink-0 flex-col border-r border-[var(--color-border)] bg-[var(--color-bg)]">
      <div className="p-4 pb-3">
        <h3 className="text-[13px] font-semibold text-[var(--color-text)]">Pipelines</h3>
      </div>

      <div className="flex-1 overflow-y-auto px-3 pb-3 space-y-1.5">
        {/* Legal Work */}
        {legalMode && (
          <button
            onClick={() => onSelect(legalMode, true)}
            className={cn(
              "flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition-colors",
              activeName === legalMode.name
                ? "bg-amber-500/[0.08] text-[var(--color-text)] ring-1 ring-inset ring-amber-500/20"
                : "text-[var(--color-text-secondary)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]",
            )}
          >
            <div
              className={cn(
                "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
                activeName === legalMode.name ? "bg-amber-500/15" : "bg-[var(--color-card)]",
              )}
            >
              <Scale className="h-4 w-4 text-amber-400/70" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-medium">Legal Work</div>
              <div className="text-[11px] text-[var(--color-text-tertiary)]">{legalMode.phases.length} phases</div>
            </div>
          </button>
        )}

        {/* Blank Canvas */}
        <button
          onClick={onNew}
          className={cn(
            "flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition-colors",
            activeName === "" && !custom.some((m) => m.name === activeName)
              ? "bg-amber-500/[0.08] text-[var(--color-text)] ring-1 ring-inset ring-amber-500/20"
              : "text-[var(--color-text-secondary)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]",
          )}
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[var(--color-card)]">
            <FileText className="h-4 w-4 text-[var(--color-text-tertiary)]" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-medium">Blank Canvas</div>
            <div className="text-[11px] text-[var(--color-text-tertiary)]">Start from scratch</div>
          </div>
        </button>

        {/* Custom pipelines */}
        {custom.length > 0 && (
          <>
            <div className="pt-3 pb-1 px-2">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--color-text-tertiary)]">Custom</span>
            </div>
            {custom.map((m) => (
              <div
                key={m.name}
                className={cn(
                  "group flex items-center gap-1 rounded-xl transition-colors",
                  activeName === m.name
                    ? "bg-amber-500/[0.08] ring-1 ring-inset ring-amber-500/20"
                    : "hover:bg-[var(--color-card)]",
                )}
              >
                <button
                  onClick={() => onSelect(m, false)}
                  className={cn(
                    "flex flex-1 items-center gap-3 px-3 py-3 text-left",
                    activeName === m.name ? "text-[var(--color-text)]" : "text-[var(--color-text-secondary)]",
                  )}
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[var(--color-card)]">
                    <FileText className="h-4 w-4 text-[var(--color-text-tertiary)]" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium">{m.label || m.name}</div>
                    <div className="text-[11px] text-[var(--color-text-tertiary)]">{m.phases.length} phases</div>
                  </div>
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(m.name);
                  }}
                  aria-label={`Delete ${m.label || m.name}`}
                  className="mr-2 hidden rounded-lg px-1.5 py-1 text-[11px] text-[var(--color-text-tertiary)] hover:bg-red-500/15 hover:text-red-400 group-hover:block"
                >
                  &times;
                </button>
              </div>
            ))}
          </>
        )}
      </div>

      <div className="border-t border-[var(--color-border)] p-3">
        <button
          onClick={onNew}
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-amber-500/15 px-3 py-2.5 text-[13px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20"
        >
          <Plus className="h-4 w-4" />
          New Pipeline
        </button>
      </div>
    </div>
  );
}
