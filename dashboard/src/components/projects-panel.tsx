import { useQueryClient } from "@tanstack/react-query";
import {
  Brain,
  Check,
  ChevronDown,
  ChevronRight,
  Folder,
  Search,
  Trash2,
  User,
  Wrench,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { FtsSearchResult } from "@/lib/api";
import {
  createProject,
  deleteAllKnowledgeFiles,
  deleteAllUserKnowledgeFiles,
  deleteKnowledgeFile,
  deleteUserKnowledgeFile,
  fetchKnowledgeContent,
  fetchUserKnowledgeContent,
  searchDocuments,
  uploadKnowledgeFile,
  uploadUserKnowledgeFile,
  useCustomModes,
  useDeleteProject,
  useKnowledgeFiles,
  useModes,
  useProjects,
  useSharedProjects,
  useUserKnowledgeFiles,
} from "@/lib/api";
import { useDashboardMode } from "@/lib/dashboard-mode";
import type { KnowledgeFile, ProjectDocument } from "@/lib/types";
import { cn } from "@/lib/utils";
import { getVocabulary, useVocabulary } from "@/lib/vocabulary";
import { ChatBody } from "./chat-body";
import {
  downloadFile,
  FileListItem,
  FileListPagination,
  FilePreviewWrapper,
  FileSearchBar,
  FileUploadArea,
  formatFileSize,
  isPreviewable,
} from "./file-list-shared";
import { GitReposPanel } from "./git-repos-panel";
import { ProjectDetail } from "./project-detail";
import { DocumentViewWrapper } from "./project-legal-view";
import { ProjectFileManager } from "./project-file-manager";

const LEGAL_VOCAB = getVocabulary("lawborg");

function isLegalWorkflowMode(mode: { name: string; label?: string; phases: Array<{ name: string }> }): boolean {
  const signature = `${mode.name} ${mode.label ?? ""}`.toLowerCase();
  return (
    mode.name === "lawborg" ||
    mode.name === "legal" ||
    signature.includes("legal") ||
    signature.includes("law") ||
    mode.phases.some((phase) => phase.name === "human_review" || phase.name === "purge")
  );
}

function openPipelinesView() {
  window.dispatchEvent(new CustomEvent("borg:navigate", { detail: { view: "creator" } }));
}

type WorkflowOption = {
  name: string;
  label?: string;
  phases: Array<{ name: string; label: string; priority?: number }>;
};

export function ProjectsPanel() {
  const { data: projects = [], refetch: refetchProjects } = useProjects();
  const { data: sharedProjects = [] } = useSharedProjects();
  const { data: modes = [] } = useModes();
  const { data: customModes = [] } = useCustomModes();
  const vocab = useVocabulary();
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [showMemory, setShowMemory] = useState<false | "org" | "my">(false);
  const [sharedExpanded, setSharedExpanded] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [ftsQuery, setFtsQuery] = useState("");
  const [ftsResults, setFtsResults] = useState<FtsSearchResult[]>([]);
  const [ftsSearching, setFtsSearching] = useState(false);
  const ftsDebounce = useRef<ReturnType<typeof setTimeout>>(null);
  const [newProjectName, setNewProjectName] = useState("");
  const [newProjectMode, setNewProjectMode] = useState("");
  const [newProjectJurisdiction, setNewProjectJurisdiction] = useState("");
  const [showLegalWorkflowPicker, setShowLegalWorkflowPicker] = useState(false);
  const [showLegalMatterDetails, setShowLegalMatterDetails] = useState(false);
  const [creating, setCreating] = useState(false);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const [projectActionError, setProjectActionError] = useState<string | null>(null);
  const deleteMut = useDeleteProject();
  const { isSWE, isLegal } = useDashboardMode();
  const legalWorkflowOptions = useMemo(() => {
    const standard = modes.find((mode) => mode.name === "lawborg" || mode.name === "legal");
    const custom = customModes
      .filter((mode) => mode.category === "Professional Services")
      .map<WorkflowOption>((mode) => ({
        name: mode.name,
        label: mode.label,
        phases: mode.phases.map((phase, index) => ({
          name: phase.name,
          label: phase.label,
          priority: index,
        })),
      }));
    const selectedNonStandard = modes.find(
      (mode) => mode.name === newProjectMode && mode.name !== "lawborg" && mode.name !== "legal",
    );

    const merged: WorkflowOption[] = [];
    if (standard) merged.push({ name: standard.name, label: standard.label, phases: standard.phases });
    if (selectedNonStandard && !custom.some((mode) => mode.name === selectedNonStandard.name)) {
      merged.push({
        name: selectedNonStandard.name,
        label: selectedNonStandard.label,
        phases: selectedNonStandard.phases,
      });
    }
    merged.push(...custom);

    const seen = new Set<string>();
    return merged
      .filter((mode) => {
        if (seen.has(mode.name)) return false;
        seen.add(mode.name);
        return true;
      })
      .sort((a, b) => {
        if (a.name === "lawborg") return -1;
        if (b.name === "lawborg") return 1;
        return (a.label ?? a.name).localeCompare(b.label ?? b.name);
      });
  }, [customModes, modes, newProjectMode]);
  const defaultLegalMode =
    legalWorkflowOptions.find((mode) => mode.name === "lawborg")?.name ?? legalWorkflowOptions[0]?.name ?? "lawborg";
  const selectedLegalWorkflow =
    legalWorkflowOptions.find((mode) => mode.name === newProjectMode) ??
    legalWorkflowOptions.find((mode) => mode.name === defaultLegalMode) ??
    null;
  const currentModeMeta = modes.find((mode) => mode.name === newProjectMode) ?? null;
  const isLegalProjectWorkflow = isLegal || !!(currentModeMeta && isLegalWorkflowMode(currentModeMeta));
  const legalWorkflowTitle =
    selectedLegalWorkflow?.name === "lawborg" || selectedLegalWorkflow?.name === "legal"
      ? "Standard Legal Workflow"
      : (selectedLegalWorkflow?.label ?? "Legal Workflow");

  const filteredProjects = useMemo(() => {
    if (!searchQuery.trim()) return projects;
    const q = searchQuery.toLowerCase();
    return projects.filter((p) => p.name.toLowerCase().includes(q) || p.jurisdiction?.toLowerCase().includes(q));
  }, [projects, searchQuery]);

  const selectedProject = projects.find((p) => p.id === selectedProjectId) ?? projects[0] ?? null;
  const activeProjectId = selectedProject?.id ?? null;
  const [selectedDoc, setSelectedDoc] = useState<ProjectDocument | null>(null);
  const [docViewMode, setDocViewMode] = useState<"view" | "redline">("view");

  useEffect(() => {
    if (!selectedProjectId && projects.length > 0) {
      setSelectedProjectId(projects[0].id);
    }
  }, [projects, selectedProjectId]);

  useEffect(() => {
    if (projectActionError && projects.every((project) => project.id !== confirmDeleteId)) {
      setProjectActionError(null);
    }
  }, [confirmDeleteId, projectActionError, projects]);

  useEffect(() => {
    if (!isLegal) return;
    if (!newProjectMode || !legalWorkflowOptions.some((mode) => mode.name === newProjectMode)) {
      setNewProjectMode(defaultLegalMode);
    }
  }, [defaultLegalMode, isLegal, legalWorkflowOptions, newProjectMode]);

  useEffect(() => {
    setSelectedDoc(null);
    setDocViewMode("view");
  }, []);

  useEffect(() => {
    if (activeProjectId) {
      window.dispatchEvent(new CustomEvent("borg:project-selected", { detail: activeProjectId }));
    }
  }, [activeProjectId]);

  useEffect(() => {
    const hash = window.location.hash || "";
    const queryIdx = hash.indexOf("?");
    if (queryIdx < 0) return;
    const params = new URLSearchParams(hash.slice(queryIdx + 1));
    const projectIdParam = params.get("project_id");
    if (!(params.get("cloud_connected") || params.get("cloud_error"))) return;
    if (projectIdParam) {
      const pid = Number(projectIdParam);
      if (Number.isFinite(pid)) setSelectedProjectId(pid);
    }
  }, []);

  function handleFtsSearch(q: string) {
    setFtsQuery(q);
    if (ftsDebounce.current) clearTimeout(ftsDebounce.current);
    if (!q.trim()) {
      setFtsResults([]);
      return;
    }
    ftsDebounce.current = setTimeout(async () => {
      setFtsSearching(true);
      try {
        const results = await searchDocuments(q.trim());
        setFtsResults(results);
      } catch {
        setFtsResults([]);
      } finally {
        setFtsSearching(false);
      }
    }, 300);
  }

  async function handleCreateProject() {
    const name = newProjectName.trim();
    if (!name || creating) return;
    setCreating(true);
    setProjectActionError(null);
    try {
      const opts = newProjectJurisdiction.trim() ? { jurisdiction: newProjectJurisdiction.trim() } : {};
      const effectiveMode = isLegal
        ? newProjectMode || defaultLegalMode
        : isSWE
          ? newProjectMode || "general"
          : "general";
      const created = await createProject(name, effectiveMode, opts);
      setNewProjectName("");
      setNewProjectJurisdiction("");
      setShowLegalWorkflowPicker(false);
      setShowLegalMatterDetails(false);
      await refetchProjects();
      setSelectedProjectId(created.id);
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="flex h-full min-h-0">
      <div className="flex w-[310px] shrink-0 flex-col border-r border-[#2a2520] bg-[#0f0e0c] p-4">
        <div className="mb-3">
          <span className="text-[12px] font-semibold uppercase tracking-wide text-[#6b6459]">
            {vocab.projectsLabel}
          </span>
        </div>
        <div className="relative mb-3">
          <Search className="absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[#6b6459]" />
          <input
            value={ftsQuery || searchQuery}
            onChange={(e) => {
              const v = e.target.value;
              setSearchQuery(v);
              handleFtsSearch(v);
            }}
            placeholder={`Search ${vocab.projectPlural} & documents...`}
            className="w-full rounded-xl border border-[#2a2520] bg-[#1c1a17] pl-8 pr-3 py-2.5 text-[13px] text-[#e8e0d4] outline-none placeholder:text-[#6b6459] focus:border-amber-500/30"
          />
        </div>
        {/* Knowledge — org-wide + personal */}
        <button
          onClick={() => {
            setShowMemory("org");
            setSelectedProjectId(null);
          }}
          className={cn(
            "mb-1 flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-[13px] transition-colors",
            showMemory === "org"
              ? "bg-violet-500/[0.08] text-[#e8e0d4] font-medium ring-1 ring-violet-500/20"
              : "text-[#9c9486] hover:bg-[#1c1a17]",
          )}
        >
          <Brain className={cn("h-4 w-4 shrink-0", showMemory === "org" ? "text-violet-400" : "text-[#6b6459]")} />
          <span>Org Knowledge</span>
        </button>
        <button
          onClick={() => {
            setShowMemory("my");
            setSelectedProjectId(null);
          }}
          className={cn(
            "mb-2 flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-[13px] transition-colors",
            showMemory === "my"
              ? "bg-amber-500/[0.08] text-[#e8e0d4] font-medium ring-1 ring-amber-500/20"
              : "text-[#9c9486] hover:bg-[#1c1a17]",
          )}
        >
          <User className={cn("h-4 w-4 shrink-0", showMemory === "my" ? "text-amber-400" : "text-[#6b6459]")} />
          <span>My Knowledge</span>
        </button>
        <div className="mb-2 h-px bg-[#2a2520]" />

        {ftsQuery.trim() && (ftsSearching || ftsResults.length > 0) ? (
          <div className="min-h-0 flex-1 space-y-1.5 overflow-y-auto mb-3">
            {ftsSearching && <div className="text-[11px] text-[#6b6459] px-1">Searching...</div>}
            {ftsResults.map((r, i) => (
              <button
                key={`${r.task_id}-${r.file_path}-${i}`}
                onClick={() => {
                  setSelectedProjectId(r.project_id);
                  setShowMemory(false);
                  setSearchQuery("");
                  setFtsQuery("");
                  setFtsResults([]);
                }}
                className="w-full rounded-xl border border-[#2a2520] bg-[#1c1a17] px-3 py-2.5 text-left hover:bg-[#232019] transition-colors"
              >
                <div className="text-[11px] text-[#6b6459] truncate flex items-center gap-1.5">
                  {r.project_name}
                  {r.source === "semantic" && (
                    <span className="px-1.5 py-0.5 rounded-lg bg-violet-900/50 text-violet-300 text-[10px]">
                      semantic
                    </span>
                  )}
                </div>
                {r.title_snippet && <div className="text-[12px] text-[#e8e0d4] truncate mt-0.5">{r.title_snippet}</div>}
                <div className="text-[11px] text-[#9c9486] line-clamp-2 mt-0.5">{r.content_snippet}</div>
              </button>
            ))}
            {!ftsSearching && ftsResults.length === 0 && (
              <div className="text-[11px] text-[#6b6459] px-1">No results.</div>
            )}
          </div>
        ) : (
          <>
            <div className="min-h-0 flex-1 space-y-1 overflow-y-auto">
              {filteredProjects.map((p) => (
                <div key={p.id} className="group/item relative">
                  {confirmDeleteId === p.id ? (
                    <div className="flex items-center gap-1.5 rounded-xl bg-red-500/[0.08] px-3 py-2.5 ring-1 ring-red-500/20">
                      <span className="min-w-0 flex-1 truncate text-[12px] text-red-300">Delete "{p.name}"?</span>
                      <button
                        onClick={async () => {
                          setProjectActionError(null);
                          try {
                            await deleteMut.mutateAsync(p.id);
                            setConfirmDeleteId(null);
                            if (selectedProjectId === p.id) setSelectedProjectId(null);
                          } catch (err) {
                            setProjectActionError(err instanceof Error ? err.message : "Failed to delete matter");
                          }
                        }}
                        disabled={deleteMut.isPending}
                        className="shrink-0 rounded-lg bg-red-500/20 px-2 py-1 text-[11px] font-medium text-red-300 hover:bg-red-500/30"
                      >
                        {deleteMut.isPending ? "Deleting..." : "Delete"}
                      </button>
                      <button
                        onClick={() => setConfirmDeleteId(null)}
                        className="shrink-0 rounded-lg px-2 py-1 text-[11px] text-[#9c9486] hover:bg-[#1c1a17]"
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => {
                        setSelectedProjectId(p.id);
                        setShowMemory(false);
                      }}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-xl px-3 py-2.5 text-left text-[13px] transition-colors",
                        p.id === activeProjectId && !showMemory
                          ? "bg-amber-500/[0.08] text-[#e8e0d4] font-medium"
                          : "text-[#9c9486] hover:bg-[#1c1a17]",
                      )}
                    >
                      <span className="shrink-0 text-[11px] text-[#6b6459] tabular-nums">#{p.id}</span>
                      <span className="min-w-0 flex-1 truncate">{p.name}</span>
                      <MatterStatusDot counts={p.task_counts} />
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setConfirmDeleteId(p.id);
                        }}
                        className="shrink-0 rounded p-0.5 text-[#6b6459] opacity-0 transition-opacity hover:text-red-400 group-hover/item:opacity-100"
                        title={`Delete ${vocab.projectSingular}`}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </button>
                  )}
                </div>
              ))}
              {projects.length === 0 && (
                <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-[#2a2520] px-4 py-6 text-center">
                  <Folder className="h-6 w-6 text-[#6b6459] mb-2" />
                  <div className="text-[12px] text-[#9c9486]">No {vocab.projectPlural} yet</div>
                  <div className="text-[11px] text-[#6b6459] mt-0.5">Create one below to get started</div>
                </div>
              )}
            </div>

            {sharedProjects.length > 0 && (
              <div className="mt-3 border-t border-[#2a2520] pt-3">
                <button
                  onClick={() => setSharedExpanded((v) => !v)}
                  className="flex w-full items-center gap-1.5 px-1 py-1 text-[11px] font-medium uppercase tracking-[0.1em] text-[#6b6459] hover:text-[#9c9486] transition-colors"
                >
                  <ChevronRight className={cn("h-3 w-3 transition-transform", sharedExpanded && "rotate-90")} />
                  Shared with you
                  <span className="ml-auto rounded-full bg-[#1c1a17] px-1.5 py-0.5 text-[10px] tabular-nums normal-case tracking-normal">
                    {sharedProjects.length}
                  </span>
                </button>
                {sharedExpanded && (
                  <div className="mt-1.5 space-y-1">
                    {sharedProjects.map((sp) => (
                      <button
                        key={sp.id}
                        onClick={() => {
                          setSelectedProjectId(sp.id);
                          setShowMemory(false);
                        }}
                        className={cn(
                          "flex w-full items-center gap-2 rounded-xl px-3 py-2.5 text-left text-[13px] transition-colors",
                          sp.id === activeProjectId && !showMemory
                            ? "bg-blue-500/[0.08] text-[#e8e0d4] font-medium"
                            : "text-[#9c9486] hover:bg-[#1c1a17]",
                        )}
                      >
                        <span className="shrink-0 text-[11px] text-[#6b6459] tabular-nums">#{sp.id}</span>
                        <span className="min-w-0 flex-1 truncate">{sp.name}</span>
                        <span className="shrink-0 rounded bg-[#1c1a17] px-1.5 py-0.5 text-[10px] text-[#6b6459]">
                          {sp.share_role}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
            {projectActionError && (
              <div className="mt-2 rounded-lg border border-red-500/20 bg-red-500/[0.06] px-3 py-2 text-[11px] text-red-300">
                {projectActionError}
              </div>
            )}
          </>
        )}
        <div className="mt-4 shrink-0 border-t border-[#2a2520] pt-4">
          <input
            value={newProjectName}
            onChange={(e) => setNewProjectName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreateProject()}
            placeholder={vocab.newProjectPlaceholder}
            className="w-full rounded-xl border border-[#2a2520] bg-[#1c1a17] px-4 py-2.5 text-[14px] text-[#e8e0d4] outline-none placeholder:text-[#6b6459] focus:border-amber-500/30"
          />
          {/* Mode picker hidden — defaults to "general" */}
          {isLegalProjectWorkflow && (
            <div className="mt-2 rounded-xl border border-[#2a2520] bg-[#151412] px-3 py-2.5">
              <div className="min-w-0">
                <div className="text-[11px] font-medium text-[#e8e0d4]">{legalWorkflowTitle}</div>
                <div className="mt-1 text-[11px] text-[#6b6459]">
                  This {LEGAL_VOCAB.projectSingular} will use this workflow automatically.
                </div>
              </div>
              <div className="mt-2 rounded-lg border border-[#2a2520] bg-[#1c1a17]">
                <button
                  type="button"
                  onClick={() => setShowLegalWorkflowPicker((open) => !open)}
                  className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left transition-colors hover:bg-[#151412]"
                >
                  <span className="min-w-0">
                    <span className="block text-[11px] font-medium text-[#e8e0d4]">Workflow</span>
                    <span className="block text-[10px] text-[#6b6459]">
                      {selectedLegalWorkflow?.label ?? legalWorkflowTitle}
                    </span>
                  </span>
                  <ChevronDown
                    className={cn(
                      "h-3.5 w-3.5 shrink-0 text-[#6b6459] transition-transform",
                      showLegalWorkflowPicker && "rotate-180",
                    )}
                  />
                </button>
                {selectedLegalWorkflow?.phases?.length ? (
                  <div className="border-t border-[#2a2520] px-3 py-2.5">
                    <span className="block text-[10px] font-medium uppercase tracking-[0.14em] text-[#6b6459]">
                      Workflow stages
                    </span>
                    <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                      {selectedLegalWorkflow.phases
                        .slice()
                        .sort(
                          (a, b) => (a.priority ?? Number.MAX_SAFE_INTEGER) - (b.priority ?? Number.MAX_SAFE_INTEGER),
                        )
                        .map((phase, i, arr) => (
                          <span key={phase.name} className="flex items-center">
                            <span className="rounded-lg bg-[#151412] px-2 py-0.5 text-[10px] text-[#9c9486] ring-1 ring-inset ring-[#2a2520]">
                              {LEGAL_VOCAB.statusLabels[phase.name] ?? phase.label ?? phase.name}
                            </span>
                            {i < arr.length - 1 && <span className="mx-1 text-[10px] text-[#6b6459]">→</span>}
                          </span>
                        ))}
                    </div>
                  </div>
                ) : null}
                {showLegalWorkflowPicker && (
                  <div className="border-t border-[#2a2520] px-3 py-2.5">
                    <div className="space-y-1 rounded-lg border border-[#2a2520] bg-[#151412] p-1.5">
                      {legalWorkflowOptions.map((mode) => {
                        const selected = mode.name === (selectedLegalWorkflow?.name ?? defaultLegalMode);
                        return (
                          <button
                            key={mode.name}
                            type="button"
                            onClick={() => {
                              setNewProjectMode(mode.name);
                              setShowLegalWorkflowPicker(false);
                            }}
                            className={cn(
                              "flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left transition-colors",
                              selected ? "bg-amber-500/[0.08] text-[#e8e0d4]" : "text-[#9c9486] hover:bg-[#1c1a17]",
                            )}
                          >
                            <span className="min-w-0">
                              <span className="block truncate text-[11px] font-medium">{mode.label ?? mode.name}</span>
                              <span className="block truncate text-[10px] text-[#6b6459]">{mode.name}</span>
                            </span>
                            {selected && <Check className="h-3.5 w-3.5 shrink-0 text-amber-400" />}
                          </button>
                        );
                      })}
                      <div className="mt-1 rounded-md border border-dashed border-[#2a2520] bg-[#1c1a17] px-2 py-2">
                        <div className="text-[10px] text-[#6b6459]">
                          {legalWorkflowOptions.length > 1
                            ? "Need to edit or add workflows?"
                            : "No custom workflows yet. Create one in Pipelines."}
                        </div>
                        <button
                          type="button"
                          onClick={openPipelinesView}
                          className="mt-2 inline-flex items-center gap-1 rounded-md bg-amber-500/10 px-2 py-1 text-[10px] font-medium text-amber-300 ring-1 ring-inset ring-amber-500/20 transition-colors hover:bg-amber-500/20"
                        >
                          <Wrench className="h-3 w-3" />
                          Open Pipelines
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
              <div className="mt-2 rounded-lg border border-[#2a2520] bg-[#1c1a17]">
                <button
                  type="button"
                  onClick={() => setShowLegalMatterDetails((open) => !open)}
                  className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left transition-colors hover:bg-[#151412]"
                >
                  <span className="min-w-0">
                    <span className="block text-[11px] font-medium text-[#e8e0d4]">Matter details</span>
                    <span className="block text-[10px] text-[#6b6459]">
                      {newProjectJurisdiction.trim()
                        ? `Jurisdiction: ${newProjectJurisdiction.trim()}`
                        : "Jurisdiction is optional. Add it if it helps agents target the right law."}
                    </span>
                  </span>
                  <ChevronDown
                    className={cn(
                      "h-3.5 w-3.5 shrink-0 text-[#6b6459] transition-transform",
                      showLegalMatterDetails && "rotate-180",
                    )}
                  />
                </button>
                {showLegalMatterDetails && (
                  <div className="border-t border-[#2a2520] px-3 py-2.5">
                    <label className="mb-1 block text-[10px] font-medium uppercase tracking-[0.14em] text-[#6b6459]">
                      Jurisdiction (Optional)
                    </label>
                    <input
                      value={newProjectJurisdiction}
                      onChange={(e) => setNewProjectJurisdiction(e.target.value)}
                      placeholder="England & Wales, Delaware, SDNY..."
                      className="w-full rounded-lg border border-[#2a2520] bg-[#151412] px-3 py-2 text-[13px] text-[#e8e0d4] outline-none placeholder:text-[#6b6459] focus:border-amber-500/30"
                    />
                    <div className="mt-1.5 text-[10px] text-[#6b6459]">
                      Helps agents ground research and retrieval. You can also add or edit it later.
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
          <button
            onClick={handleCreateProject}
            disabled={creating || !newProjectName.trim()}
            className="mt-2.5 w-full rounded-lg bg-amber-500/20 px-3 py-2.5 text-[13px] font-medium text-amber-300 hover:bg-amber-500/30 transition-colors disabled:cursor-not-allowed disabled:text-[#6b6459]"
          >
            {creating
              ? "Creating..."
              : `Create ${vocab.projectSingular[0].toUpperCase()}${vocab.projectSingular.slice(1)}`}
          </button>
        </div>
      </div>

      {/* Center: Chat (project view only, not knowledge tabs) */}
      {!isSWE && !showMemory && selectedProject && !selectedDoc && (
        <div className="flex min-w-0 flex-1 flex-col border-r border-[#2a2520]">
          <ChatBody thread={`web:project-${selectedProject?.id}`} className="bg-[#0f0e0c]" />
        </div>
      )}

      {/* Center/main panel */}
      <div
        className={cn(
          "flex flex-col overflow-hidden",
          !isSWE && !showMemory && selectedProject && !selectedDoc ? "w-[525px] shrink-0" : "min-w-0 flex-1",
        )}
      >
        {showMemory === "org" ? (
          <KnowledgeView scope="org" />
        ) : showMemory === "my" ? (
          <KnowledgeView scope="my" />
        ) : !selectedProject ? (
          <div className="flex h-full items-center justify-center">
            <div className="max-w-[360px] text-center">
              <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-[#1c1a17] ring-1 ring-amber-900/20">
                <Folder className="h-7 w-7 text-[#6b6459]" />
              </div>
              <div className="text-[16px] font-semibold text-[#e8e0d4]">Get Started</div>
              <div className="mt-2 text-[13px] leading-relaxed text-[#9c9486]">
                <p>Create a {vocab.projectSingular} in the sidebar to start.</p>
                <p>Each {vocab.projectSingular} gets its own document store and AI agent.</p>
              </div>
              <div className="mt-5 space-y-2.5 text-left text-[13px] text-[#9c9486]">
                <div className="rounded-xl border border-[#2a2520] bg-[#151412] px-4 py-3">
                  <span className="text-[#e8e0d4] font-medium">1.</span> Name your {vocab.projectSingular} and select a
                  mode
                </div>
                <div className="rounded-xl border border-[#2a2520] bg-[#151412] px-4 py-3">
                  <span className="text-[#e8e0d4] font-medium">2.</span> Upload reference documents
                </div>
                <div className="rounded-xl border border-[#2a2520] bg-[#151412] px-4 py-3">
                  <span className="text-[#e8e0d4] font-medium">3.</span> Chat with Borg about your docs
                </div>
              </div>
            </div>
          </div>
        ) : !isSWE ? (
          selectedDoc ? (
            <DocumentViewWrapper
              projectId={selectedProject.id}
              doc={selectedDoc}
              viewMode={docViewMode}
              onBack={() => {
                setSelectedDoc(null);
                setDocViewMode("view");
              }}
              onToggleMode={() => setDocViewMode(docViewMode === "view" ? "redline" : "view")}
              defaultTemplateId={undefined}
            />
          ) : (
            <ProjectDetail
              projectId={selectedProject.id}
              onDocumentSelect={setSelectedDoc}
              onDelete={() => setSelectedProjectId(null)}
            />
          )
        ) : (
          <ProjectFileManager project={selectedProject} />
        )}
      </div>

      {/* Right: Chat panel for knowledge tabs */}
      {!isSWE && showMemory && (
        <div className="flex h-full w-[30vw] shrink-0 flex-col border-l border-[#2a2520] bg-[#0f0e0c] overflow-hidden">
          <ChatBody thread="web:dashboard" className="bg-[#0f0e0c]" />
        </div>
      )}
    </div>
  );
}

function MatterStatusDot({ counts }: { counts?: import("@/lib/types").ProjectTaskCounts }) {
  if (!counts || counts.total === 0) return null;

  if (counts.active > 0) {
    return (
      <span className="relative flex h-2 w-2 shrink-0" title="Agent working">
        <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-amber-400 opacity-75" />
        <span className="relative inline-flex h-2 w-2 rounded-full bg-amber-400" />
      </span>
    );
  }
  if (counts.review > 0) {
    return <span className="h-2 w-2 shrink-0 rounded-full bg-orange-400" title="Needs review" />;
  }
  if (counts.done > 0) {
    return <span className="h-2 w-2 shrink-0 rounded-full bg-emerald-500" title="Complete" />;
  }
  return null;
}

function KnowledgeView({ scope }: { scope: "org" | "my" }) {
  const vocab = useVocabulary();
  const isOrg = scope === "org";
  const queryKey = isOrg ? "knowledge" : "my-knowledge";
  const title = isOrg ? "Org Knowledge" : "My Knowledge";
  const subtitle = isOrg
    ? `Shared across all ${vocab.projectPlural} in this workspace`
    : "Personal knowledge — only your agents see this";
  const emptyTitle = isOrg ? "No org documents yet" : "No personal documents yet";
  const emptySubtitle = isOrg
    ? `Upload files to make them available to all users and ${vocab.projectPlural}`
    : "Upload files that only your agents will use";
  const accentBg = isOrg ? "bg-violet-500/10" : "bg-amber-500/10";
  const accentRing = isOrg ? "ring-violet-500/20" : "ring-amber-500/20";
  const accentText = isOrg ? "text-violet-400" : "text-amber-400";
  const Icon = isOrg ? Brain : User;

  const [search, setSearch] = useState("");
  const [offset, setOffset] = useState(0);
  const [pageSize, setPageSize] = useState(20);
  const orgPage = useKnowledgeFiles(isOrg ? { limit: pageSize, offset, q: search } : undefined);
  const myPage = useUserKnowledgeFiles(!isOrg ? { limit: pageSize, offset, q: search } : undefined);
  const { data: page, isLoading } = isOrg ? orgPage : myPage;
  const files = page?.files ?? [];
  const queryClient = useQueryClient();
  const [previewFile, setPreviewFile] = useState<KnowledgeFile | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deletingAll, setDeletingAll] = useState(false);

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: [queryKey] });
  }

  async function handleUpload(fileList: File[]) {
    for (const file of fileList) {
      if (isOrg) await uploadKnowledgeFile(file, "", false);
      else await uploadUserKnowledgeFile(file, "", false);
    }
    invalidate();
  }

  async function handleDeleteAll() {
    if (deletingAll) return;
    if (!confirm(`Delete all documents in ${title}? This cannot be undone.`)) return;
    setDeleteError(null);
    setDeletingAll(true);
    try {
      if (isOrg) await deleteAllKnowledgeFiles();
      else await deleteAllUserKnowledgeFiles();
      setPreviewFile(null);
      setSearch("");
      setOffset(0);
      invalidate();
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Failed to delete");
    } finally {
      setDeletingAll(false);
    }
  }

  async function handleDeleteOne(file: KnowledgeFile) {
    if (!confirm(`Delete "${file.file_name}"?`)) return;
    if (isOrg) await deleteKnowledgeFile(file.id);
    else await deleteUserKnowledgeFile(file.id);
    invalidate();
  }

  const hasFiles = (page?.total ?? 0) > 0;

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 space-y-3 p-5 pb-3">
        <div className="flex items-center gap-3">
          <div
            className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl ${accentBg} ring-1 ${accentRing}`}
          >
            <Icon className={`h-6 w-6 ${accentText}`} />
          </div>
          <div>
            <div className="text-[16px] font-semibold text-[#e8e0d4]">{title}</div>
            <div className="text-[13px] text-[#6b6459]">{subtitle}</div>
          </div>
        </div>

        <FileUploadArea onUploadFiles={handleUpload} onUploaded={invalidate} subtitle={emptySubtitle} />

        {deleteError && <div className="text-[12px] text-red-400">{deleteError}</div>}

        {hasFiles && (
          <>
            <FileSearchBar
              value={search}
              onChange={(v) => {
                setSearch(v);
                setOffset(0);
              }}
              stats={
                <>
                  {page?.total ?? 0} files {formatFileSize(page?.total_bytes ?? 0)}
                </>
              }
            />
            <FileListPagination
              filePage={{ total: page?.total ?? 0, has_more: page?.has_more ?? false }}
              currentOffset={offset}
              fileCount={files.length}
              pageSize={pageSize}
              onPageSizeChange={(s) => {
                setPageSize(s);
                setOffset(0);
              }}
              canGoPrev={offset > 0}
              onPrev={() => setOffset((prev) => Math.max(0, prev - pageSize))}
              canGoNext={page?.has_more ?? false}
              onNext={() => setOffset((prev) => prev + pageSize)}
              actions={
                <button
                  type="button"
                  onClick={handleDeleteAll}
                  disabled={deletingAll}
                  className="inline-flex items-center gap-1.5 rounded-lg border border-red-500/20 bg-red-500/[0.08] px-3 py-1.5 text-[12px] font-medium text-red-300 transition-colors hover:bg-red-500/[0.14] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  {deletingAll ? "Deleting..." : "Delete All"}
                </button>
              }
            />
          </>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5 space-y-4">
        <GitReposPanel isOrg={isOrg} accentBg={accentBg} accentText={accentText} />

        {/* Files section */}
        <div className="space-y-1.5">
          {isLoading && (
            <div className="flex items-center justify-center py-12">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-zinc-700 border-t-zinc-400" />
            </div>
          )}

          {!isLoading && files.length === 0 && !hasFiles && !search && (
            <div className="flex flex-col items-center py-12 text-center">
              <div
                className={`mb-4 flex h-14 w-14 items-center justify-center rounded-2xl ${accentBg} ring-1 ${accentRing}`}
              >
                <Icon className={`h-6 w-6 ${accentText}`} />
              </div>
              <p className="text-[14px] text-[#9c9486]">{emptyTitle}</p>
              <p className="mt-1 text-[12px] text-[#6b6459]">{emptySubtitle}</p>
            </div>
          )}

          {!isLoading && files.length === 0 && search && (
            <div className="rounded-xl border border-dashed border-[#2a2520] px-4 py-4 text-[12px] text-[#6b6459] text-center">
              No files match the current filter.
            </div>
          )}

          {files.map((file, i) => (
            <FileListItem
              key={file.id}
              file={file}
              index={offset + i + 1}
              onClick={isPreviewable(file) ? () => setPreviewFile(file) : undefined}
              onDownload={() => downloadFile(
                isOrg ? fetchKnowledgeContent : fetchUserKnowledgeContent,
                file,
              )}
              onDelete={() => handleDeleteOne(file)}
            />
          ))}
        </div>
      </div>

      <FilePreviewWrapper
        file={previewFile}
        fetchContent={isOrg ? fetchKnowledgeContent : fetchUserKnowledgeContent}
        onClose={() => setPreviewFile(null)}
      />
    </div>
  );
}
