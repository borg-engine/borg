import { createContext, useContext, type ReactNode } from "react";

export type DomainProfile = "legal" | "general";

export interface DomainConfig {
  profile: DomainProfile;
  tagline: string;
  accentColor: string;
  accentBg: string;
  defaultMode: "minimal" | "advanced";
  defaultView: "tasks" | "projects";
  hiddenNavKeys: string[];
}

const PROFILES: Record<DomainProfile, DomainConfig> = {
  legal: {
    profile: "legal",
    tagline: "Autonomous Work Engine",
    accentColor: "text-orange-400",
    accentBg: "bg-orange-500",
    defaultMode: "minimal",
    defaultView: "projects",
    hiddenNavKeys: ["logs", "queue", "knowledge"],
  },
  general: {
    profile: "general",
    tagline: "Autonomous Work Engine",
    accentColor: "text-orange-400",
    accentBg: "bg-orange-500",
    defaultMode: "advanced",
    defaultView: "tasks",
    hiddenNavKeys: [],
  },
};

function detectProfile(): DomainProfile {
  if (typeof window === "undefined") return "general";
  const envProfile = import.meta.env.VITE_DOMAIN_PROFILE;
  if (envProfile === "legal" || envProfile === "general") return envProfile;
  return "general";
}

const ctx = createContext<DomainConfig>(PROFILES.general);

export function DomainProvider({ children }: { children: ReactNode }) {
  const config = PROFILES[detectProfile()];
  return <ctx.Provider value={config}>{children}</ctx.Provider>;
}

export function useDomain(): DomainConfig {
  return useContext(ctx);
}
