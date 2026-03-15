import type { UploadSession, Settings } from "@/lib/api";
import { CloudStoragePanel } from "./cloud-storage";

export function UploadSessionsSection({
  uploadSessions,
  uploadSessionCounts,
  uploadSessionsLoading,
  onRetrySession,
}: {
  uploadSessions: UploadSession[];
  uploadSessionCounts: Record<string, number>;
  uploadSessionsLoading: boolean;
  onRetrySession: (sessionId: number) => void;
}) {
  if (!uploadSessionsLoading && uploadSessions.length === 0) return null;

  return (
    <div className="rounded-xl border border-[#2a2520] bg-[#151412] p-4">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[12px] font-semibold text-[#e8e0d4]">Upload Sessions</span>
        <span className="text-[11px] text-[#6b6459] tabular-nums">
          {uploadSessionCounts.uploading ?? 0} uploading &middot; {uploadSessionCounts.processing ?? 0}{" "}
          processing &middot; {uploadSessionCounts.done ?? 0} done
        </span>
      </div>
      <div className="space-y-1.5 max-h-32 overflow-y-auto">
        {uploadSessions.slice(0, 8).map((s) => (
          <div
            key={s.id}
            className="flex items-center justify-between rounded-lg border border-[#2a2520] px-3 py-2 text-[12px]"
          >
            <span className="truncate pr-2 text-[#e8e0d4]">
              #{s.id} {s.file_name}
            </span>
            <div className="flex items-center gap-2">
              <span className="text-[#6b6459]">{s.status}</span>
              {s.status === "failed" && (
                <button
                  onClick={() => onRetrySession(s.id)}
                  className="rounded-lg border border-amber-500/30 px-2 py-1 text-[11px] text-amber-300 hover:bg-amber-500/10"
                >
                  Retry
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function ProjectCloudStorageSection({
  projectId,
  settings,
  onImported,
}: {
  projectId: number | null;
  settings: Settings | null;
  onImported: () => void;
}) {
  return (
    <CloudStoragePanel
      projectId={projectId}
      settings={settings}
      onImported={onImported}
    />
  );
}
