import { useState, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  useKnowledgeFiles,
  uploadKnowledgeFile,
  updateKnowledgeFile,
  deleteKnowledgeFile,
} from "@/lib/api";
import type { KnowledgeFile } from "@/lib/types";
import { cn } from "@/lib/utils";

function formatBytes(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function FileRow({
  file,
  onDeleted,
  onUpdated,
}: {
  file: KnowledgeFile;
  onDeleted: () => void;
  onUpdated: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [desc, setDesc] = useState(file.description);
  const [inline, setInline] = useState(file.inline);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);

  async function save() {
    setSaving(true);
    try {
      await updateKnowledgeFile(file.id, { description: desc, inline });
      onUpdated();
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    if (!confirm(`Delete "${file.file_name}"?`)) return;
    setDeleting(true);
    try {
      await deleteKnowledgeFile(file.id);
      onDeleted();
    } finally {
      setDeleting(false);
    }
  }

  return (
    <div className="border border-zinc-700 rounded-lg p-3 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="font-mono text-sm text-zinc-100 truncate">{file.file_name}</div>
          <div className="text-xs text-zinc-500 mt-0.5">
            {formatBytes(file.size_bytes)} &middot; {new Date(file.created_at).toLocaleDateString()}
          </div>
        </div>
        <div className="flex gap-2 shrink-0">
          <button
            onClick={() => setEditing((v) => !v)}
            className="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700 hover:border-zinc-500 transition-colors"
          >
            {editing ? "Cancel" : "Edit"}
          </button>
          <button
            onClick={remove}
            disabled={deleting}
            className="text-xs text-red-400 hover:text-red-300 px-2 py-1 rounded border border-zinc-700 hover:border-red-700 transition-colors disabled:opacity-50"
          >
            {deleting ? "..." : "Delete"}
          </button>
        </div>
      </div>

      {!editing && file.description && (
        <div className="text-sm text-zinc-400">{file.description}</div>
      )}
      {!editing && (
        <div className="flex items-center gap-1.5">
          <span
            className={cn(
              "text-xs px-1.5 py-0.5 rounded",
              file.inline
                ? "bg-blue-900/50 text-blue-300 border border-blue-700"
                : "bg-zinc-800 text-zinc-400 border border-zinc-700",
            )}
          >
            {file.inline ? "Inline" : "Listed"}
          </span>
          {file.inline && (
            <span className="text-xs text-zinc-500">Content injected into agent prompts</span>
          )}
          {!file.inline && (
            <span className="text-xs text-zinc-500">Filename listed in agent prompts</span>
          )}
        </div>
      )}

      {editing && (
        <div className="space-y-2 pt-1">
          <div>
            <label className="text-xs text-zinc-400 block mb-1">Description</label>
            <input
              type="text"
              value={desc}
              onChange={(e) => setDesc(e.target.value)}
              placeholder="Brief description of this file"
              className="w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-100 focus:outline-none focus:border-zinc-400"
            />
          </div>
          <div className="flex items-center gap-2">
            <input
              id={`inline-${file.id}`}
              type="checkbox"
              checked={inline}
              onChange={(e) => setInline(e.target.checked)}
              className="rounded"
            />
            <label htmlFor={`inline-${file.id}`} className="text-sm text-zinc-300">
              Inline (embed content in prompt)
            </label>
          </div>
          <button
            onClick={save}
            disabled={saving}
            className="text-xs bg-zinc-700 hover:bg-zinc-600 text-zinc-100 px-3 py-1.5 rounded transition-colors disabled:opacity-50"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      )}
    </div>
  );
}

export function KnowledgePanel() {
  const { data: files, isLoading } = useKnowledgeFiles();
  const queryClient = useQueryClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [description, setDescription] = useState("");
  const [inline, setInline] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: ["knowledge"] });
  }

  async function handleUpload() {
    if (!selectedFile) return;
    setUploading(true);
    setUploadError(null);
    try {
      await uploadKnowledgeFile(selectedFile, description, inline);
      setSelectedFile(null);
      setDescription("");
      setInline(false);
      if (fileInputRef.current) fileInputRef.current.value = "";
      invalidate();
    } catch (e) {
      setUploadError(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setUploading(false);
    }
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto p-4 space-y-6 max-w-2xl mx-auto w-full">
      <div>
        <h2 className="text-lg font-semibold text-zinc-100">Knowledge Base</h2>
        <p className="text-sm text-zinc-400 mt-1">
          Files available to all agents at <code className="text-zinc-300">/knowledge/</code>.
          Inline files are embedded directly in the prompt; listed files are mentioned by name.
        </p>
      </div>

      {/* Upload form */}
      <div className="border border-zinc-700 rounded-lg p-4 space-y-3 bg-zinc-900/50">
        <h3 className="text-sm font-medium text-zinc-200">Upload File</h3>
        <div>
          <input
            ref={fileInputRef}
            type="file"
            onChange={(e) => setSelectedFile(e.target.files?.[0] ?? null)}
            className="block w-full text-sm text-zinc-400 file:mr-3 file:py-1.5 file:px-3 file:rounded file:border file:border-zinc-600 file:bg-zinc-800 file:text-zinc-200 file:text-xs file:cursor-pointer hover:file:bg-zinc-700"
          />
        </div>
        <div>
          <label className="text-xs text-zinc-400 block mb-1">Description</label>
          <input
            type="text"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What is this file? (shown in prompt)"
            className="w-full bg-zinc-800 border border-zinc-600 rounded px-2 py-1 text-sm text-zinc-100 focus:outline-none focus:border-zinc-400"
          />
        </div>
        <div className="flex items-center gap-2">
          <input
            id="upload-inline"
            type="checkbox"
            checked={inline}
            onChange={(e) => setInline(e.target.checked)}
            className="rounded"
          />
          <label htmlFor="upload-inline" className="text-sm text-zinc-300">
            Inline (embed file content in agent prompts)
          </label>
        </div>
        {uploadError && <div className="text-xs text-red-400">{uploadError}</div>}
        <button
          onClick={handleUpload}
          disabled={!selectedFile || uploading}
          className="text-sm bg-zinc-700 hover:bg-zinc-600 text-zinc-100 px-4 py-2 rounded transition-colors disabled:opacity-40"
        >
          {uploading ? "Uploading..." : "Upload"}
        </button>
      </div>

      {/* File list */}
      <div className="space-y-3">
        {isLoading && <div className="text-sm text-zinc-500">Loading...</div>}
        {!isLoading && (!files || files.length === 0) && (
          <div className="text-sm text-zinc-500 text-center py-8">
            No knowledge files uploaded yet.
          </div>
        )}
        {files?.map((file) => (
          <FileRow key={file.id} file={file} onDeleted={invalidate} onUpdated={invalidate} />
        ))}
      </div>
    </div>
  );
}
