import { FileText, Folder, RotateCw, Trash2, Upload } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { UploadSession } from "@/lib/api";
import {
  completeProjectUploadSession,
  createProjectUploadSession,
  deleteAllProjectFiles,
  deleteProjectFile,
  fetchProjectFileContent,
  fetchProjectFileText,
  getProjectUploadSessionStatus,
  listProjectUploadSessions,
  reextractProjectFile,
  retryProjectUploadSession,
  uploadProjectUploadChunk,
  useSettings,
} from "@/lib/api";
import type { Project } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useVocabulary } from "@/lib/vocabulary";
import {
  FileListPagination,
  FilePreviewWrapper,
  FileSearchBar,
  formatFileSize,
  isPreviewable,
  useFileList,
  useFilePreview,
} from "./file-list-shared";
import { ProjectCloudStorageSection, UploadSessionsSection } from "./project-cloud-storage";

const RESUMABLE_UPLOAD_CHUNK_SIZE = 8 * 1024 * 1024;
const RESUMABLE_UPLOAD_PARALLEL_CHUNKS = 4;
const RESUMABLE_UPLOAD_CHUNK_RETRIES = 3;
const UPLOAD_SESSION_KEY_PREFIX = "borg-upload-session";

type FileUploadProgress = {
  id: string;
  fileName: string;
  uploadedBytes: number;
  totalBytes: number;
  status: "starting" | "uploading" | "processing" | "done" | "failed";
  sessionId?: number;
  error?: string;
};

export function ProjectFileManager({ project }: { project: Project }) {
  const vocab = useVocabulary();
  const { data: settings } = useSettings();
  const activeProjectId = project.id;

  const fl = useFileList(activeProjectId);
  const { filePage, files, filesLoading, fileSearch, setFileSearch, refetchFiles, resetPagination } = fl;
  const fileSummary = filePage?.summary;
  const { previewFile, setPreviewFile } = useFilePreview();
  const [uploading, setUploading] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [deleteFilesError, setDeleteFilesError] = useState<string | null>(null);
  const [deletingAllFiles, setDeletingAllFiles] = useState(false);
  const [textViewFile, setTextViewFile] = useState<{ id: number; name: string; text: string } | null>(null);
  const [extracting, setExtracting] = useState<number | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploadSessions, setUploadSessions] = useState<UploadSession[]>([]);
  const [uploadSessionCounts, setUploadSessionCounts] = useState<Record<string, number>>({});
  const [uploadSessionsLoading, setUploadSessionsLoading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<FileUploadProgress[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const dropRef = useRef<HTMLDivElement>(null);

  const totalBytes = fileSummary?.total_bytes ?? 0;
  const projectMaxBytes = Math.max(1, settings?.project_max_bytes ?? 100 * 1024 * 1024);

  const updateUploadProgress = useCallback((id: string, patch: Partial<FileUploadProgress>) => {
    setUploadProgress((prev) => prev.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)));
  }, []);

  const refreshUploadSessions = useCallback(async () => {
    if (!activeProjectId) return;
    const data = await listProjectUploadSessions(activeProjectId, 30);
    setUploadSessions(data.sessions || []);
    setUploadSessionCounts(data.counts || {});
  }, [activeProjectId]);

  useEffect(() => {
    if (!activeProjectId) return;
    let cancelled = false;
    const load = async () => {
      setUploadSessionsLoading(true);
      try {
        const data = await listProjectUploadSessions(activeProjectId, 30);
        if (cancelled) return;
        setUploadSessions(data.sessions || []);
        setUploadSessionCounts(data.counts || {});
      } finally {
        if (!cancelled) setUploadSessionsLoading(false);
      }
    };
    load();
    const t = setInterval(load, 5000);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [activeProjectId]);

  function uploadSessionStorageKey(projectId: number, file: File): string {
    return `${UPLOAD_SESSION_KEY_PREFIX}:${projectId}:${file.name}:${file.size}:${file.lastModified}`;
  }

  function buildChunkQueueFromRanges(ranges: Array<[number, number]>, totalChunks: number): number[] {
    const queue: number[] = [];
    if (ranges.length === 0) {
      for (let idx = 0; idx < totalChunks; idx += 1) queue.push(idx);
      return queue;
    }
    for (const [startRaw, endRaw] of ranges) {
      const start = Math.max(0, startRaw);
      const end = Math.min(totalChunks - 1, endRaw);
      for (let idx = start; idx <= end; idx += 1) queue.push(idx);
    }
    return queue;
  }

  async function uploadChunkQueue(
    projectId: number,
    sessionId: number,
    file: File,
    chunkSize: number,
    queue: number[],
    onChunkUploaded: (bytes: number) => void,
  ) {
    const workerCount = Math.min(RESUMABLE_UPLOAD_PARALLEL_CHUNKS, queue.length);
    await Promise.all(
      Array.from({ length: workerCount }, async () => {
        while (true) {
          const chunkIndex = queue.shift();
          if (chunkIndex === undefined) return;
          const start = chunkIndex * chunkSize;
          const end = Math.min(start + chunkSize, file.size);
          const blob = file.slice(start, end);
          let uploaded = false;
          let lastErr: unknown = null;
          for (let attempt = 1; attempt <= RESUMABLE_UPLOAD_CHUNK_RETRIES; attempt += 1) {
            try {
              await uploadProjectUploadChunk(projectId, sessionId, chunkIndex, blob);
              uploaded = true;
              break;
            } catch (err) {
              lastErr = err;
              if (attempt < RESUMABLE_UPLOAD_CHUNK_RETRIES) {
                await new Promise((resolve) => setTimeout(resolve, attempt * 500));
              }
            }
          }
          if (!uploaded) {
            throw lastErr instanceof Error ? lastErr : new Error("chunk upload failed");
          }
          onChunkUploaded(blob.size);
        }
      }),
    );
  }

  async function handleUpload(filesToUpload: FileList | File[]) {
    if (!activeProjectId || uploading) return;
    setUploading(true);
    setUploadError(null);
    const fileArr = Array.from(filesToUpload).filter((file) => file.size > 0);
    if (fileArr.length === 0) {
      setUploadError("No non-empty files selected.");
      setUploading(false);
      return;
    }
    const startingProgress: FileUploadProgress[] = fileArr.map((file, idx) => ({
      id: `${Date.now()}-${idx}-${file.name}`,
      fileName: file.name,
      totalBytes: file.size,
      uploadedBytes: 0,
      status: "starting",
    }));
    setUploadProgress(startingProgress);
    const fileFailures: Array<{ fileName: string; error: string }> = [];
    try {
      for (let fileIndex = 0; fileIndex < fileArr.length; fileIndex += 1) {
        const file = fileArr[fileIndex];
        const progressId = startingProgress[fileIndex]?.id ?? `${Date.now()}-${fileIndex}-${file.name}`;
        try {
          const chunkSize = RESUMABLE_UPLOAD_CHUNK_SIZE;
          const totalChunks = Math.max(1, Math.ceil(file.size / chunkSize));
          const sessionKey = uploadSessionStorageKey(activeProjectId, file);
          let sessionId = Number(localStorage.getItem(sessionKey) || "");
          let status = null as Awaited<ReturnType<typeof getProjectUploadSessionStatus>> | null;

          if (!(Number.isFinite(sessionId) && sessionId > 0)) {
            sessionId = 0;
          } else {
            try {
              status = await getProjectUploadSessionStatus(activeProjectId, sessionId);
              if (status.session.status !== "uploading") {
                localStorage.removeItem(sessionKey);
              }
            } catch {
              sessionId = 0;
              status = null;
              localStorage.removeItem(sessionKey);
            }
          }

          if (!status) {
            const created = await createProjectUploadSession(activeProjectId, {
              file_name: file.name,
              mime_type: file.type || "application/octet-stream",
              file_size: file.size,
              chunk_size: chunkSize,
              total_chunks: totalChunks,
              is_zip: file.name.toLowerCase().endsWith(".zip"),
            });
            sessionId = created.session_id;
            localStorage.setItem(sessionKey, String(sessionId));
            status = await getProjectUploadSessionStatus(activeProjectId, sessionId);
          }

          updateUploadProgress(progressId, {
            sessionId,
            uploadedBytes: status.session.uploaded_bytes,
            status: status.session.status === "uploading" ? "uploading" : status.session.status,
          });

          if (status.session.status === "uploading") {
            const queue = buildChunkQueueFromRanges(status.missing_ranges, status.total_chunks);
            await uploadChunkQueue(activeProjectId, sessionId, file, status.session.chunk_size, queue, (bytes) => {
              setUploadProgress((prev) =>
                prev.map((entry) =>
                  entry.id === progressId
                    ? {
                        ...entry,
                        uploadedBytes: Math.min(entry.uploadedBytes + bytes, entry.totalBytes),
                        status: "uploading",
                      }
                    : entry,
                ),
              );
            });
            await completeProjectUploadSession(activeProjectId, sessionId);
            localStorage.removeItem(sessionKey);
            updateUploadProgress(progressId, {
              uploadedBytes: file.size,
              status: "processing",
            });
          } else if (status.session.status === "done") {
            localStorage.removeItem(sessionKey);
            updateUploadProgress(progressId, {
              uploadedBytes: file.size,
              status: "done",
            });
          } else if (status.session.status === "failed") {
            setUploadProgress((prev) =>
              prev.map((entry) =>
                entry.id === progressId
                  ? { ...entry, status: "failed", error: status.session.error || "upload processing failed" }
                  : entry,
              ),
            );
          } else {
            updateUploadProgress(progressId, {
              uploadedBytes: file.size,
              status: "processing",
            });
          }
        } catch (err) {
          const msg = err instanceof Error ? err.message : "upload failed";
          fileFailures.push({ fileName: file.name, error: msg });
          updateUploadProgress(progressId, {
            status: "failed",
            error: msg,
          });
        }
      }
      resetPagination();
      await refetchFiles();
      await refreshUploadSessions();
      if (fileFailures.length > 0) {
        const sample = fileFailures[0];
        const summary =
          fileFailures.length === 1
            ? `Upload failed for ${sample.fileName}: ${sample.error}`
            : `${fileFailures.length} files failed (first: ${sample.fileName}: ${sample.error})`;
        setUploadError(summary);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "upload failed";
      setUploadError(
        msg === "413" ? `Upload exceeds project limit (${formatFileSize(projectMaxBytes)}).` : `Upload failed (${msg})`,
      );
      setUploadProgress((prev) =>
        prev.map((entry) => (entry.status === "done" ? entry : { ...entry, status: "failed", error: msg })),
      );
    } finally {
      setUploading(false);
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  }

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const droppedFiles = e.dataTransfer.files;
      if (droppedFiles.length > 0) handleUpload(droppedFiles);
    },
    [handleUpload],
  );

  async function handleDeleteAllProjectFiles() {
    if (!activeProjectId || deletingAllFiles) return;
    if (
      !confirm(
        `Delete all documents in this ${vocab.projectSingular}? This removes every file in the ${vocab.projectSingular}, not just the current search results.`,
      )
    ) {
      return;
    }
    setDeleteFilesError(null);
    setDeletingAllFiles(true);
    try {
      await deleteAllProjectFiles(activeProjectId);
      setPreviewFile(null);
      setTextViewFile(null);
      resetPagination();
      await refetchFiles();
    } catch (err) {
      setDeleteFilesError(
        err instanceof Error ? err.message : `Failed to delete ${vocab.projectDocsLabel.toLowerCase()}`,
      );
    } finally {
      setDeletingAllFiles(false);
    }
  }

  async function retryUploadSession(sessionId: number) {
    if (!activeProjectId) return;
    try {
      await retryProjectUploadSession(activeProjectId, sessionId);
      await refreshUploadSessions();
    } catch {
      // no-op
    }
  }

  return (
    <>
      <div className="flex flex-col h-full">
        {/* Sticky top: header + search + upload */}
        <div className="shrink-0 mx-auto w-full max-w-3xl px-6 pt-8 pb-4 space-y-4">
          {/* Header */}
          <div className="flex items-center gap-3">
            <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--color-card)] ring-1 ring-amber-900/20">
              <Folder className="h-6 w-6 text-amber-400/60" />
            </div>
            <div>
              <h2 className="text-[20px] font-semibold text-[var(--color-text)]">
                <span className="text-[14px] text-[var(--color-text-tertiary)] tabular-nums mr-2">#{project.id}</span>
                {project.name}
              </h2>
              <p className="text-[13px] text-[var(--color-text-tertiary)]">{vocab.projectDocsDescription}</p>
            </div>
          </div>

          {/* Search & stats */}
          <FileSearchBar
            value={fileSearch}
            onChange={(v) => {
              setFileSearch(v);
              resetPagination();
            }}
            placeholder={`Search ${vocab.projectSingular} files...`}
            stats={
              <>
                {fileSummary?.total_files ?? files.length} files {formatFileSize(totalBytes)}/
                {formatFileSize(projectMaxBytes)}
              </>
            }
          />

          {/* Drag-and-drop upload area */}
          <div
            ref={dropRef}
            onDragOver={(e) => {
              e.preventDefault();
              setDragOver(true);
            }}
            onDragLeave={() => setDragOver(false)}
            onDrop={handleDrop}
            onClick={() => fileInputRef.current?.click()}
            className={cn(
              "rounded-xl border-2 border-dashed p-4 transition-colors cursor-pointer",
              dragOver
                ? "border-amber-500/40 bg-amber-500/[0.04]"
                : "border-[var(--color-border)] bg-[var(--color-bg-secondary)] hover:border-amber-500/20",
            )}
          >
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--color-card)]">
                <Upload className="h-4 w-4 text-[var(--color-text-tertiary)]" />
              </div>
              <div>
                <p className="text-[13px] font-medium text-[var(--color-text)]">
                  Drop files here or <span className="text-amber-400">browse</span>
                </p>
                <p className="mt-0.5 text-[11px] text-[var(--color-text-tertiary)]">Supports any file type. Multiple files allowed.</p>
              </div>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                onChange={(e) => e.target.files && handleUpload(e.target.files)}
                className="hidden"
              />
            </div>
          </div>

          {uploadError && <p className="text-[12px] text-red-400">{uploadError}</p>}
          {deleteFilesError && <p className="text-[12px] text-red-400">{deleteFilesError}</p>}

          {/* Upload progress */}
          {uploadProgress.length > 0 && (
            <div className="space-y-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-4">
              {uploadProgress.map((entry) => {
                const pct = entry.totalBytes > 0 ? Math.round((entry.uploadedBytes / entry.totalBytes) * 100) : 0;
                return (
                  <div key={entry.id} className="text-[12px]">
                    <div className="flex items-center justify-between gap-2 text-[var(--color-text)]">
                      <span className="truncate">{entry.fileName}</span>
                      <span className="shrink-0 text-[var(--color-text-tertiary)]">
                        {entry.status} {["uploading", "processing", "done"].includes(entry.status) ? `${pct}%` : ""}
                      </span>
                    </div>
                    <div className="mt-1 h-1.5 w-full overflow-hidden rounded bg-[var(--color-card)]">
                      <div
                        className={cn(
                          "h-full transition-all",
                          entry.status === "failed" ? "bg-red-500/70" : "bg-amber-500/70",
                        )}
                        style={{ width: `${Math.max(0, Math.min(100, pct))}%` }}
                      />
                    </div>
                    {entry.error && <div className="mt-0.5 text-[10px] text-red-400">{entry.error}</div>}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Scrollable: file list + cloud + sessions */}
        <div className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-3xl px-6 pb-8 space-y-6">
            {/* File list */}
            <div className="space-y-3">
              {filesLoading && files.length === 0 && (
                <div className="flex items-center justify-center py-12">
                  <div className="h-6 w-6 animate-spin rounded-full border-2 border-[var(--color-border)] border-t-amber-400" />
                </div>
              )}
              {!filesLoading && files.length === 0 && (
                <div className="flex flex-col items-center py-12 text-center">
                  <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-[var(--color-card)] ring-1 ring-amber-900/20">
                    <FileText className="h-6 w-6 text-[var(--color-text-tertiary)]" />
                  </div>
                  <p className="text-[14px] text-[var(--color-text-secondary)]">
                    {filePage && filePage.total > 0 ? "No files match your search" : "No files uploaded yet"}
                  </p>
                  <p className="mt-1 text-[12px] text-[var(--color-text-tertiary)]">
                    {filePage && filePage.total > 0
                      ? "Try a different search term"
                      : `Upload files to make them available for this ${vocab.projectSingular}`}
                  </p>
                </div>
              )}
              {files.map((f) => {
                const canPreview = isPreviewable(f);
                return (
                  <div
                    key={f.id}
                    onClick={() => canPreview && setPreviewFile(f)}
                    className={cn(
                      "group rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-4 transition-colors hover:border-amber-900/30",
                      canPreview && "cursor-pointer",
                    )}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex items-start gap-3 min-w-0">
                        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-[var(--color-card)] ring-1 ring-amber-900/20">
                          <FileText className="h-4 w-4 text-[var(--color-text-tertiary)]" />
                        </div>
                        <div className="min-w-0">
                          <div className="text-[13px] font-medium text-[var(--color-text)] truncate">{f.file_name}</div>
                          <div className="mt-0.5 text-[12px] text-[var(--color-text-tertiary)]">
                            {formatFileSize(f.size_bytes)}
                            {f.source_path && f.source_path !== f.file_name && (
                              <span className="ml-1.5">&middot; {f.source_path}</span>
                            )}
                          </div>
                        </div>
                      </div>
                      <div className="flex gap-1.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
                        {f.has_text && (
                          <button
                            onClick={async (e) => {
                              e.stopPropagation();
                              if (!activeProjectId) return;
                              const data = await fetchProjectFileText(activeProjectId, f.id);
                              setTextViewFile({ id: f.id, name: data.file_name, text: data.extracted_text });
                            }}
                            className="rounded-lg p-2 text-[var(--color-text-tertiary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-emerald-400"
                            title={`View extracted text (${(f.text_chars / 1000).toFixed(1)}k chars)`}
                          >
                            <FileText className="h-3.5 w-3.5" />
                          </button>
                        )}
                        {!f.has_text && (
                          <button
                            onClick={async (e) => {
                              e.stopPropagation();
                              if (!activeProjectId) return;
                              setExtracting(f.id);
                              try {
                                await reextractProjectFile(activeProjectId, f.id);
                                refetchFiles();
                              } finally {
                                setExtracting(null);
                              }
                            }}
                            disabled={extracting === f.id}
                            className="rounded-lg p-2 text-[var(--color-text-tertiary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-[var(--color-text)] disabled:animate-spin"
                            title="Extract text"
                          >
                            <RotateCw className="h-3.5 w-3.5" />
                          </button>
                        )}
                        <button
                          onClick={async (e) => {
                            e.stopPropagation();
                            if (!activeProjectId) return;
                            if (!confirm(`Delete "${f.file_name}"?`)) return;
                            await deleteProjectFile(activeProjectId, f.id);
                            refetchFiles();
                          }}
                          className="rounded-lg p-2 text-[var(--color-text-tertiary)] transition-colors hover:bg-[var(--color-card-alt)] hover:text-red-400"
                          title="Delete file"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </div>
                    {f.has_text && (
                      <div className="mt-3 flex items-center gap-2">
                        <span className="rounded-full bg-emerald-500/15 px-2.5 py-0.5 text-[11px] font-medium text-emerald-300 ring-1 ring-inset ring-emerald-500/20">
                          Extracted
                        </span>
                        <span className="text-[11px] text-[var(--color-text-tertiary)]">
                          {(f.text_chars / 1000).toFixed(1)}k chars
                        </span>
                      </div>
                    )}
                  </div>
                );
              })}

              {/* Pagination */}
              {filePage && (
                <FileListPagination
                  filePage={filePage}
                  currentOffset={fl.currentFilePage.offset}
                  fileCount={files.length}
                  pageSize={fl.pageSize}
                  onPageSizeChange={(s) => {
                    fl.setPageSize(s);
                    resetPagination();
                  }}
                  canGoPrev={fl.filePageStack.length > 1}
                  onPrev={() => fl.setFilePageStack((prev) => (prev.length > 1 ? prev.slice(0, -1) : prev))}
                  canGoNext={!!(filePage.has_more && filePage.next_cursor)}
                  onNext={() => {
                    if (!filePage.next_cursor) return;
                    fl.setFilePageStack((prev) => [
                      ...prev,
                      {
                        cursor: filePage?.next_cursor ?? null,
                        offset: fl.currentFilePage.offset + files.length,
                      },
                    ]);
                  }}
                  actions={
                    <button
                      type="button"
                      onClick={handleDeleteAllProjectFiles}
                      disabled={deletingAllFiles}
                      className="inline-flex items-center gap-1.5 rounded-lg border border-red-500/20 bg-red-500/[0.08] px-3 py-1.5 text-[12px] font-medium text-red-300 transition-colors hover:bg-red-500/[0.14] disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                      {deletingAllFiles ? "Deleting..." : "Delete All"}
                    </button>
                  }
                />
              )}
            </div>

            {/* Upload sessions */}
            <UploadSessionsSection
              uploadSessions={uploadSessions}
              uploadSessionCounts={uploadSessionCounts}
              uploadSessionsLoading={uploadSessionsLoading}
              onRetrySession={retryUploadSession}
            />

            {/* Cloud storage */}
            <ProjectCloudStorageSection
              projectId={activeProjectId}
              settings={settings ?? null}
              onImported={() => {
                resetPagination();
                refetchFiles();
              }}
            />
          </div>
        </div>
      </div>

      {activeProjectId && (
        <FilePreviewWrapper file={previewFile} fetchContent={(id) => fetchProjectFileContent(activeProjectId, id)} onClose={() => setPreviewFile(null)} />
      )}
      {textViewFile && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onClick={() => setTextViewFile(null)}
        >
          <div
            className="mx-4 flex max-h-[80vh] w-full max-w-3xl flex-col rounded-xl border border-white/10 bg-zinc-900 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
              <span className="text-[15px] font-semibold text-zinc-100">{textViewFile.name} — Extracted Text</span>
              <button onClick={() => setTextViewFile(null)} className="text-zinc-500 hover:text-zinc-300">
                ✕
              </button>
            </div>
            <pre className="flex-1 overflow-auto whitespace-pre-wrap p-5 font-mono text-[13px] leading-relaxed text-zinc-300">
              {textViewFile.text}
            </pre>
          </div>
        </div>
      )}
    </>
  );
}
