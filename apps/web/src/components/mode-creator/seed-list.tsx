import type { SeedConfigFull, SeedOutputType } from "@/lib/types";
import { cn } from "@/lib/utils";
import { AutoTextarea } from "./auto-textarea";

export function SeedList({
  seeds,
  expandedIndex,
  readOnly,
  onExpand,
  onUpdate,
  onAdd,
  onRemove,
}: {
  seeds: SeedConfigFull[];
  expandedIndex: number | null;
  readOnly: boolean;
  onExpand: (index: number | null) => void;
  onUpdate: (index: number, patch: Partial<SeedConfigFull>) => void;
  onAdd: () => void;
  onRemove: (index: number) => void;
}) {
  return (
    <div className="space-y-3">
      {seeds.length === 0 && (
        <div className="flex flex-col items-center rounded-xl border-2 border-dashed border-[var(--color-border)] py-12 text-center">
          <p className="text-[14px] text-[var(--color-text-secondary)]">No auto tasks configured</p>
          <p className="mt-1 text-[12px] text-[var(--color-text-tertiary)]">
            Auto tasks are generated automatically when the pipeline is idle.
          </p>
        </div>
      )}

      {seeds.map((seed, i) => {
        const expanded = i === expandedIndex;
        return (
          <div
            key={`${seed.name}-${i}`}
            className={cn(
              "rounded-xl border transition-colors",
              expanded
                ? "border-amber-500/30 bg-amber-500/[0.03]"
                : "border-[var(--color-border)] bg-[var(--color-bg-secondary)] hover:border-amber-900/30",
            )}
          >
            {/* Summary row */}
            <button
              onClick={() => onExpand(expanded ? null : i)}
              className="flex w-full items-center gap-3 px-4 py-3 text-left"
            >
              <span className="text-[10px] text-[var(--color-text-tertiary)]">{expanded ? "\u25BC" : "\u25B6"}</span>
              <span
                className={cn("min-w-[80px] text-[13px] font-medium", seed.name ? "text-[var(--color-text)]" : "text-[var(--color-text-tertiary)]")}
              >
                {seed.name || "unnamed"}
              </span>
              <span className="flex-1 truncate text-[12px] text-[var(--color-text-tertiary)]">
                {seed.label || seed.prompt.slice(0, 60) || "\u2014"}
              </span>
              <span
                className={cn(
                  "rounded-lg px-2 py-0.5 text-[10px] font-medium",
                  seed.output_type === "task" ? "bg-amber-500/15 text-amber-300" : "bg-violet-500/15 text-violet-300",
                )}
              >
                {seed.output_type}
              </span>
            </button>

            {/* Expanded editor */}
            {expanded && (
              <div className="border-t border-[var(--color-border)] px-4 pb-4 pt-3 space-y-4">
                <div className="flex gap-3">
                  <Field label="Name" className="flex-1">
                    <input
                      value={seed.label}
                      onChange={(e) => {
                        onUpdate(i, { label: e.target.value });
                        onUpdate(i, {
                          name: e.target.value
                            .toLowerCase()
                            .replace(/[^a-z0-9_-]/g, "_")
                            .replace(/_+/g, "_")
                            .replace(/^_|_$/g, ""),
                        });
                      }}
                      disabled={readOnly}
                      placeholder="Auto task name"
                      className={inputCls}
                    />
                  </Field>
                  <Field label="Output" className="w-28">
                    <select
                      value={seed.output_type}
                      onChange={(e) => onUpdate(i, { output_type: e.target.value as SeedOutputType })}
                      disabled={readOnly}
                      className={inputCls}
                    >
                      <option value="task">Task</option>
                      <option value="proposal">Proposal</option>
                    </select>
                  </Field>
                </div>

                <Field label="Prompt">
                  <AutoTextarea
                    value={seed.prompt}
                    onChange={(v) => onUpdate(i, { prompt: v })}
                    disabled={readOnly}
                    placeholder="Seed prompt..."
                    minRows={3}
                  />
                </Field>

                <div className="flex items-center gap-4">
                  {!readOnly && (
                    <button
                      onClick={() => onRemove(i)}
                      className="ml-auto rounded-lg bg-red-500/10 px-3 py-1.5 text-[12px] text-red-400 ring-1 ring-inset ring-red-500/20 transition-colors hover:bg-red-500/20"
                    >
                      Remove
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>
        );
      })}

      {!readOnly && (
        <button
          onClick={onAdd}
          className="rounded-lg bg-[var(--color-card)] px-3 py-1.5 text-[12px] text-[var(--color-text-secondary)] ring-1 ring-inset ring-[var(--color-border)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)]"
        >
          + Add Auto Task
        </button>
      )}
    </div>
  );
}

const inputCls =
  "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30 disabled:opacity-50 disabled:cursor-not-allowed";

function Field({ label, className, children }: { label: string; className?: string; children: React.ReactNode }) {
  return (
    <div className={className}>
      <div className="mb-1.5 text-[12px] font-medium text-[var(--color-text-secondary)]">{label}</div>
      {children}
    </div>
  );
}
