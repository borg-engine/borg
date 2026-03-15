import { ChevronDown, FolderOpen, Globe } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useProjects } from "@/lib/api";
import { useDashboardMode } from "@/lib/dashboard-mode";
import { cn } from "@/lib/utils";
import { ChatBody } from "./chat-body";

function threadLabel(id: string, projects: { id: number; name: string }[]): string {
  if (id === "web:dashboard") return "Global";
  const match = id.match(/^web:project-(\d+)$/);
  if (match) {
    const proj = projects.find((p) => p.id === Number(match[1]));
    return proj?.name ?? `#${match[1]}`;
  }
  return id.replace("web:", "");
}

interface ChatDrawerProps {
  defaultThread?: string;
  view?: string;
}

export function ChatDrawer({ defaultThread = "web:dashboard", view }: ChatDrawerProps) {
  const { data: projects = [] } = useProjects();
  const { isSWE } = useDashboardMode();
  const [thread, setThread] = useState(defaultThread);

  useEffect(() => {
    function handleProjectSelected(e: Event) {
      const id = (e as CustomEvent).detail;
      if (typeof id === "number") {
        setThread(`web:project-${id}`);
      }
    }
    window.addEventListener("borg:project-selected", handleProjectSelected);
    return () => window.removeEventListener("borg:project-selected", handleProjectSelected);
  }, []);

  const scopeLabel = threadLabel(thread, projects);

  const [threadPickerOpen, setThreadPickerOpen] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!threadPickerOpen) return;
    function close(e: MouseEvent) {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) setThreadPickerOpen(false);
    }
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [threadPickerOpen]);

  const threadOptions = useMemo(() => {
    const opts: { id: string; label: string; icon: "globe" | "folder" }[] = [
      { id: "web:dashboard", label: "Global", icon: "globe" },
    ];
    for (const p of projects) {
      opts.push({ id: `web:project-${p.id}`, label: p.name, icon: "folder" });
    }
    return opts;
  }, [projects]);

  // Hide when projects view is active in non-SWE mode (chat is embedded in ProjectsPanel)
  if (view === "projects" && !isSWE) return null;

  return (
    <div className="hidden md:flex h-full md:w-[40vw] lg:w-[30vw] min-w-[300px] max-w-[500px] shrink-0 flex-col border-l border-[var(--color-border)] bg-[var(--color-bg)] overflow-hidden">
      <div className="min-h-0 flex-1">
        <ChatBody thread={thread} />
      </div>

      {/* Thread picker */}
      <div className="shrink-0 border-t border-[var(--color-border)] bg-[var(--color-bg)]/90 px-3 py-1.5">
        <div className="relative flex items-center" ref={pickerRef}>
          <button
            onClick={() => setThreadPickerOpen((v) => !v)}
            className="flex items-center gap-1.5 rounded-lg px-2 py-1 text-[12px] text-[var(--color-text-secondary)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)] transition-colors"
          >
            {thread === "web:dashboard" ? (
              <Globe className="h-3.5 w-3.5 text-amber-400/60" />
            ) : (
              <FolderOpen className="h-3.5 w-3.5 text-[var(--color-text-tertiary)]" />
            )}
            <span className="truncate">{scopeLabel} Chat</span>
            <ChevronDown className={cn("h-3 w-3 transition-transform", threadPickerOpen && "rotate-180")} />
          </button>
          {threadPickerOpen && (
            <div className="absolute bottom-full left-0 mb-1 w-56 max-h-[320px] overflow-y-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] py-1 shadow-xl z-50">
              {threadOptions.map((opt) => (
                <button
                  key={opt.id}
                  onClick={() => {
                    setThread(opt.id);
                    setThreadPickerOpen(false);
                  }}
                  className={cn(
                    "flex w-full items-center gap-2 px-3 py-1.5 text-[12px] transition-colors hover:bg-[var(--color-card-alt)]",
                    thread === opt.id ? "text-amber-400" : "text-[var(--color-text-secondary)]",
                  )}
                >
                  {opt.icon === "globe" ? <Globe className="h-3.5 w-3.5" /> : <FolderOpen className="h-3.5 w-3.5" />}
                  <span className="truncate">{opt.label} Chat</span>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
