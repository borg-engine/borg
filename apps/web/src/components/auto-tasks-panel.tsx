import { Zap } from "lucide-react";
import { useCallback, useMemo, useReducer, useState } from "react";
import { saveCustomMode, useCustomModes, useFullModes, useSettings } from "@/lib/api";
import { editorReducer, INITIAL_STATE } from "./mode-creator/reducer";
import { SeedList } from "./mode-creator/seed-list";

const CORE_MODES = new Set(["sweborg", "lawborg", "swe", "legal", "knowledge"]);

export function AutoTasksPanel() {
  const { data: allModes = [], refetch: refetchAll } = useFullModes();
  const { data: customModes = [], refetch: refetchCustom } = useCustomModes();
  const { data: settings } = useSettings();
  const [state, dispatch] = useReducer(editorReducer, INITIAL_STATE);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");

  const allowExperimental = settings?.experimental_domains === true;
  const customNameSet = useMemo(() => new Set(customModes.map((m) => m.name)), [customModes]);

  const activeModes = useMemo(
    () => [
      ...customModes,
      ...allModes.filter((m) => !customNameSet.has(m.name) && (allowExperimental || CORE_MODES.has(m.name))),
    ],
    [allModes, customModes, customNameSet, allowExperimental],
  );

  const [selectedMode, setSelectedMode] = useState<string>("");

  const currentMode = useMemo(
    () => activeModes.find((m) => m.name === selectedMode) ?? activeModes[0],
    [activeModes, selectedMode],
  );

  const isCustom = useMemo(
    () => (currentMode ? customNameSet.has(currentMode.name) : false),
    [currentMode, customNameSet],
  );

  // Load mode into reducer when selection changes
  useMemo(() => {
    if (currentMode) {
      dispatch({ type: "LOAD_MODE", mode: currentMode, readOnly: !isCustom });
      dispatch({ type: "SET_TAB", tab: "seeds" });
    }
  }, [currentMode?.name, isCustom, currentMode]);

  const handleSave = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    setMsg("");
    try {
      await saveCustomMode(state.mode);
      await Promise.all([refetchAll(), refetchCustom()]);
      setMsg("Saved");
    } catch (err) {
      setMsg(`Save failed: ${err instanceof Error ? err.message : "unknown"}`);
    } finally {
      setBusy(false);
    }
  }, [busy, state.mode, refetchAll, refetchCustom]);

  const { mode, expandedSeedIndex, isDirty, isReadOnly } = state;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="shrink-0 border-b border-[#2a2520] p-5">
        <div className="flex items-center gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[#1c1a17] ring-1 ring-amber-900/20">
            <Zap className="h-6 w-6 text-amber-400/60" />
          </div>
          <div className="flex-1">
            <h2 className="text-[20px] font-semibold text-[#e8e0d4]">Auto Tasks</h2>
            <p className="text-[13px] text-[#6b6459]">Tasks generated automatically when the pipeline is idle.</p>
          </div>
          {activeModes.length > 1 && (
            <div>
              <select
                value={currentMode?.name ?? ""}
                onChange={(e) => setSelectedMode(e.target.value)}
                className="rounded-lg border border-[#2a2520] bg-[#1c1a17] px-3 py-2 text-[13px] text-[#e8e0d4] outline-none focus:border-amber-500/30"
              >
                {activeModes.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.label || m.name}
                  </option>
                ))}
              </select>
            </div>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-5">
        {isReadOnly && (
          <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/[0.04] px-4 py-2.5 text-[12px] text-amber-400/80">
            This is a built-in pipeline. Copy &amp; Edit it from the Pipelines view to modify auto tasks.
          </div>
        )}
        <SeedList
          seeds={mode.seed_modes}
          expandedIndex={expandedSeedIndex}
          readOnly={isReadOnly}
          onExpand={(i) => dispatch({ type: "EXPAND_SEED", index: i })}
          onUpdate={(i, patch) => dispatch({ type: "UPDATE_SEED", index: i, patch })}
          onAdd={() => dispatch({ type: "ADD_SEED" })}
          onRemove={(i) => dispatch({ type: "REMOVE_SEED", index: i })}
        />
      </div>

      {(isDirty || msg) && (
        <div className="sticky bottom-0 flex shrink-0 items-center gap-3 border-t border-[#2a2520] bg-[#0f0e0c]/95 px-5 py-3 backdrop-blur">
          {isDirty && !isReadOnly && (
            <button
              onClick={handleSave}
              disabled={busy}
              className="rounded-lg bg-amber-500/20 px-4 py-2 text-[13px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/30 disabled:opacity-50"
            >
              {busy ? "Saving..." : "Save"}
            </button>
          )}
          {msg && <span className="ml-auto text-[12px] text-[#6b6459]">{msg}</span>}
        </div>
      )}
    </div>
  );
}
