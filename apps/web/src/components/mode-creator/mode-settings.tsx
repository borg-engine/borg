import type { PipelineModeFull } from "@/lib/types";

export function ModeSettings({
  mode,
  readOnly,
  onChange,
}: {
  mode: PipelineModeFull;
  readOnly: boolean;
  onChange: (key: keyof PipelineModeFull, value: unknown) => void;
  profile?: unknown;
}) {
  if (readOnly) {
    return <h2 className="text-[18px] font-semibold text-[var(--color-text)]">{mode.label || mode.name}</h2>;
  }

  return (
    <div>
      <div className="mb-1.5 text-[12px] font-medium text-[var(--color-text-secondary)]">Pipeline Name</div>
      <input
        value={mode.label}
        onChange={(e) => {
          onChange("label", e.target.value);
          onChange(
            "name",
            e.target.value
              .toLowerCase()
              .replace(/[^a-z0-9_-]/g, "_")
              .replace(/_+/g, "_")
              .replace(/^_|_$/g, ""),
          );
        }}
        placeholder="My Pipeline"
        className="w-64 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[13px] text-[var(--color-text)] outline-none transition-colors focus:border-amber-500/30"
      />
    </div>
  );
}
