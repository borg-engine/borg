import type { PipelineModeFull } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Scale, FileText, Plus } from "lucide-react";

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
  const legalMode = builtIn.find((m) => m.name === "legal" || m.name === "lawborg");

  return (
    <div className="flex h-full w-[250px] shrink-0 flex-col border-r border-[#2a2520] bg-[#0f0e0c]">
      <div className="p-4 pb-3">
        <h3 className="text-[13px] font-semibold text-[#e8e0d4]">Pipelines</h3>
      </div>

      <div className="flex-1 overflow-y-auto px-3 pb-3 space-y-1.5">
        {/* Legal Work */}
        {legalMode && (
          <button
            onClick={() => onSelect(legalMode, true)}
            className={cn(
              "flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition-colors",
              activeName === legalMode.name
                ? "bg-amber-500/[0.08] text-[#e8e0d4] ring-1 ring-inset ring-amber-500/20"
                : "text-[#9c9486] hover:bg-[#1c1a17] hover:text-[#e8e0d4]",
            )}
          >
            <div
              className={cn(
                "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
                activeName === legalMode.name ? "bg-amber-500/15" : "bg-[#1c1a17]",
              )}
            >
              <Scale className="h-4 w-4 text-amber-400/70" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-medium">Legal Work</div>
              <div className="text-[11px] text-[#6b6459]">{legalMode.phases.length} phases</div>
            </div>
          </button>
        )}

        {/* Blank Canvas */}
        <button
          onClick={onNew}
          className={cn(
            "flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition-colors",
            activeName === "" && !custom.some((m) => m.name === activeName)
              ? "bg-amber-500/[0.08] text-[#e8e0d4] ring-1 ring-inset ring-amber-500/20"
              : "text-[#9c9486] hover:bg-[#1c1a17] hover:text-[#e8e0d4]",
          )}
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[#1c1a17]">
            <FileText className="h-4 w-4 text-[#6b6459]" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-medium">Blank Canvas</div>
            <div className="text-[11px] text-[#6b6459]">Start from scratch</div>
          </div>
        </button>

        {/* Custom pipelines */}
        {custom.length > 0 && (
          <>
            <div className="pt-3 pb-1 px-2">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-[#6b6459]">Custom</span>
            </div>
            {custom.map((m) => (
              <div
                key={m.name}
                className={cn(
                  "group flex items-center gap-1 rounded-xl transition-colors",
                  activeName === m.name
                    ? "bg-amber-500/[0.08] ring-1 ring-inset ring-amber-500/20"
                    : "hover:bg-[#1c1a17]",
                )}
              >
                <button
                  onClick={() => onSelect(m, false)}
                  className={cn(
                    "flex flex-1 items-center gap-3 px-3 py-3 text-left",
                    activeName === m.name ? "text-[#e8e0d4]" : "text-[#9c9486]",
                  )}
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-[#1c1a17]">
                    <FileText className="h-4 w-4 text-[#6b6459]" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium">{m.label || m.name}</div>
                    <div className="text-[11px] text-[#6b6459]">{m.phases.length} phases</div>
                  </div>
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(m.name);
                  }}
                  aria-label={`Delete ${m.label || m.name}`}
                  className="mr-2 hidden rounded-lg px-1.5 py-1 text-[11px] text-[#6b6459] hover:bg-red-500/15 hover:text-red-400 group-hover:block"
                >
                  &times;
                </button>
              </div>
            ))}
          </>
        )}
      </div>

      <div className="border-t border-[#2a2520] p-3">
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
