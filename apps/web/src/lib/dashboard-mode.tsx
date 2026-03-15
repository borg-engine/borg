import { createContext, type ReactNode, useContext, useMemo } from "react";
import { useUserSettings } from "./api";

export type DashboardMode = "general" | "swe" | "legal" | "knowledge";

interface DashboardModeCtx {
  mode: DashboardMode;
  isLegal: boolean;
  isSWE: boolean;
}

const ctx = createContext<DashboardModeCtx>({ mode: "general", isLegal: false, isSWE: false });

function parseMode(raw: string | undefined): DashboardMode {
  if (raw === "swe") return "swe";
  if (raw === "legal") return "legal";
  if (raw === "knowledge") return "knowledge";
  return "general";
}

export function DashboardModeProvider({ children }: { children: ReactNode }) {
  const { data: userSettings } = useUserSettings();
  const value = useMemo<DashboardModeCtx>(() => {
    const mode = parseMode(userSettings?.dashboard_mode);
    return { mode, isLegal: mode === "legal", isSWE: mode === "swe" };
  }, [userSettings?.dashboard_mode]);
  return <ctx.Provider value={value}>{children}</ctx.Provider>;
}

export function useDashboardMode(): DashboardModeCtx {
  return useContext(ctx);
}
