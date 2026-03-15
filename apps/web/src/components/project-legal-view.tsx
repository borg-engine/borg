import { ArrowLeft } from "lucide-react";
import { useProjectDocumentVersions } from "@/lib/api";
import type { ProjectDocument } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useVocabulary } from "@/lib/vocabulary";
import { MarkdownLegalViewer } from "./viewers/markdown-legal-viewer";
import { RedlineViewer } from "./viewers/redline-viewer";

export function DocumentViewWrapper({
  projectId,
  doc,
  viewMode,
  onBack,
  onToggleMode,
  defaultTemplateId,
}: {
  projectId: number;
  doc: ProjectDocument;
  viewMode: "view" | "redline";
  onBack: () => void;
  onToggleMode: () => void;
  defaultTemplateId?: number | null;
}) {
  const { data: versions = [] } = useProjectDocumentVersions(projectId, doc.task_id, doc.file_name);
  const vocab = useVocabulary();

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-white/[0.07] px-4 py-3">
        <button
          onClick={onBack}
          className="flex items-center gap-1.5 text-[12px] text-zinc-400 hover:text-zinc-200 transition-colors"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          Back to {vocab.projectSingular}
        </button>
        <span className="text-[12px] text-zinc-600">&middot;</span>
        <span className="truncate text-[12px] text-zinc-400">{doc.file_name}</span>
        {versions.length >= 2 && (
          <button
            onClick={onToggleMode}
            className={cn(
              "ml-auto rounded-lg border px-3 py-1 text-[12px] font-medium transition-colors",
              viewMode === "redline"
                ? "border-blue-500/30 bg-blue-500/10 text-blue-400"
                : "border-white/[0.08] text-zinc-400 hover:border-white/[0.14] hover:text-zinc-200",
            )}
          >
            {viewMode === "redline" ? "Document View" : "Compare Versions"}
          </button>
        )}
      </div>
      <div className="min-h-0 flex-1">
        {viewMode === "redline" && versions.length >= 2 ? (
          <RedlineViewer projectId={projectId} taskId={doc.task_id} path={doc.file_name} versions={versions} />
        ) : (
          <MarkdownLegalViewer
            projectId={projectId}
            taskId={doc.task_id}
            path={doc.file_name}
            defaultTemplateId={defaultTemplateId}
          />
        )}
      </div>
    </div>
  );
}
