import { useState, useRef, useEffect, useMemo } from "react";
import { ChevronDown, Globe, FolderOpen } from "lucide-react";
import { cn } from "@/lib/utils";
import { useProjects } from "@/lib/api";
import { useDashboardMode } from "@/lib/dashboard-mode";
import { ChatBody } from "./chat-body";

function threadLabel(id: string, projects: { id: number; name: string }[]): string {
  if (id === "web:dashboard") return "Global";
  const match = id.match(/^web:project-(\d+)$/);
  if (match) {
    const proj = projects.find((p) => p.id === Number(match[1]));
    return proj?.name ?? `Project #${match[1]}`;
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
    <div className="flex h-full w-[30vw] shrink-0 flex-col border-l border-[#2a2520] bg-[#0f0e0c] overflow-hidden">
      <div className="min-h-0 flex-1">
        <ChatBody thread={thread} />
      </div>

      {/* Thread picker */}
      <div className="shrink-0 border-t border-[#2a2520] bg-[#0f0e0c]/90 px-3 py-1.5">
        <div className="relative flex items-center" ref={pickerRef}>
          <button
            onClick={() => setThreadPickerOpen((v) => !v)}
            className="flex items-center gap-1.5 rounded-lg px-2 py-1 text-[12px] text-[#9c9486] hover:bg-[#1c1a17] hover:text-[#e8e0d4] transition-colors"
          >
            {thread === "web:dashboard" ? (
              <Globe className="h-3.5 w-3.5 text-amber-400/60" />
            ) : (
              <FolderOpen className="h-3.5 w-3.5 text-[#6b6459]" />
            )}
            <span className="truncate">{scopeLabel} Chat</span>
            <ChevronDown className={cn("h-3 w-3 transition-transform", threadPickerOpen && "rotate-180")} />
          </button>
          {threadPickerOpen && (
            <div className="absolute bottom-full left-0 mb-1 w-56 max-h-[320px] overflow-y-auto rounded-lg border border-[#2a2520] bg-[#1c1a17] py-1 shadow-xl z-50">
              {threadOptions.map((opt) => (
                <button
                  key={opt.id}
                  onClick={() => {
                    setThread(opt.id);
                    setThreadPickerOpen(false);
                  }}
                  className={cn(
                    "flex w-full items-center gap-2 px-3 py-1.5 text-[12px] transition-colors hover:bg-[#232019]",
                    thread === opt.id ? "text-amber-400" : "text-[#9c9486]",
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
