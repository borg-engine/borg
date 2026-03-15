import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

export type ThemePreset = "dark" | "light" | "midnight" | "forest" | "warm" | "custom";
export type ThemeDensity = "compact" | "default" | "comfortable";
export type ThemeRadius = "sharp" | "rounded" | "pill";

export interface ThemeConfig {
  preset: ThemePreset;
  colors: Record<string, string>;
  fontFamily: string;
  density: ThemeDensity;
  radius: ThemeRadius;
}

export const PRESETS: { key: ThemePreset; label: string; swatches: string[] }[] = [
  { key: "dark", label: "Dark", swatches: ["#0f0e0c", "#1c1a17", "#f59e0b", "#e8e0d4"] },
  { key: "light", label: "Light", swatches: ["#faf8f5", "#f0ece5", "#b45309", "#1c1a17"] },
  { key: "midnight", label: "Midnight", swatches: ["#0a0a1a", "#161636", "#818cf8", "#e0e0f0"] },
  { key: "forest", label: "Forest", swatches: ["#0a120a", "#162216", "#22c55e", "#d8e8d8"] },
  { key: "warm", label: "Warm", swatches: ["#1a1210", "#2a201e", "#f97316", "#f0e0d0"] },
];

export const COLOR_LABELS: { key: string; label: string }[] = [
  { key: "--color-bg", label: "Background" },
  { key: "--color-card", label: "Card" },
  { key: "--color-border", label: "Border" },
  { key: "--color-accent", label: "Accent" },
  { key: "--color-text", label: "Text" },
  { key: "--color-text-secondary", label: "Text Secondary" },
  { key: "--color-text-tertiary", label: "Text Tertiary" },
  { key: "--color-accent-text", label: "Accent Text" },
];

export const FONT_OPTIONS = [
  { value: "system", label: "System (Inter)" },
  { value: "mono", label: "Monospace" },
  { value: "serif", label: "Serif" },
];

const FONT_STACKS: Record<string, string> = {
  system: '"Inter Variable", "Inter", system-ui, -apple-system, sans-serif',
  mono: 'ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, monospace',
  serif: '"Georgia", "Times New Roman", serif',
};

const DENSITY_SCALE: Record<ThemeDensity, string> = {
  compact: "0.85",
  default: "1",
  comfortable: "1.15",
};

const RADIUS_VALS: Record<ThemeRadius, { sm: string; md: string; lg: string }> = {
  sharp: { sm: "2px", md: "4px", lg: "6px" },
  rounded: { sm: "6px", md: "10px", lg: "16px" },
  pill: { sm: "12px", md: "18px", lg: "9999px" },
};

const STORAGE_KEY = "borg-theme-config";
const LEGACY_STORAGE_KEY = "borg-theme";

const DEFAULT_CONFIG: ThemeConfig = {
  preset: "dark",
  colors: {},
  fontFamily: "system",
  density: "default",
  radius: "rounded",
};

interface ThemeContextValue {
  theme: ThemeConfig;
  setTheme: (config: ThemeConfig) => void;
  updateTheme: (partial: Partial<ThemeConfig>) => void;
  resetTheme: () => void;
  exportTheme: () => string;
  importTheme: (json: string) => boolean;
  presets: typeof PRESETS;
  isDark: boolean;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

function loadTheme(): ThemeConfig {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      return { ...DEFAULT_CONFIG, ...parsed };
    }
    // Migrate from legacy toggle
    const legacy = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (legacy === "light") {
      return { ...DEFAULT_CONFIG, preset: "light" };
    }
  } catch {}
  return { ...DEFAULT_CONFIG };
}

function applyThemeToDOM(config: ThemeConfig) {
  const el = document.documentElement;

  // Set data-theme for CSS preset rules
  el.setAttribute("data-theme", config.preset === "custom" ? "dark" : config.preset);

  // Maintain .dark class for existing Tailwind dark: utilities
  if (config.preset === "light") {
    el.classList.remove("dark");
  } else {
    el.classList.add("dark");
  }

  // Apply custom color overrides
  if (config.colors && Object.keys(config.colors).length > 0) {
    for (const [prop, value] of Object.entries(config.colors)) {
      el.style.setProperty(prop, value);
    }
  } else {
    // Clear any custom properties
    for (const { key } of COLOR_LABELS) {
      el.style.removeProperty(key);
    }
    // Clear derived properties too
    const derived = [
      "--color-bg-secondary", "--color-card-alt", "--color-surface",
      "--color-border-hover", "--color-text-muted", "--color-text-faint",
      "--color-accent-dim", "--color-accent-hover", "--color-accent-soft",
      "--color-accent-soft-hover", "--color-accent-badge",
      "--color-accent-btn-bg", "--color-accent-btn-ring", "--color-accent-btn-hover",
      "--color-input-bg", "--color-scrollbar", "--color-scrollbar-hover",
      "--color-selection-bg",
    ];
    for (const key of derived) {
      el.style.removeProperty(key);
    }
  }

  // Font family
  const fontStack = FONT_STACKS[config.fontFamily] || FONT_STACKS.system;
  el.style.setProperty("--font-family", fontStack);
  document.body.style.fontFamily = fontStack;

  // Density
  el.style.setProperty("--spacing-density", DENSITY_SCALE[config.density] || "1");

  // Radius
  const radii = RADIUS_VALS[config.radius] || RADIUS_VALS.rounded;
  el.style.setProperty("--radius-sm", radii.sm);
  el.style.setProperty("--radius-md", radii.md);
  el.style.setProperty("--radius-lg", radii.lg);
}

// Apply theme before first paint (synchronous)
const initialTheme = loadTheme();
applyThemeToDOM(initialTheme);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeConfig>(initialTheme);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>(null);

  const persistTheme = useCallback((config: ThemeConfig) => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
    // Also update legacy key for backwards compat
    localStorage.setItem(LEGACY_STORAGE_KEY, config.preset === "light" ? "light" : "dark");
  }, []);

  const setTheme = useCallback((config: ThemeConfig) => {
    setThemeState(config);
    applyThemeToDOM(config);
    persistTheme(config);

    // Debounced API save
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(() => {
      saveThemeToAPI(config).catch(() => {});
    }, 1000);
  }, [persistTheme]);

  const updateTheme = useCallback((partial: Partial<ThemeConfig>) => {
    setThemeState(prev => {
      const next = { ...prev, ...partial };
      applyThemeToDOM(next);
      persistTheme(next);
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => {
        saveThemeToAPI(next).catch(() => {});
      }, 1000);
      return next;
    });
  }, [persistTheme]);

  const resetTheme = useCallback(() => {
    setTheme({ ...DEFAULT_CONFIG });
  }, [setTheme]);

  const exportTheme = useCallback(() => {
    return JSON.stringify(theme, null, 2);
  }, [theme]);

  const importTheme = useCallback((json: string): boolean => {
    try {
      const parsed = JSON.parse(json);
      if (typeof parsed !== "object" || !parsed.preset) return false;
      const config: ThemeConfig = { ...DEFAULT_CONFIG, ...parsed };
      setTheme(config);
      return true;
    } catch {
      return false;
    }
  }, [setTheme]);

  const isDark = theme.preset !== "light";

  // Load from API on mount (API takes precedence over localStorage if newer)
  useEffect(() => {
    loadThemeFromAPI().then(apiTheme => {
      if (apiTheme) {
        setThemeState(apiTheme);
        applyThemeToDOM(apiTheme);
        persistTheme(apiTheme);
      }
    }).catch(() => {});
  }, [persistTheme]);

  const value = useMemo(() => ({
    theme,
    setTheme,
    updateTheme,
    resetTheme,
    exportTheme,
    importTheme,
    presets: PRESETS,
    isDark,
  }), [theme, setTheme, updateTheme, resetTheme, exportTheme, importTheme, isDark]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useThemeConfig(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useThemeConfig must be used within ThemeProvider");
  return ctx;
}

// Minimal backward-compat hook for existing code using the old useTheme
export function useTheme() {
  const { theme, updateTheme } = useThemeConfig();
  return {
    theme: theme.preset === "light" ? "light" as const : "dark" as const,
    setTheme: (t: "dark" | "light") => updateTheme({ preset: t, colors: {} }),
    toggle: () => updateTheme({ preset: theme.preset === "light" ? "dark" : "light", colors: {} }),
  };
}

// ── API persistence ─────────────────────────────────────────────────────

async function saveThemeToAPI(config: ThemeConfig) {
  try {
    const { apiFetch } = await import("@/lib/api");
    await apiFetch("/api/user/settings", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ theme: JSON.stringify(config) }),
    });
  } catch {}
}

async function loadThemeFromAPI(): Promise<ThemeConfig | null> {
  try {
    const { fetchJson } = await import("@/lib/api");
    const settings = await fetchJson<Record<string, unknown>>("/api/user/settings");
    if (settings?.theme && typeof settings.theme === "string") {
      const parsed = JSON.parse(settings.theme);
      return { ...DEFAULT_CONFIG, ...parsed };
    }
  } catch {}
  return null;
}
