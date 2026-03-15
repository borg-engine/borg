import { Check, Copy, Download, RotateCcw, Upload } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import {
  COLOR_LABELS,
  FONT_OPTIONS,
  PRESETS,
  useThemeConfig,
  type ThemeDensity,
  type ThemePreset,
  type ThemeRadius,
} from "@/lib/theme-provider";
import { cn } from "@/lib/utils";

export function ThemePanel() {
  const { theme, updateTheme, resetTheme, exportTheme, importTheme } = useThemeConfig();
  const [copied, setCopied] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handlePresetClick = useCallback((preset: ThemePreset) => {
    updateTheme({ preset, colors: {} });
  }, [updateTheme]);

  const handleColorChange = useCallback((key: string, value: string) => {
    updateTheme({
      preset: "custom",
      colors: { ...theme.colors, [key]: value },
    });
  }, [theme.colors, updateTheme]);

  const handleExport = useCallback(() => {
    const json = exportTheme();
    navigator.clipboard.writeText(json).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [exportTheme]);

  const handleImport = useCallback(() => {
    const input = prompt("Paste theme JSON:");
    if (!input) return;
    const ok = importTheme(input);
    if (!ok) {
      setImportError("Invalid theme JSON");
      setTimeout(() => setImportError(null), 3000);
    }
  }, [importTheme]);

  const handleFileImport = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const ok = importTheme(reader.result as string);
      if (!ok) {
        setImportError("Invalid theme file");
        setTimeout(() => setImportError(null), 3000);
      }
    };
    reader.readAsText(file);
    e.target.value = "";
  }, [importTheme]);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-2xl space-y-8 px-6 py-8">
        <div>
          <h2 className="text-[18px] font-semibold text-[var(--color-text)]">Theme</h2>
          <p className="mt-1 text-[13px] text-[var(--color-text-tertiary)]">
            Customize the dashboard appearance. Changes apply instantly and sync across sessions.
          </p>
        </div>

        {/* Presets */}
        <section className="space-y-3">
          <h3 className="text-[13px] font-medium uppercase tracking-wider text-[var(--color-text-tertiary)]">Presets</h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-3">
            {PRESETS.map(({ key, label, swatches }) => {
              const active = theme.preset === key;
              return (
                <button
                  key={key}
                  onClick={() => handlePresetClick(key)}
                  className={cn(
                    "group relative flex flex-col items-center gap-2 rounded-xl border p-3 transition-all",
                    active
                      ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                      : "border-[var(--color-border)] bg-[var(--color-card)] hover:border-[var(--color-border-hover)]",
                  )}
                >
                  <div className="flex gap-1">
                    {swatches.map((color, i) => (
                      <div
                        key={i}
                        className="h-6 w-6 rounded-md ring-1 ring-inset ring-black/10"
                        style={{ backgroundColor: color }}
                      />
                    ))}
                  </div>
                  <span className={cn(
                    "text-[12px] font-medium",
                    active ? "text-[var(--color-accent-text)]" : "text-[var(--color-text-secondary)]",
                  )}>
                    {label}
                  </span>
                  {active && (
                    <div className="absolute -top-1.5 -right-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-[var(--color-accent)] text-white">
                      <Check className="h-3 w-3" strokeWidth={3} />
                    </div>
                  )}
                </button>
              );
            })}
          </div>
        </section>

        {/* Color Customization */}
        <section className="space-y-3">
          <h3 className="text-[13px] font-medium uppercase tracking-wider text-[var(--color-text-tertiary)]">
            Colors
            {theme.preset === "custom" && (
              <span className="ml-2 text-[11px] normal-case tracking-normal text-[var(--color-accent-text)]">Custom</span>
            )}
          </h3>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            {COLOR_LABELS.map(({ key, label }) => {
              const currentValue = theme.colors[key] || getComputedStyle(document.documentElement).getPropertyValue(key).trim();
              return (
                <div key={key} className="flex items-center gap-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2.5">
                  <label className="relative flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-lg ring-1 ring-inset ring-black/10 overflow-hidden" style={{ backgroundColor: currentValue }}>
                    <input
                      type="color"
                      value={currentValue || "#000000"}
                      onChange={(e) => handleColorChange(key, e.target.value)}
                      className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                    />
                  </label>
                  <div className="min-w-0 flex-1">
                    <div className="text-[12px] font-medium text-[var(--color-text)]">{label}</div>
                    <div className="text-[11px] font-mono text-[var(--color-text-muted)]">{currentValue}</div>
                  </div>
                </div>
              );
            })}
          </div>
        </section>

        {/* Font */}
        <section className="space-y-3">
          <h3 className="text-[13px] font-medium uppercase tracking-wider text-[var(--color-text-tertiary)]">Font</h3>
          <div className="flex gap-2">
            {FONT_OPTIONS.map(({ value, label }) => (
              <button
                key={value}
                onClick={() => updateTheme({ fontFamily: value })}
                className={cn(
                  "rounded-lg border px-4 py-2 text-[12px] font-medium transition-all",
                  theme.fontFamily === value
                    ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent-text)]"
                    : "border-[var(--color-border)] bg-[var(--color-card)] text-[var(--color-text-secondary)] hover:border-[var(--color-border-hover)]",
                )}
              >
                {label}
              </button>
            ))}
          </div>
        </section>

        {/* Density */}
        <section className="space-y-3">
          <h3 className="text-[13px] font-medium uppercase tracking-wider text-[var(--color-text-tertiary)]">Density</h3>
          <div className="flex gap-2">
            {(["compact", "default", "comfortable"] as ThemeDensity[]).map((d) => (
              <button
                key={d}
                onClick={() => updateTheme({ density: d })}
                className={cn(
                  "rounded-lg border px-4 py-2 text-[12px] font-medium capitalize transition-all",
                  theme.density === d
                    ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent-text)]"
                    : "border-[var(--color-border)] bg-[var(--color-card)] text-[var(--color-text-secondary)] hover:border-[var(--color-border-hover)]",
                )}
              >
                {d}
              </button>
            ))}
          </div>
          <DensityPreview />
        </section>

        {/* Border Radius */}
        <section className="space-y-3">
          <h3 className="text-[13px] font-medium uppercase tracking-wider text-[var(--color-text-tertiary)]">Border Radius</h3>
          <div className="flex gap-2">
            {(["sharp", "rounded", "pill"] as ThemeRadius[]).map((r) => (
              <button
                key={r}
                onClick={() => updateTheme({ radius: r })}
                className={cn(
                  "flex items-center gap-2 border px-4 py-2 text-[12px] font-medium capitalize transition-all",
                  r === "sharp" ? "rounded" : r === "rounded" ? "rounded-lg" : "rounded-full",
                  theme.radius === r
                    ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent-text)]"
                    : "border-[var(--color-border)] bg-[var(--color-card)] text-[var(--color-text-secondary)] hover:border-[var(--color-border-hover)]",
                )}
              >
                <RadiusPreviewIcon radius={r} active={theme.radius === r} />
                {r}
              </button>
            ))}
          </div>
        </section>

        {/* Actions */}
        <section className="flex flex-wrap items-center gap-3 border-t border-[var(--color-border)] pt-6">
          <button
            onClick={resetTheme}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-border-hover)] hover:text-[var(--color-text)]"
          >
            <RotateCcw className="h-3.5 w-3.5" />
            Reset to Default
          </button>
          <button
            onClick={handleExport}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-border-hover)] hover:text-[var(--color-text)]"
          >
            {copied ? <Check className="h-3.5 w-3.5 text-[var(--color-success)]" /> : <Copy className="h-3.5 w-3.5" />}
            {copied ? "Copied!" : "Export"}
          </button>
          <button
            onClick={handleImport}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-border-hover)] hover:text-[var(--color-text)]"
          >
            <Download className="h-3.5 w-3.5" />
            Import
          </button>
          <label className="flex cursor-pointer items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[12px] font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-border-hover)] hover:text-[var(--color-text)]">
            <Upload className="h-3.5 w-3.5" />
            Import File
            <input ref={fileInputRef} type="file" accept=".json" onChange={handleFileImport} className="hidden" />
          </label>
          {importError && (
            <span className="text-[12px] text-[var(--color-error)]">{importError}</span>
          )}
        </section>

        {/* Live preview card */}
        <section className="space-y-3">
          <h3 className="text-[13px] font-medium uppercase tracking-wider text-[var(--color-text-tertiary)]">Preview</h3>
          <PreviewCard />
        </section>
      </div>
    </div>
  );
}

function DensityPreview() {
  return (
    <div className="flex gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-3">
      {[1, 2, 3].map((i) => (
        <div
          key={i}
          className="h-3 flex-1 rounded bg-[var(--color-border)]"
          style={{ transform: `scaleY(var(--spacing-density, 1))` }}
        />
      ))}
    </div>
  );
}

function RadiusPreviewIcon({ radius, active }: { radius: ThemeRadius; active: boolean }) {
  const r = radius === "sharp" ? "0" : radius === "rounded" ? "3" : "6";
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="shrink-0">
      <rect
        x="1" y="1" width="12" height="12" rx={r}
        stroke={active ? "var(--color-accent)" : "var(--color-text-tertiary)"}
        strokeWidth="1.5"
        fill="none"
      />
    </svg>
  );
}

function PreviewCard() {
  return (
    <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] p-4 space-y-3">
      <div className="flex items-center gap-3">
        <div className="h-8 w-8 rounded-lg bg-[var(--color-accent-soft)] flex items-center justify-center">
          <div className="h-4 w-4 rounded bg-[var(--color-accent)]" />
        </div>
        <div className="flex-1">
          <div className="text-[13px] font-medium text-[var(--color-text)]">Sample Card Title</div>
          <div className="text-[11px] text-[var(--color-text-secondary)]">Secondary description text</div>
        </div>
      </div>
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-3">
        <div className="flex items-center justify-between">
          <span className="text-[12px] text-[var(--color-text-tertiary)]">Status</span>
          <span className="rounded-full bg-[var(--color-accent-badge)] px-2 py-0.5 text-[11px] font-medium text-[var(--color-accent-text)]">
            Active
          </span>
        </div>
      </div>
      <div className="flex gap-2">
        <button className="rounded-lg bg-[var(--color-accent-btn-bg)] px-3 py-1.5 text-[12px] font-medium text-[var(--color-accent-text)] ring-1 ring-inset ring-[var(--color-accent-btn-ring)] transition-colors hover:bg-[var(--color-accent-btn-hover)]">
          Primary
        </button>
        <button className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-[12px] font-medium text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text)]">
          Secondary
        </button>
      </div>
    </div>
  );
}
