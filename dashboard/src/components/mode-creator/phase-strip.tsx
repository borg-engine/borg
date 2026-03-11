import { ChevronDown } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { PhaseConfigFull, PhaseType } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useVocabulary } from "@/lib/vocabulary";

const TYPE_COLORS: Record<PhaseType, string> = {
  setup: "bg-[#1c1a17] text-[#6b6459]",
  agent: "bg-amber-500/15 text-amber-300",
  validate: "bg-teal-500/15 text-teal-400",
  rebase: "bg-violet-500/15 text-violet-400",
  lint_fix: "bg-cyan-500/15 text-cyan-400",
  human_review: "bg-emerald-500/15 text-emerald-400",
  compliance_check: "bg-fuchsia-500/15 text-fuchsia-400",
};

const LOOP_COLORS = ["stroke-amber-500/50", "stroke-violet-500/50", "stroke-cyan-500/50", "stroke-rose-500/50"];

const LOOP_FILL_COLORS = ["fill-amber-500/50", "fill-violet-500/50", "fill-cyan-500/50", "fill-rose-500/50"];

const LOOP_TEXT_COLORS = ["fill-amber-500/60", "fill-violet-500/60", "fill-cyan-500/60", "fill-rose-500/60"];

const COL_W = 130;
const ARC_ROW_H = 28;

interface LoopEdge {
  fromIndex: number;
  toIndex: number;
  label: string;
}

export function PhaseStrip({
  phases,
  selectedIndex,
  readOnly,
  showComplianceOptions,
  onSelect,
  onAdd,
  onAddCompliance,
  onRemove,
  onMove,
}: {
  phases: PhaseConfigFull[];
  selectedIndex: number | null;
  readOnly: boolean;
  showComplianceOptions: boolean;
  onSelect: (index: number | null) => void;
  onAdd: (afterIndex: number) => void;
  onAddCompliance: (afterIndex: number, profile: "uk_sra" | "us_prof_resp") => void;
  onRemove: (index: number) => void;
  onMove: (from: number, to: number) => void;
}) {
  const vocab = useVocabulary();
  const [showAddMenu, setShowAddMenu] = useState(false);
  const addMenuRef = useRef<HTMLDivElement | null>(null);
  const nameToIndex = useMemo(() => {
    const map = new Map<string, number>();
    phases.forEach((p, i) => {
      map.set(p.name, i);
    });
    return map;
  }, [phases]);

  useEffect(() => {
    function handlePointerDown(event: MouseEvent) {
      if (!addMenuRef.current?.contains(event.target as Node)) {
        setShowAddMenu(false);
      }
    }
    if (showAddMenu) {
      document.addEventListener("mousedown", handlePointerDown);
    }
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [showAddMenu]);

  useEffect(() => {
    setShowAddMenu(false);
  }, []);

  const loops = useMemo(() => {
    const edges: LoopEdge[] = [];
    for (let i = 0; i < phases.length; i++) {
      const phase = phases[i];
      const targetIdx = nameToIndex.get(phase.next);
      if (targetIdx !== undefined && targetIdx <= i) {
        edges.push({ fromIndex: i, toIndex: targetIdx, label: phase.next });
      }
    }
    return edges;
  }, [phases, nameToIndex]);

  const loopRows = useMemo(() => {
    const sorted = [...loops].sort((a, b) => b.fromIndex - b.toIndex - (a.fromIndex - a.toIndex));
    const rows: LoopEdge[][] = [];
    for (const edge of sorted) {
      let placed = false;
      for (const row of rows) {
        const overlaps = row.some(
          (e) =>
            !(
              Math.max(e.toIndex, edge.toIndex) < Math.min(e.fromIndex, edge.fromIndex) ||
              Math.max(e.fromIndex, edge.fromIndex) < Math.min(e.toIndex, edge.toIndex)
            ),
        );
        if (!overlaps) {
          row.push(edge);
          placed = true;
          break;
        }
      }
      if (!placed) rows.push([edge]);
    }
    return rows;
  }, [loops]);

  const totalW = (phases.length + 1) * COL_W;
  const arcH = loopRows.length * ARC_ROW_H + (loopRows.length > 0 ? 8 : 0);

  return (
    <div className="space-y-3">
      <div className="overflow-x-auto pb-1">
        <div style={{ minWidth: totalW }}>
          {/* Phase nodes */}
          <div className="flex items-center">
            {phases.map((phase, i) => {
              const selected = i === selectedIndex;
              return (
                <div key={`${phase.name}-${i}`} className="flex shrink-0 items-center" style={{ width: COL_W }}>
                  {i > 0 && (
                    <div className="flex items-center">
                      <div className="h-px w-3 bg-[#2a2520]" />
                      <span className="text-[10px] text-[#3d3830]">&rsaquo;</span>
                      <div className="h-px w-3 bg-[#2a2520]" />
                    </div>
                  )}
                  <button
                    onClick={() => onSelect(selected ? null : i)}
                    className={cn(
                      "flex-1 rounded-xl border px-3 py-2.5 text-left transition-colors",
                      selected
                        ? "border-amber-500/30 bg-amber-500/[0.06] ring-1 ring-amber-500/30"
                        : "border-[#2a2520] bg-[#151412] hover:border-amber-900/30 hover:bg-[#1c1a17]",
                    )}
                  >
                    <div
                      className={cn("text-[12px] font-medium truncate", selected ? "text-[#e8e0d4]" : "text-[#9c9486]")}
                    >
                      {vocab.statusLabels[phase.name] || phase.label || phase.name}
                    </div>
                    <span
                      className={cn(
                        "mt-1 inline-block rounded-md px-1.5 py-0.5 text-[10px]",
                        TYPE_COLORS[phase.phase_type],
                      )}
                    >
                      {phase.phase_type === "human_review" ? "\u{1F464} review" : phase.phase_type}
                    </span>
                  </button>
                </div>
              );
            })}
          </div>

          {/* Loop arcs */}
          {loopRows.length > 0 && (
            <svg width={totalW} height={arcH} className="mt-1">
              {loopRows.map((row, rowIdx) =>
                row.map((edge, edgeIdx) => {
                  const colorIdx = (rowIdx + edgeIdx) % LOOP_COLORS.length;
                  const fromX = edge.fromIndex * COL_W + COL_W / 2;
                  const toX = edge.toIndex * COL_W + COL_W / 2;
                  const y0 = 4;
                  const y1 = (rowIdx + 1) * ARC_ROW_H;
                  const midX = (fromX + toX) / 2;

                  const d = `M ${fromX} ${y0} L ${fromX} ${y1} Q ${fromX} ${y1 + 8} ${fromX - 8} ${y1 + 8} L ${toX + 8} ${y1 + 8} Q ${toX} ${y1 + 8} ${toX} ${y1} L ${toX} ${y0}`;

                  return (
                    <g key={`${edge.fromIndex}-${edge.toIndex}`}>
                      <path
                        d={d}
                        className={LOOP_COLORS[colorIdx]}
                        fill="none"
                        strokeWidth={1.5}
                        strokeDasharray="4 2"
                      />
                      <polygon
                        points={`${toX - 3},${y0 + 5} ${toX + 3},${y0 + 5} ${toX},${y0}`}
                        className={LOOP_FILL_COLORS[colorIdx]}
                      />
                      <text
                        x={midX}
                        y={y1 + 5}
                        textAnchor="middle"
                        className={cn("text-[8px]", LOOP_TEXT_COLORS[colorIdx])}
                      >
                        loop
                      </text>
                    </g>
                  );
                }),
              )}
            </svg>
          )}
        </div>
      </div>

      {/* Actions bar */}
      {!readOnly && (
        <div className="flex items-center gap-2">
          <div className="relative" ref={addMenuRef}>
            <button
              type="button"
              onClick={() => setShowAddMenu((open) => !open)}
              className="inline-flex items-center gap-1.5 rounded-lg bg-[#1c1a17] px-3 py-1.5 text-[12px] text-[#9c9486] ring-1 ring-inset ring-[#2a2520] transition-colors hover:bg-[#232019] hover:text-[#e8e0d4]"
            >
              + Add Phase
              <ChevronDown className={cn("h-3.5 w-3.5 transition-transform", showAddMenu && "rotate-180")} />
            </button>
            {showAddMenu && (
              <div className="absolute left-0 top-full z-20 mt-2 min-w-[220px] rounded-xl border border-[#2a2520] bg-[#151412] p-1.5 shadow-2xl">
                <button
                  type="button"
                  onClick={() => {
                    onAdd(selectedIndex ?? phases.length - 1);
                    setShowAddMenu(false);
                  }}
                  className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-[12px] text-[#e8e0d4] transition-colors hover:bg-[#1c1a17]"
                >
                  <span>Standard Phase</span>
                  <span className="text-[10px] text-[#6b6459]">blank</span>
                </button>
                {showComplianceOptions && (
                  <>
                    <div className="my-1 h-px bg-[#2a2520]" />
                    <button
                      type="button"
                      onClick={() => {
                        onAddCompliance(selectedIndex ?? phases.length - 1, "uk_sra");
                        setShowAddMenu(false);
                      }}
                      className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-[12px] text-[#e8e0d4] transition-colors hover:bg-[#1c1a17]"
                    >
                      <span>UK SRA Check</span>
                      <span className="text-[10px] text-[#6b6459]">compliance</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        onAddCompliance(selectedIndex ?? phases.length - 1, "us_prof_resp");
                        setShowAddMenu(false);
                      }}
                      className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left text-[12px] text-[#e8e0d4] transition-colors hover:bg-[#1c1a17]"
                    >
                      <span>US Ethics Check</span>
                      <span className="text-[10px] text-[#6b6459]">compliance</span>
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
          {selectedIndex !== null && (
            <>
              <button
                onClick={() => {
                  if (selectedIndex > 0) onMove(selectedIndex, selectedIndex - 1);
                }}
                disabled={selectedIndex <= 0}
                aria-label="Move phase left"
                className="rounded-lg bg-[#1c1a17] px-3 py-1.5 text-[12px] text-[#9c9486] ring-1 ring-inset ring-[#2a2520] transition-colors hover:bg-[#232019] disabled:opacity-30"
              >
                &larr;
              </button>
              <button
                onClick={() => {
                  if (selectedIndex < phases.length - 1) onMove(selectedIndex, selectedIndex + 1);
                }}
                disabled={selectedIndex >= phases.length - 1}
                aria-label="Move phase right"
                className="rounded-lg bg-[#1c1a17] px-3 py-1.5 text-[12px] text-[#9c9486] ring-1 ring-inset ring-[#2a2520] transition-colors hover:bg-[#232019] disabled:opacity-30"
              >
                &rarr;
              </button>
              <button
                onClick={() => onRemove(selectedIndex)}
                className="rounded-lg bg-red-500/10 px-3 py-1.5 text-[12px] text-red-400 ring-1 ring-inset ring-red-500/20 transition-colors hover:bg-red-500/20"
              >
                Remove
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
