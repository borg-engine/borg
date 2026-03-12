import { Layers } from "lucide-react";
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { removeCustomMode, saveCustomMode, useCustomModes, useFullModes, useSettings } from "@/lib/api";
import { useDashboardMode } from "@/lib/dashboard-mode";
import type { PipelineModeFull } from "@/lib/types";
import { getProfile } from "./mode-creator/category-profiles";
import { ModeSettings } from "./mode-creator/mode-settings";
import { ModeSidebar } from "./mode-creator/mode-sidebar";
import { PhaseDetail } from "./mode-creator/phase-detail";
import { PhaseStrip } from "./mode-creator/phase-strip";
import { blankMode, editorReducer, INITIAL_STATE } from "./mode-creator/reducer";

const CORE_MODES = new Set(["sweborg", "lawborg", "swe", "legal", "knowledge"]);

export function ModeCreatorPanel() {
  const { data: allModes = [], refetch: refetchAll } = useFullModes();
  const { data: customModes = [], refetch: refetchCustom } = useCustomModes();
  const { data: settings } = useSettings();
  const [state, dispatch] = useReducer(editorReducer, INITIAL_STATE);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");

  const { isSWE, isLegal: isDashLegal, mode: dashboardMode } = useDashboardMode();
  const allowExperimental = settings?.experimental_domains === true;
  const visibleCats = useMemo(() => {
    const raw = settings?.visible_categories ?? "";
    const cats = raw
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    return cats.length > 0 ? new Set(cats) : null;
  }, [settings?.visible_categories]);

  const customNameSet = useMemo(() => new Set(customModes.map((m) => m.name)), [customModes]);
  const builtInModes = useMemo(
    () =>
      allModes.filter(
        (m) =>
          !customNameSet.has(m.name) &&
          (allowExperimental || CORE_MODES.has(m.name)) &&
          (visibleCats === null || visibleCats.has(m.category || "")),
      ),
    [allModes, customNameSet, allowExperimental, visibleCats],
  );

  const autoLoaded = useRef(false);
  useEffect(() => {
    if (autoLoaded.current || builtInModes.length === 0) return;
    if (isDashLegal) {
      const legal = builtInModes.find((m) => m.name === "legal" || m.name === "lawborg");
      if (legal) {
        dispatch({ type: "LOAD_MODE", mode: legal, readOnly: true });
        autoLoaded.current = true;
      }
    }
  }, [builtInModes, isDashLegal]);

  const handleSelect = useCallback((mode: PipelineModeFull, readOnly: boolean) => {
    dispatch({ type: "LOAD_MODE", mode, readOnly });
    setMsg("");
  }, []);

  const handleNew = useCallback(() => {
    const mode = blankMode();
    if (isSWE) {
      if (!allowExperimental && (mode.category ?? "").toLowerCase() !== "engineering") {
        mode.category = "Engineering";
      }
    } else {
      mode.category = "Professional Services";
      mode.integration = "none" as PipelineModeFull["integration"];
    }
    dispatch({ type: "LOAD_MODE", mode, readOnly: false });
    setMsg("");
  }, [allowExperimental, isSWE]);

  const handleFork = useCallback(() => {
    const forkName = `${state.mode.name}_custom`;
    dispatch({ type: "FORK", newName: forkName });
    setMsg("");
  }, [state.mode.name]);

  const handleSave = useCallback(async () => {
    if (busy) return;
    if (!allowExperimental && !CORE_MODES.has(state.mode.name)) {
      setMsg("Save blocked: enable Experimental Domains in Settings for non-core mode names.");
      return;
    }
    setBusy(true);
    setMsg("");
    try {
      await saveCustomMode(state.mode);
      await Promise.all([refetchAll(), refetchCustom()]);
      dispatch({ type: "LOAD_MODE", mode: state.mode, readOnly: false });
      setMsg(`Saved '${state.mode.name}'`);
    } catch (err) {
      setMsg(`Save failed: ${err instanceof Error ? err.message : "unknown"}`);
    } finally {
      setBusy(false);
    }
  }, [allowExperimental, busy, state.mode, refetchAll, refetchCustom]);

  const handleDiscard = useCallback(() => {
    if (!state.original) return;
    const orig = JSON.parse(state.original) as PipelineModeFull;
    dispatch({ type: "LOAD_MODE", mode: orig, readOnly: state.isReadOnly });
    setMsg("");
  }, [state.original, state.isReadOnly]);

  const handleDelete = useCallback(
    async (name: string) => {
      if (busy) return;
      setBusy(true);
      setMsg("");
      try {
        await removeCustomMode(name);
        await Promise.all([refetchAll(), refetchCustom()]);
        if (state.mode.name === name) {
          dispatch({ type: "LOAD_MODE", mode: blankMode(), readOnly: false });
        }
        setMsg(`Deleted '${name}'`);
      } catch (err) {
        setMsg(`Delete failed: ${err instanceof Error ? err.message : "unknown"}`);
      } finally {
        setBusy(false);
      }
    },
    [busy, state.mode.name, refetchAll, refetchCustom],
  );

  const { mode, selectedPhaseIndex, isDirty, isReadOnly } = state;
  const selectedPhase = selectedPhaseIndex !== null ? mode.phases[selectedPhaseIndex] : null;
  const phaseNames = mode.phases.map((p) => p.name);
  const profile = useMemo(() => getProfile(mode.category || "", false, dashboardMode), [mode.category, dashboardMode]);

  return (
    <div className="flex h-full min-h-0">
      <ModeSidebar
        builtIn={builtInModes}
        custom={customModes}
        allowExperimental={allowExperimental}
        activeName={mode.name}
        onSelect={handleSelect}
        onNew={handleNew}
        onDelete={handleDelete}
      />

      <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
        {/* Fork banner for built-in modes */}
        {isReadOnly && mode.name && (
          <button
            onClick={handleFork}
            className="flex shrink-0 items-center justify-between border-b border-amber-500/20 bg-amber-500/[0.04] px-5 py-3 text-left transition-colors hover:bg-amber-500/[0.08]"
          >
            <div>
              <div className="text-[13px] font-medium text-amber-300">Viewing built-in template</div>
              <div className="text-[12px] text-amber-400/50">Click to create an editable copy</div>
            </div>
            <span className="rounded-lg bg-amber-500/15 px-4 py-2 text-[13px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20">
              Copy &amp; Edit
            </span>
          </button>
        )}

        {/* Header */}
        <div className="shrink-0 border-b border-[#2a2520] p-5">
          {!mode.name && !isReadOnly ? (
            <div className="flex items-center gap-3">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[#1c1a17] ring-1 ring-amber-900/20">
                <Layers className="h-6 w-6 text-amber-400/60" />
              </div>
              <div>
                <h2 className="text-[20px] font-semibold text-[#e8e0d4]">Pipelines</h2>
                <p className="text-[13px] text-[#6b6459]">View and configure pipeline definitions for your agents.</p>
              </div>
            </div>
          ) : (
            <ModeSettings
              mode={mode}
              readOnly={isReadOnly}
              onChange={(key, value) => dispatch({ type: "UPDATE_MODE", key, value })}
            />
          )}
        </div>

        {/* Phase content */}
        <div className="flex-1 overflow-y-auto p-5">
          <div className="space-y-4">
            <PhaseStrip
              phases={mode.phases}
              selectedIndex={selectedPhaseIndex}
              readOnly={isReadOnly}
              showComplianceOptions={profile.showComplianceButtons}
              onSelect={(i) => dispatch({ type: "SELECT_PHASE", index: i })}
              onAdd={(after) => dispatch({ type: "ADD_PHASE", afterIndex: after })}
              onAddCompliance={(after, complianceProfile) =>
                dispatch({ type: "ADD_COMPLIANCE_PHASE", afterIndex: after, profile: complianceProfile })
              }
              onRemove={(i) => dispatch({ type: "REMOVE_PHASE", index: i })}
              onMove={(from, to) => dispatch({ type: "MOVE_PHASE", from, to })}
            />
            {selectedPhase && selectedPhaseIndex !== null && (
              <PhaseDetail
                phase={selectedPhase}
                phaseNames={phaseNames}
                readOnly={isReadOnly}
                onChange={(patch) => dispatch({ type: "UPDATE_PHASE", index: selectedPhaseIndex, patch })}
                profile={profile}
              />
            )}
            {!selectedPhase && mode.phases.length > 0 && (
              <div className="flex flex-col items-center rounded-xl border-2 border-dashed border-[#2a2520] py-10 text-center">
                <p className="text-[14px] text-[#9c9486]">Select a phase above to edit</p>
                <p className="mt-1 text-[12px] text-[#6b6459]">Click on any phase node to view its configuration</p>
              </div>
            )}
          </div>
        </div>

        {/* Sticky save bar */}
        {(isDirty || msg) && (
          <div className="sticky bottom-0 flex shrink-0 items-center gap-3 border-t border-[#2a2520] bg-[#0f0e0c]/95 px-5 py-3 backdrop-blur">
            {isDirty && !isReadOnly && (
              <>
                <button
                  onClick={handleDiscard}
                  disabled={busy}
                  className="rounded-lg border border-[#2a2520] bg-[#1c1a17] px-4 py-2 text-[13px] text-[#9c9486] transition-colors hover:text-[#e8e0d4] disabled:opacity-50"
                >
                  Discard
                </button>
                <button
                  onClick={handleSave}
                  disabled={busy || !mode.name.trim()}
                  className="rounded-lg bg-amber-500/20 px-4 py-2 text-[13px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/30 disabled:opacity-50"
                >
                  {busy ? "Saving..." : "Save"}
                </button>
              </>
            )}
            {msg && <span className="ml-auto text-[12px] text-[#6b6459]">{msg}</span>}
          </div>
        )}
      </div>
    </div>
  );
}
