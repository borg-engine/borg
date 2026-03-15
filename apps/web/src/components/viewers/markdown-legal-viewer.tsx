import { useEffect, useMemo, useRef, useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ScrollArea } from "@/components/ui/scroll-area";
import { apiBase, authHeaders, tokenReady, useTemplates } from "@/lib/api";
import { cn } from "@/lib/utils";

// ── Types ─────────────────────────────────────────────────────────────────────

interface DocumentVersion {
  sha: string;
  message: string;
  date: string;
  author: string;
}

type CitationStatus = "verified" | "unverified" | "flagged";

interface Citation {
  text: string;
  status: CitationStatus;
  anchorId: string;
  index: number;
}

export interface MarkdownLegalViewerProps {
  projectId: number;
  taskId: number;
  path: string;
  defaultTemplateId?: number | null;
}

// ── Citation parsing ──────────────────────────────────────────────────────────

const CITATION_PATTERNS = [
  // US case: Name v. Name, 123 U.S. 456 (1999)
  /[A-Z][A-Za-z\s'&,.-]+\s+v\.\s+[A-Z][A-Za-z\s'&,.-]+,\s*\d+\s+[A-Z][A-Z.]+\d*\s+\d+(?:\s*\([^)]+\d{4}\))?/g,
  // Federal Reporter (F.2d, F.3d, F.4th, F.Supp., F.Supp.2d, F.Supp.3d, F.App'x)
  /\d+\s+F\.(?:\d+(?:d|th)|Supp\.(?:\s*\d+d)?|App'x)\s+\d+(?:\s*\([^)]+\d{4}\))?/g,
  // US Reports: 123 U.S. 456
  /\d+\s+U\.S\.\s+\d+/g,
  // Supreme Court Reporter: 123 S. Ct. 456
  /\d+\s+S\.\s*Ct\.\s+\d+/g,
  // L.Ed: 123 L. Ed. 2d 456
  /\d+\s+L\.\s*Ed\.\s*(?:2d\s+)?\d+/g,
  // State reporters (Cal., N.Y., N.Y.S., A.2d/3d, N.E.2d/3d, So.2d/3d, P.2d/3d, S.E.2d, N.W.2d, S.W.3d)
  /\d+\s+(?:Cal\.(?:\s*\d+th)?|N\.Y\.(?:S\.)?(?:\s*\d+d)?|A\.\d+d|N\.E\.\d+d|So\.\s*\d+d|P\.\d+d|S\.E\.\d+d|N\.W\.\d+d|S\.W\.\d+d)\s+\d+/g,
  // US Code: 42 U.S.C. § 1983
  /\d+\s+U\.S\.C\.?\s+§+\s*\d+[\w-]*/g,
  // CFR: 29 C.F.R. § 541.100
  /\d+\s+C\.F\.R\.?\s+§+\s*\d+[\d.]*/g,
  // State statutes: Cal. Civ. Code § 1234, N.Y. Gen. Bus. Law § 349
  /(?:Cal|N\.Y|Tex|Fla|Ill|Ohio|Pa|Mass|Mich|Ga|N\.J|Va|Wash|Ariz|Md|Minn|Mo|Wis|Colo|Conn|Or|S\.C|Ky|La|Okla|Ala|Ind)\.?\s+[A-Z][A-Za-z.&\s]+§+\s*\d+[\w.-]*/g,
  // UK neutral citations: [2023] UKSC 12, [2023] EWCA Civ 456
  /\[\d{4}\]\s+(?:UKSC|EWCA\s+(?:Civ|Crim)|EWHC|UKHL|UKPC|EWCOP|UKUT|UKFTT)\s+\d+/g,
  // EU Case: Case C-123/45, Case T-123/45
  /Case\s+[CT]-\d+\/\d+/g,
  // CELEX numbers: 62019CJ0311
  /\d{5}[A-Z]{2}\d{4}/g,
  // Canadian: [2023] SCC 12, 2023 SCC 12, CanLII format
  /(?:\[\d{4}\]\s+|\d{4}\s+)(?:SCC|SCR|FC|FCA|ONCA|BCCA|ABCA|QCCA|NSCA|NBCA)\s+\d+/g,
  // CanLII citation
  /\d{4}\s+CanLII\s+\d+\s+\([A-Z]+\)/g,
  // Bare section references: § 1983
  /§+\s*\d+[\w.-]*/g,
];

function extractCitations(content: string): Citation[] {
  const found: Array<{ text: string; index: number }> = [];
  const seen = new Set<string>();

  for (const pattern of CITATION_PATTERNS) {
    const re = new RegExp(pattern.source, pattern.flags);
    let m: RegExpExecArray | null;
    // biome-ignore lint/suspicious/noAssignInExpressions: standard regex exec loop
    while ((m = re.exec(content)) !== null) {
      const text = m[0].trim();
      if (!seen.has(text)) {
        seen.add(text);
        found.push({ text, index: m.index });
      }
    }
  }

  found.sort((a, b) => a.index - b.index);

  return found.map((f, i) => ({
    text: f.text,
    status: deriveCitationStatus(content, f.index, f.text),
    anchorId: `citation-${i}`,
    index: i,
  }));
}

function deriveCitationStatus(content: string, pos: number, text: string): CitationStatus {
  // Search a window around the citation for verification signals
  const window = content.slice(Math.max(0, pos - 300), pos + 300).toUpperCase();

  // Negative treatment — flagged
  if (
    window.includes("OVERRULED") ||
    window.includes("REVERSED") ||
    window.includes("ABROGATED") ||
    window.includes("NEGATIVE TREATMENT") ||
    window.includes("FLAGGED") ||
    window.includes("NO LONGER GOOD LAW")
  ) {
    return "flagged";
  }

  // Verified via premium tools (Shepard's, KeyCite) or confirmed by name
  if (
    window.includes("VERIFIED") ||
    window.includes("GOOD LAW") ||
    window.includes("POSITIVE TREATMENT") ||
    window.includes("SHEPARD") ||
    window.includes("KEYCITE") ||
    window.includes("CONFIDENCE: HIGH")
  ) {
    return "verified";
  }

  // Existence confirmed (CourtListener) — partial verification
  if (window.includes("EXISTENCE CONFIRMED") || window.includes("COURTLISTENER")) {
    return "verified";
  }

  // Also scan the whole doc's Key Authorities section for this specific citation
  const citShort = text.slice(0, 40);
  const authIdx = content.indexOf("## Key Authorities");
  if (authIdx >= 0) {
    const authSection = content.slice(authIdx, content.indexOf("\n## ", authIdx + 5) || undefined).toUpperCase();
    if (authSection.includes(citShort.toUpperCase())) {
      if (authSection.includes("VERIFIED") || authSection.includes("EXISTENCE CONFIRMED")) {
        return "verified";
      }
    }
  }

  // Explicit unverified signals
  if (
    window.includes("UNVERIFIED") ||
    window.includes("TRAINING-DATA-ONLY") ||
    window.includes("TRAINING DATA") ||
    window.includes("CONFIDENCE: LOW")
  ) {
    return "unverified";
  }

  return "unverified";
}

// ── Markdown components ───────────────────────────────────────────────────────

function ConfidenceInline({ level }: { level: string }) {
  const lc = level.toLowerCase();
  return (
    <span
      className={cn(
        "ml-1 inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
        lc === "high" && "bg-emerald-500/15 text-emerald-400",
        lc === "medium" && "bg-amber-500/15 text-amber-400",
        lc === "low" && "bg-red-500/15 text-red-400",
      )}
    >
      {level}
    </span>
  );
}

const remarkPlugins = [remarkGfm];

// ── Status dot ────────────────────────────────────────────────────────────────

function StatusDot({ status }: { status: CitationStatus }) {
  return (
    <span
      className={cn(
        "inline-block h-2 w-2 shrink-0 rounded-full",
        status === "verified" && "bg-emerald-400",
        status === "unverified" && "bg-amber-400",
        status === "flagged" && "bg-red-400",
      )}
      title={status}
    />
  );
}

// ── API helpers ───────────────────────────────────────────────────────────────

async function fetchDocumentContent(projectId: number, taskId: number, path: string, ref?: string): Promise<string> {
  await tokenReady;
  let url = `${apiBase()}/api/projects/${projectId}/documents/${taskId}/content?path=${encodeURIComponent(path)}`;
  if (ref) url += `&ref_name=${encodeURIComponent(ref)}`;
  const res = await fetch(url, { headers: authHeaders() });
  if (!res.ok) throw new Error(`${res.status}`);
  return res.text();
}

async function fetchDocumentVersions(projectId: number, taskId: number, path: string): Promise<DocumentVersion[]> {
  await tokenReady;
  const url = `${apiBase()}/api/projects/${projectId}/documents/${taskId}/versions?path=${encodeURIComponent(path)}`;
  const res = await fetch(url, { headers: authHeaders() });
  if (!res.ok) return [];
  return res.json();
}

// ── Main component ────────────────────────────────────────────────────────────

export function MarkdownLegalViewer({ projectId, taskId, path, defaultTemplateId }: MarkdownLegalViewerProps) {
  const [content, setContent] = useState<string>("");
  const [versions, setVersions] = useState<DocumentVersion[]>([]);
  const [selectedSha, setSelectedSha] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeCitation, setActiveCitation] = useState<string | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportToc, setExportToc] = useState(false);
  const [exportNumbered, setExportNumbered] = useState(false);
  const [exportTemplate, setExportTemplate] = useState<number | null>(defaultTemplateId ?? null);
  const { data: templates = [] } = useTemplates("template");
  const contentRef = useRef<HTMLDivElement>(null);
  const exportRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!exportOpen) return;
    function handleClick(e: MouseEvent) {
      if (exportRef.current && !exportRef.current.contains(e.target as Node)) {
        setExportOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [exportOpen]);

  async function triggerExport(format: "pdf" | "docx") {
    setExportOpen(false);
    setExporting(true);
    try {
      await tokenReady;
      const params = new URLSearchParams({
        path: path,
        format,
      });
      if (selectedSha) params.set("ref_name", selectedSha);
      if (exportToc) params.set("toc", "true");
      if (exportNumbered) params.set("number_sections", "true");
      if (exportTemplate) params.set("template_id", String(exportTemplate));
      const url = `${apiBase()}/api/projects/${projectId}/documents/${taskId}/export?${params}`;
      const res = await fetch(url, { headers: authHeaders() });
      if (!res.ok) {
        const text = await res.text();
        alert(`Export failed: ${text || res.status}`);
        return;
      }
      const blob = await res.blob();
      const blobUrl = URL.createObjectURL(blob);
      const a = document.createElement("a");
      const stem =
        path
          .split("/")
          .pop()
          ?.replace(/\.\w+$/, "") ?? "document";
      a.href = blobUrl;
      a.download = `${stem}.${format}`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(blobUrl);
    } finally {
      setExporting(false);
    }
  }

  useEffect(() => {
    fetchDocumentVersions(projectId, taskId, path)
      .then(setVersions)
      .catch(() => setVersions([]));
  }, [projectId, taskId, path]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    const ref = selectedSha || undefined;
    fetchDocumentContent(projectId, taskId, path, ref)
      .then((text) => {
        setContent(text);
        setLoading(false);
      })
      .catch((e) => {
        setError(e.message || "Failed to load document");
        setLoading(false);
      });
  }, [projectId, taskId, path, selectedSha]);

  const isPrivileged = useMemo(() => /PRIVILEGED\s+AND\s+CONFIDENTIAL/i.test(content), [content]);

  const citations = useMemo(() => extractCitations(content), [content]);

  function scrollToCitation(citation: Citation) {
    setActiveCitation(citation.anchorId);
    if (!contentRef.current) return;

    const allText = contentRef.current.querySelectorAll("p, li, blockquote, td");
    for (const el of allText) {
      if (el.textContent?.includes(citation.text.slice(0, 30))) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
        (el as HTMLElement).classList.add("citation-highlight");
        setTimeout(() => (el as HTMLElement).classList.remove("citation-highlight"), 2000);
        break;
      }
    }
  }

  const mdComponents = useMemo(
    () => ({
      p: ({ children }: { children?: React.ReactNode }) => (
        <ParagraphNode citations={citations}>{children}</ParagraphNode>
      ),
      blockquote: ({ children }: { children?: React.ReactNode }) => (
        <blockquote className="my-3 border-l-2 border-blue-500/40 bg-blue-500/[0.04] py-2 pl-4 text-[13px] italic text-zinc-400">
          {children}
        </blockquote>
      ),
      h1: ({ children }: { children?: React.ReactNode }) => (
        <h1 className="mb-3 mt-6 border-b border-white/[0.08] pb-2 text-[18px] font-semibold text-zinc-100">
          {children}
        </h1>
      ),
      h2: ({ children }: { children?: React.ReactNode }) => (
        <h2 className="mb-2 mt-5 text-[15px] font-semibold text-zinc-200">{children}</h2>
      ),
      h3: ({ children }: { children?: React.ReactNode }) => (
        <h3 className="mb-2 mt-4 text-[13px] font-semibold text-zinc-300">{children}</h3>
      ),
      table: ({ children }: { children?: React.ReactNode }) => (
        <div className="my-3 overflow-x-auto rounded border border-white/[0.08]">
          <table className="w-full text-[12px]">{children}</table>
        </div>
      ),
      th: ({ children }: { children?: React.ReactNode }) => (
        <th className="border-b border-white/[0.1] bg-white/[0.04] px-3 py-2 text-left font-medium text-zinc-400">
          {children}
        </th>
      ),
      td: ({ children }: { children?: React.ReactNode }) => (
        <td className="border-b border-white/[0.05] px-3 py-2 text-zinc-300">{children}</td>
      ),
      strong: ({ children }: { children?: React.ReactNode }) => (
        <strong className="font-semibold text-zinc-200">{children}</strong>
      ),
      a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer"
          className="text-blue-400 underline underline-offset-2 hover:text-blue-300"
        >
          {children}
        </a>
      ),
      ul: ({ children }: { children?: React.ReactNode }) => (
        <ul className="my-2 space-y-1 pl-5 text-[13px] text-zinc-300 [&_li]:list-disc">{children}</ul>
      ),
      ol: ({ children }: { children?: React.ReactNode }) => (
        <ol className="my-2 space-y-1 pl-5 text-[13px] text-zinc-300 [&_li]:list-decimal">{children}</ol>
      ),
    }),
    [citations],
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Header bar */}
      <div className="flex shrink-0 items-center gap-3 border-b border-white/[0.06] px-4 py-2.5">
        <span className="truncate text-[12px] font-medium text-zinc-300">{path}</span>
        {versions.length > 0 && (
          <select
            value={selectedSha}
            onChange={(e) => setSelectedSha(e.target.value)}
            className="ml-auto shrink-0 rounded border border-white/[0.08] bg-white/[0.04] px-2 py-1 text-[11px] text-zinc-400 outline-none focus:border-blue-500/40"
          >
            <option value="">Latest</option>
            {versions.map((v) => (
              <option key={v.sha} value={v.sha}>
                {v.sha.slice(0, 7)} — {v.message.slice(0, 40)}
                {v.message.length > 40 ? "…" : ""} ({v.date.slice(0, 10)})
              </option>
            ))}
          </select>
        )}
        {/* Export dropdown */}
        <div ref={exportRef} className={cn("relative shrink-0", versions.length === 0 && "ml-auto")}>
          <button
            onClick={() => setExportOpen((v) => !v)}
            disabled={exporting || loading}
            className="flex items-center gap-1.5 rounded border border-white/[0.08] bg-white/[0.04] px-2.5 py-1 text-[11px] text-zinc-400 transition-colors hover:border-white/[0.14] hover:text-zinc-300 disabled:opacity-50"
          >
            {exporting ? (
              <span className="h-3 w-3 animate-spin rounded-full border border-zinc-500 border-t-zinc-300" />
            ) : (
              <svg className="h-3 w-3" fill="none" viewBox="0 0 16 16" stroke="currentColor" strokeWidth={1.5}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M4 8v5h8V8M8 2v8M5.5 7.5 8 10l2.5-2.5" />
              </svg>
            )}
            Export
            <svg
              className="h-2.5 w-2.5 opacity-60"
              fill="none"
              viewBox="0 0 10 10"
              stroke="currentColor"
              strokeWidth={1.5}
            >
              <path strokeLinecap="round" d="M2 3.5 5 6.5 8 3.5" />
            </svg>
          </button>
          {exportOpen && (
            <div className="absolute right-0 top-full z-50 mt-1 w-56 overflow-hidden rounded border border-white/[0.1] bg-zinc-900 shadow-xl">
              <div className="border-b border-white/[0.06] px-3 py-2 space-y-1.5">
                {templates.length > 0 && (
                  <div>
                    <label className="text-[10px] text-zinc-500 block mb-0.5">Template</label>
                    <select
                      value={exportTemplate ?? ""}
                      onChange={(e) => setExportTemplate(e.target.value ? Number(e.target.value) : null)}
                      className="w-full rounded border border-white/[0.08] bg-zinc-800 px-1.5 py-1 text-[11px] text-zinc-300 outline-none focus:border-blue-500/40"
                    >
                      <option value="">None (default styling)</option>
                      {templates.map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.file_name}
                          {t.description ? ` — ${t.description}` : ""}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
                <label className="flex items-center gap-2 text-[11px] text-zinc-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={exportToc}
                    onChange={(e) => setExportToc(e.target.checked)}
                    className="rounded"
                  />
                  Table of Contents
                </label>
                <label className="flex items-center gap-2 text-[11px] text-zinc-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={exportNumbered}
                    onChange={(e) => setExportNumbered(e.target.checked)}
                    className="rounded"
                  />
                  Numbered sections
                </label>
              </div>
              <button
                onClick={() => triggerExport("pdf")}
                className="flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-zinc-300 transition-colors hover:bg-white/[0.06]"
              >
                Export as PDF
              </button>
              <button
                onClick={() => triggerExport("docx")}
                className="flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-zinc-300 transition-colors hover:bg-white/[0.06]"
              >
                Export as DOCX
                {exportTemplate && <span className="text-[9px] text-violet-400 ml-auto">with template</span>}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Privilege banner */}
      {isPrivileged && (
        <div className="shrink-0 bg-red-900/40 px-4 py-2 text-center text-[11px] font-semibold uppercase tracking-widest text-red-300 ring-1 ring-inset ring-red-500/30">
          Privileged and Confidential — Attorney-Client Communication
        </div>
      )}

      {/* Body: document + sidebar */}
      <div className="flex min-h-0 flex-1">
        {/* Document content */}
        <ScrollArea className="min-w-0 flex-1">
          <div className="px-6 py-5">
            {loading && (
              <div className="flex h-40 items-center justify-center">
                <div className="h-5 w-5 animate-spin rounded-full border-2 border-zinc-600 border-t-zinc-300" />
              </div>
            )}
            {error && (
              <div className="rounded border border-red-500/20 bg-red-500/[0.05] p-4 text-[12px] text-red-400">
                {error}
              </div>
            )}
            {!loading && !error && (
              <div
                ref={contentRef}
                className="[&_.citation-highlight]:bg-amber-500/20 [&_.citation-highlight]:transition-colors"
              >
                <ConfidenceAwareMarkdown content={content} components={mdComponents} />
              </div>
            )}
          </div>
        </ScrollArea>

        {/* Citation sidebar */}
        {citations.length > 0 && (
          <div className="flex w-[280px] shrink-0 flex-col border-l border-white/[0.06]">
            <div className="shrink-0 border-b border-white/[0.06] px-3 py-2.5">
              <div className="text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                Citations ({citations.length})
              </div>
              <div className="mt-1.5 flex items-center gap-3 text-[10px] text-zinc-600">
                <span className="flex items-center gap-1">
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                  verified
                </span>
                <span className="flex items-center gap-1">
                  <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                  unverified
                </span>
                <span className="flex items-center gap-1">
                  <span className="h-1.5 w-1.5 rounded-full bg-red-400" />
                  flagged
                </span>
              </div>
            </div>
            <ScrollArea className="flex-1">
              <div className="space-y-px p-2">
                {citations.map((c) => (
                  <button
                    key={c.anchorId}
                    onClick={() => scrollToCitation(c)}
                    className={cn(
                      "flex w-full items-start gap-2.5 rounded px-2 py-2 text-left transition-colors hover:bg-white/[0.05]",
                      activeCitation === c.anchorId && "bg-white/[0.07]",
                    )}
                  >
                    <StatusDot status={c.status} />
                    <span className="text-[11px] leading-snug text-zinc-400">{c.text}</span>
                  </button>
                ))}
              </div>
            </ScrollArea>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Confidence-aware renderer ─────────────────────────────────────────────────

function ParagraphNode({ children, citations: _citations }: { children?: React.ReactNode; citations: Citation[] }) {
  const processed = processConfidenceInChildren(children);
  return <p className="my-2 text-[13px] leading-relaxed text-zinc-300">{processed}</p>;
}

function processConfidenceInChildren(children: React.ReactNode): React.ReactNode {
  if (typeof children === "string") {
    return splitConfidence(children);
  }
  if (Array.isArray(children)) {
    return children.map((child, i) => <span key={i}>{processConfidenceInChildren(child)}</span>);
  }
  return children;
}

function splitConfidence(text: string): React.ReactNode {
  const parts = text.split(/(\bConfidence:\s*(?:High|Medium|Low)\b)/gi);
  if (parts.length === 1) return text;
  return parts.map((part, i) => {
    const m = /^Confidence:\s*(High|Medium|Low)$/i.exec(part);
    if (m) {
      return (
        <span key={i} className="inline-flex items-center gap-1">
          <span className="text-zinc-500">Confidence:</span>
          <ConfidenceInline level={m[1]} />
        </span>
      );
    }
    return <span key={i}>{part}</span>;
  });
}

function ConfidenceAwareMarkdown({ content, components }: { content: string; components: Record<string, unknown> }) {
  return (
    <Markdown remarkPlugins={remarkPlugins} components={components as any}>
      {content}
    </Markdown>
  );
}
