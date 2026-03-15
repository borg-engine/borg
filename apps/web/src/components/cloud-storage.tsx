import {
  ChevronDown,
  CloudOff,
  File,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Folder,
  FolderOpen,
  Info,
  Loader2,
  RefreshCcw,
  Search,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { CloudBrowseItem, CloudConnection, Settings } from "@/lib/api";
import {
  browseProjectCloudFiles,
  deleteProjectCloudConnection,
  importProjectCloudFiles,
  useProjectCloudConnections,
} from "@/lib/api";
import { cn, formatFileSize } from "@/lib/utils";

const CLOUD_PROVIDERS = [
  { id: "dropbox", label: "Dropbox", clientIdKey: "dropbox_client_id", clientSecretKey: "dropbox_client_secret" },
  {
    id: "google_drive",
    label: "Google Drive",
    clientIdKey: "google_client_id",
    clientSecretKey: "google_client_secret",
  },
  { id: "onedrive", label: "OneDrive", clientIdKey: "ms_client_id", clientSecretKey: "ms_client_secret" },
] as const;

const MAX_CLOUD_IMPORT_SELECTION = 1000;

function cloudProviderLabel(provider: string): string {
  return CLOUD_PROVIDERS.find((p) => p.id === provider)?.label ?? provider;
}

function DropboxIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4 w-4" aria-hidden>
      <path
        fill="#0D63D6"
        d="m6.1 3.2-4.7 3 4.7 3 4.7-3-4.7-3Zm11.8 0-4.7 3 4.7 3 4.7-3-4.7-3ZM6.1 10.7l-4.7 3 4.7 3 4.7-3-4.7-3Zm11.8 0-4.7 3 4.7 3 4.7-3-4.7-3ZM12 14.9l-4.7 3 4.7 3 4.7-3-4.7-3Z"
      />
    </svg>
  );
}

function GoogleDriveIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4 w-4" aria-hidden>
      <path fill="#0F9D58" d="M6.5 20.3h11l-2.7-4.7h-11l2.7 4.7Z" />
      <path fill="#FFC107" d="m12 3.7 5.5 9.5h5.4L17.4 3.7H12Z" />
      <path fill="#4285F4" d="M1.1 13.2h5.4L12 3.7H6.6L1.1 13.2Z" />
    </svg>
  );
}

function OneDriveIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4 w-4" aria-hidden>
      <path
        fill="#0078D4"
        d="M10.2 9a5.4 5.4 0 0 1 10.2 2.4h.2a3.4 3.4 0 1 1 0 6.8H6.5a4.5 4.5 0 0 1-.8-8.9A5.7 5.7 0 0 1 10.2 9Z"
      />
    </svg>
  );
}

function ICloudIcon() {
  return <CloudOff className="h-4 w-4 text-[var(--color-text-tertiary)]" />;
}

function CloudProviderIcon({ provider }: { provider: string }) {
  if (provider === "dropbox") return <DropboxIcon />;
  if (provider === "google_drive") return <GoogleDriveIcon />;
  return <OneDriveIcon />;
}

const EXT_SPREADSHEET = new Set(["xls", "xlsx", "csv", "tsv", "ods", "numbers"]);
const EXT_IMAGE = new Set(["jpg", "jpeg", "png", "gif", "svg", "webp", "bmp", "ico", "tiff", "heic"]);
const EXT_VIDEO = new Set(["mp4", "mov", "avi", "mkv", "webm", "wmv", "flv", "m4v"]);
const EXT_CODE = new Set(["js", "ts", "jsx", "tsx", "py", "rs", "go", "java", "c", "cpp", "h", "rb", "sh", "json", "yaml", "yml", "toml", "xml", "html", "css", "scss"]);
const EXT_DOC = new Set(["pdf", "doc", "docx", "rtf", "odt", "pages", "txt", "md"]);

function FileTypeIcon({ name }: { name: string }) {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (EXT_SPREADSHEET.has(ext)) return <FileSpreadsheet className="h-3.5 w-3.5 text-emerald-400/70" />;
  if (EXT_IMAGE.has(ext)) return <FileImage className="h-3.5 w-3.5 text-purple-400/70" />;
  if (EXT_VIDEO.has(ext)) return <FileVideo className="h-3.5 w-3.5 text-pink-400/70" />;
  if (EXT_CODE.has(ext)) return <FileCode className="h-3.5 w-3.5 text-cyan-400/70" />;
  if (EXT_DOC.has(ext)) return <FileText className="h-3.5 w-3.5 text-amber-400/70" />;
  return <File className="h-3.5 w-3.5 text-zinc-500" />;
}

interface ImportProgress {
  total: number;
  completed: number;
  failed: string[];
}

interface CloudStoragePanelProps {
  projectId: number | null;
  settings: Settings | null;
  onImported: () => void;
}

export function CloudStoragePanel({ projectId, settings, onImported }: CloudStoragePanelProps) {
  const {
    data: cloudConnections = [],
    refetch: refetchCloudConnections,
    isLoading: cloudConnectionsLoading,
  } = useProjectCloudConnections(projectId);

  const [cloudMessage, setCloudMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [cloudModalOpen, setCloudModalOpen] = useState(false);
  const [cloudModalConn, setCloudModalConn] = useState<CloudConnection | null>(null);
  const [cloudItems, setCloudItems] = useState<CloudBrowseItem[]>([]);
  const [cloudLoading, setCloudLoading] = useState(false);
  const [cloudLoadError, setCloudLoadError] = useState<string | null>(null);
  const [cloudCursor, setCloudCursor] = useState<string | null>(null);
  const [cloudHasMore, setCloudHasMore] = useState(false);
  const [cloudSelected, setCloudSelected] = useState<Record<string, CloudBrowseItem>>({});
  const [cloudImporting, setCloudImporting] = useState(false);
  const [cloudBreadcrumbs, setCloudBreadcrumbs] = useState<Array<{ id?: string; name: string }>>([{ name: "Root" }]);
  const [filterText, setFilterText] = useState("");
  const [importProgress, setImportProgress] = useState<ImportProgress | null>(null);
  const [tokenExpired, setTokenExpired] = useState(false);

  const publicUrl = settings?.public_url?.trim() || "";
  const publicUrlValid = useMemo(() => {
    if (!publicUrl) return false;
    try {
      const parsed = new URL(publicUrl);
      return parsed.protocol === "http:" || parsed.protocol === "https:";
    } catch {
      return false;
    }
  }, [publicUrl]);

  const maxCloudImportSelection = Math.max(1, settings?.cloud_import_max_batch_files ?? MAX_CLOUD_IMPORT_SELECTION);
  const currentCloudFolderId = cloudBreadcrumbs[cloudBreadcrumbs.length - 1]?.id;

  const filteredItems = useMemo(() => {
    if (!filterText.trim()) return cloudItems;
    const lower = filterText.toLowerCase();
    return cloudItems.filter((item) => item.name.toLowerCase().includes(lower));
  }, [cloudItems, filterText]);

  const selectableFiles = useMemo(
    () => filteredItems.filter((item) => item.type === "file"),
    [filteredItems],
  );

  const selectedFiles = useMemo(
    () => Object.values(cloudSelected).filter((i) => i.type === "file"),
    [cloudSelected],
  );

  const selectedTotalSize = useMemo(
    () => selectedFiles.reduce((sum, f) => sum + (f.size || 0), 0),
    [selectedFiles],
  );

  const allFilesSelected = selectableFiles.length > 0 && selectableFiles.every((f) => cloudSelected[f.id]);

  // OAuth callback URL hash parsing
  useEffect(() => {
    const hash = window.location.hash || "";
    const queryIdx = hash.indexOf("?");
    if (queryIdx < 0) return;

    const params = new URLSearchParams(hash.slice(queryIdx + 1));
    const connected = params.get("cloud_connected");
    const error = params.get("cloud_error");
    const provider = params.get("provider");
    if (!connected && !error) return;

    if (connected) {
      setCloudMessage({ type: "success", text: `${cloudProviderLabel(connected)} connected.` });
      refetchCloudConnections();
    } else if (error) {
      const prefix = provider ? `${cloudProviderLabel(provider)}: ` : "";
      if (error === "access_denied") {
        setCloudMessage({ type: "error", text: `${prefix}authorization was denied.` });
      } else if (error === "token_exchange") {
        setCloudMessage({
          type: "error",
          text: `${prefix}token exchange failed. Check client ID/secret and callback URL.`,
        });
      } else if (error === "missing_public_url") {
        setCloudMessage({
          type: "error",
          text: "Set a valid Public URL in Settings before connecting cloud providers.",
        });
      } else if (error === "missing_credentials") {
        setCloudMessage({ type: "error", text: `${prefix}credentials are missing in Settings > Cloud Storage.` });
      } else {
        setCloudMessage({ type: "error", text: `${prefix}connection failed (${error}).` });
      }
    }

    const cleanHash = hash.slice(0, queryIdx) || "#/projects";
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}${cleanHash}`);
  }, [refetchCloudConnections]);

  // Reset cloud state when projectId changes
  useEffect(() => {
    if (!projectId) {
      setCloudModalOpen(false);
      setCloudModalConn(null);
      setCloudItems([]);
      setCloudSelected({});
      setCloudBreadcrumbs([{ name: "Root" }]);
      setFilterText("");
    }
  }, [projectId]);

  function hasCloudCredentials(provider: (typeof CLOUD_PROVIDERS)[number]) {
    if (!settings) return false;
    const id = settings[provider.clientIdKey] ?? "";
    const secret = settings[provider.clientSecretKey] ?? "";
    return id.trim().length > 0 && secret.trim().length > 0;
  }

  function isTokenExpiredError(msg: string): boolean {
    const lower = msg.toLowerCase();
    return lower.includes("401") || lower.includes("expired") || lower.includes("unauthorized") || lower.includes("invalid_grant");
  }

  async function loadCloudFolder(
    connection: CloudConnection,
    folderId?: string,
    opts?: { append?: boolean; cursor?: string },
  ) {
    if (!projectId) return;
    setCloudLoading(true);
    setCloudLoadError(null);
    setTokenExpired(false);
    try {
      const data = await browseProjectCloudFiles(projectId, connection.id, {
        folder_id: folderId,
        cursor: opts?.cursor,
      });
      setCloudItems((prev) => (opts?.append ? [...prev, ...(data.items || [])] : data.items || []));
      const nextCursor = data.cursor ?? data.next_page_token ?? null;
      setCloudCursor(nextCursor);
      setCloudHasMore(Boolean(data.has_more || data.next_page_token));
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to browse cloud files";
      if (isTokenExpiredError(msg)) {
        setTokenExpired(true);
        setCloudLoadError("Session expired. Please reconnect your account.");
      } else {
        setCloudLoadError(msg);
      }
    } finally {
      setCloudLoading(false);
    }
  }

  function connectCloudProvider(provider: (typeof CLOUD_PROVIDERS)[number]["id"]) {
    if (!projectId) return;
    if (!publicUrlValid) {
      setCloudMessage({ type: "error", text: "Set a valid Public URL in Settings before connecting cloud providers." });
      return;
    }
    window.location.href = `/api/cloud/${provider}/auth?project_id=${projectId}`;
  }

  function reconnectProvider(connection: CloudConnection) {
    connectCloudProvider(connection.provider as (typeof CLOUD_PROVIDERS)[number]["id"]);
  }

  async function openCloudBrowser(connection: CloudConnection) {
    setCloudModalConn(connection);
    setCloudModalOpen(true);
    setCloudSelected({});
    setCloudBreadcrumbs([{ name: "Root" }]);
    setCloudCursor(null);
    setCloudHasMore(false);
    setFilterText("");
    setImportProgress(null);
    setTokenExpired(false);
    await loadCloudFolder(connection);
  }

  async function disconnectCloudConnection(connection: CloudConnection) {
    if (!projectId) return;
    if (
      !confirm(
        `Disconnect ${cloudProviderLabel(connection.provider)} account ${connection.account_email || connection.id}?`,
      )
    )
      return;
    try {
      await deleteProjectCloudConnection(projectId, connection.id);
      setCloudMessage({ type: "success", text: `${cloudProviderLabel(connection.provider)} disconnected.` });
      await refetchCloudConnections();
    } catch (err) {
      const msg = err instanceof Error ? err.message : "disconnect failed";
      setCloudMessage({ type: "error", text: `Failed to disconnect (${msg}).` });
    }
  }

  function toggleSelectAll() {
    if (allFilesSelected) {
      setCloudSelected((prev) => {
        const next = { ...prev };
        for (const f of selectableFiles) delete next[f.id];
        return next;
      });
    } else {
      setCloudSelected((prev) => {
        const next = { ...prev };
        for (const f of selectableFiles) next[f.id] = f;
        return next;
      });
    }
  }

  async function importSelectedCloudFiles() {
    if (!projectId || !cloudModalConn || cloudImporting) return;
    const filesToImport = selectedFiles.map((item) => ({ id: item.id, name: item.name, size: item.size }));
    if (filesToImport.length === 0) return;
    if (filesToImport.length > maxCloudImportSelection) {
      setCloudLoadError(`Please select at most ${maxCloudImportSelection} files per import.`);
      return;
    }

    setCloudImporting(true);
    setImportProgress({ total: filesToImport.length, completed: 0, failed: [] });

    const batchSize = 10;
    let completed = 0;
    const failed: string[] = [];

    for (let i = 0; i < filesToImport.length; i += batchSize) {
      const batch = filesToImport.slice(i, i + batchSize);
      try {
        await importProjectCloudFiles(projectId, cloudModalConn.id, batch);
        completed += batch.length;
      } catch (err) {
        const msg = err instanceof Error ? err.message : "import failed";
        for (const f of batch) failed.push(f.name);
        completed += batch.length;
        if (isTokenExpiredError(msg)) {
          setTokenExpired(true);
          for (const f of filesToImport.slice(i + batchSize)) failed.push(f.name);
          completed = filesToImport.length;
          setImportProgress({ total: filesToImport.length, completed, failed });
          setCloudImporting(false);
          setCloudLoadError("Session expired during import. Please reconnect and retry.");
          return;
        }
      }
      setImportProgress({ total: filesToImport.length, completed, failed: [...failed] });
    }

    setCloudImporting(false);

    if (failed.length === 0) {
      setCloudMessage({ type: "success", text: `Imported ${filesToImport.length} file(s).` });
      setCloudModalOpen(false);
      setCloudSelected({});
      setImportProgress(null);
      onImported();
    } else {
      const succeeded = filesToImport.length - failed.length;
      setCloudLoadError(
        `${succeeded} of ${filesToImport.length} files imported. ${failed.length} failed: ${failed.slice(0, 5).join(", ")}${failed.length > 5 ? ` and ${failed.length - 5} more` : ""}.`,
      );
      if (succeeded > 0) onImported();
    }
  }

  function retryFailedImport() {
    if (!importProgress?.failed.length) return;
    const failedSet = new Set(importProgress.failed);
    const retryItems: Record<string, CloudBrowseItem> = {};
    for (const item of Object.values(cloudSelected)) {
      if (item.type === "file" && failedSet.has(item.name)) {
        retryItems[item.id] = item;
      }
    }
    setCloudSelected(retryItems);
    setImportProgress(null);
    setCloudLoadError(null);
  }

  const [expanded, setExpanded] = useState(false);

  const importButtonLabel = useMemo(() => {
    const count = selectedFiles.length;
    if (count === 0) return "Import Selected";
    const sizeStr = formatFileSize(selectedTotalSize);
    return `Import ${count} file${count !== 1 ? "s" : ""} (${sizeStr})`;
  }, [selectedFiles.length, selectedTotalSize]);

  return (
    <>
      {/* Cloud Storage dropdown */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          className="flex w-full items-center justify-between px-4 py-3 text-[12px] font-semibold text-[var(--color-text)] hover:bg-[#1a1816] rounded-xl transition-colors"
        >
          <span className="flex items-center gap-2">
            Cloud Storage
            {cloudConnections.length > 0 && (
              <span className="rounded-full bg-[var(--color-border)] px-1.5 py-0.5 text-[10px] text-[var(--color-text-tertiary)]">
                {cloudConnections.length}
              </span>
            )}
          </span>
          <ChevronDown className={cn("h-3.5 w-3.5 text-[var(--color-text-tertiary)] transition-transform", expanded && "rotate-180")} />
        </button>
        {expanded && (
          <div className="px-4 pb-4">
            {!publicUrlValid && (
              <div className="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-300">
                Configure a valid Public URL in Settings before connecting cloud accounts.
              </div>
            )}
            {cloudMessage && (
              <div
                className={cn(
                  "mb-3 flex items-start justify-between gap-2 rounded-lg border px-3 py-2 text-[11px]",
                  cloudMessage.type === "success"
                    ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-400"
                    : "border-red-500/30 bg-red-500/10 text-red-400",
                )}
              >
                <span>{cloudMessage.text}</span>
                <button onClick={() => setCloudMessage(null)} className="shrink-0 text-[var(--color-text-tertiary)] hover:text-[var(--color-text)]">
                  <X className="h-3 w-3" />
                </button>
              </div>
            )}
            <div className="mb-3 flex flex-wrap gap-1.5">
              {CLOUD_PROVIDERS.map((provider) => {
                const configured = hasCloudCredentials(provider);
                return (
                  <button
                    key={provider.id}
                    onClick={() => connectCloudProvider(provider.id)}
                    disabled={!configured || !projectId || !publicUrlValid}
                    title={
                      !publicUrlValid
                        ? "Set a valid Public URL in Settings > Cloud Storage"
                        : configured
                          ? `Connect ${provider.label}`
                          : `Configure ${provider.label} credentials in Settings > Cloud Storage`
                    }
                    className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-[12px] text-[var(--color-text)] transition-colors hover:bg-[var(--color-card-alt)] disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    <CloudProviderIcon provider={provider.id} />
                    {provider.label}
                  </button>
                );
              })}
              {/* iCloud - disabled with explanation */}
              <div
                className="group relative inline-flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-[12px] text-[var(--color-text-tertiary)] cursor-not-allowed opacity-50"
              >
                <ICloudIcon />
                <span>iCloud</span>
                <Info className="h-3 w-3 text-[var(--color-text-tertiary)]" />
                <div className="pointer-events-none absolute bottom-full left-1/2 z-10 mb-2 w-56 -translate-x-1/2 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-[11px] leading-relaxed text-[var(--color-text-secondary)] opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
                  iCloud Drive does not support standard OAuth. To import iCloud files, download them to your device first, then upload directly.
                </div>
              </div>
            </div>
            <div className="space-y-1.5 max-h-36 overflow-y-auto">
              {cloudConnections.map((conn) => (
                <div
                  key={conn.id}
                  className="flex items-center justify-between rounded-lg border border-[var(--color-border)] px-3 py-2 text-[12px]"
                >
                  <div className="min-w-0 flex items-center gap-1.5 text-[var(--color-text)]">
                    <CloudProviderIcon provider={conn.provider} />
                    <span className="truncate">{conn.account_email || cloudProviderLabel(conn.provider)}</span>
                  </div>
                  <div className="flex shrink-0 items-center gap-1.5">
                    <button
                      onClick={() => openCloudBrowser(conn)}
                      className="inline-flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-2.5 py-1 text-[12px] text-[var(--color-text)] transition-colors hover:bg-[var(--color-card-alt)]"
                    >
                      <Folder className="h-3 w-3" />
                      Browse
                    </button>
                    <button
                      onClick={() => disconnectCloudConnection(conn)}
                      className="rounded-lg p-1.5 text-[var(--color-text-tertiary)] transition-colors hover:bg-red-500/10 hover:text-red-400"
                      title="Disconnect"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </div>
                </div>
              ))}
              {!cloudConnectionsLoading && cloudConnections.length === 0 && (
                <div className="text-[12px] text-[var(--color-text-tertiary)]">No connected cloud accounts.</div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Cloud browser modal */}
      {cloudModalOpen && cloudModalConn && projectId && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onClick={() => setCloudModalOpen(false)}
        >
          <div
            className="mx-4 flex max-h-[82vh] w-full max-w-4xl flex-col rounded-xl border border-white/10 bg-zinc-900 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
              <div className="min-w-0 flex-1">
                <div className="text-[15px] font-semibold text-zinc-100">
                  {cloudProviderLabel(cloudModalConn.provider)} - {cloudModalConn.account_email || "Account"}
                </div>
                <div className="mt-1.5 flex items-center gap-1 overflow-x-auto text-[12px] text-zinc-400">
                  {cloudBreadcrumbs.map((crumb, idx) => (
                    <button
                      key={`${crumb.id ?? "root"}-${idx}`}
                      onClick={async () => {
                        const next = cloudBreadcrumbs.slice(0, idx + 1);
                        setCloudBreadcrumbs(next);
                        setCloudSelected({});
                        setCloudCursor(null);
                        setCloudHasMore(false);
                        setFilterText("");
                        await loadCloudFolder(cloudModalConn, next[next.length - 1]?.id);
                      }}
                      className="shrink-0 hover:text-zinc-300"
                    >
                      {idx > 0 ? " / " : ""}
                      {crumb.name}
                    </button>
                  ))}
                </div>
              </div>
              <button onClick={() => setCloudModalOpen(false)} className="ml-3 rounded-lg p-1.5 text-zinc-500 hover:bg-white/[0.06] hover:text-zinc-300 transition-colors">
                <X className="h-4 w-4" />
              </button>
            </div>

            {/* Search bar */}
            <div className="border-b border-white/[0.06] px-5 py-2.5">
              <div className="relative">
                <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-zinc-500" />
                <input
                  type="text"
                  value={filterText}
                  onChange={(e) => setFilterText(e.target.value)}
                  placeholder="Filter files in this folder..."
                  className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] py-1.5 pl-8 pr-3 text-[12px] text-zinc-300 placeholder:text-zinc-600 focus:border-white/[0.15] focus:outline-none transition-colors"
                />
                {filterText && (
                  <button
                    onClick={() => setFilterText("")}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300"
                  >
                    <X className="h-3 w-3" />
                  </button>
                )}
              </div>
            </div>

            {/* File list */}
            <div className="min-h-0 flex-1 overflow-y-auto p-4">
              {cloudLoadError && (
                <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[12px] text-red-400">
                  <div className="flex items-start justify-between gap-2">
                    <span>{cloudLoadError}</span>
                    <div className="flex shrink-0 items-center gap-2">
                      {tokenExpired && cloudModalConn && (
                        <button
                          onClick={() => reconnectProvider(cloudModalConn)}
                          className="inline-flex items-center gap-1 rounded-md border border-red-500/30 px-2 py-0.5 text-[11px] text-red-300 hover:bg-red-500/10 transition-colors"
                        >
                          <RefreshCcw className="h-3 w-3" />
                          Reconnect
                        </button>
                      )}
                      {importProgress && importProgress.failed.length > 0 && (
                        <button
                          onClick={retryFailedImport}
                          className="inline-flex items-center gap-1 rounded-md border border-red-500/30 px-2 py-0.5 text-[11px] text-red-300 hover:bg-red-500/10 transition-colors"
                        >
                          <RefreshCcw className="h-3 w-3" />
                          Retry failed
                        </button>
                      )}
                      <button
                        onClick={() => { setCloudLoadError(null); setImportProgress(null); }}
                        className="text-zinc-500 hover:text-zinc-300"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </div>
                  </div>
                </div>
              )}
              <div className="overflow-hidden rounded-xl border border-white/[0.08]">
                {/* Header row with Select All */}
                {selectableFiles.length > 0 && !cloudLoading && (
                  <div className="flex items-center gap-2 border-b border-white/[0.07] bg-white/[0.02] px-3 py-2 text-[11px] font-medium text-zinc-500 uppercase tracking-wider">
                    <input
                      type="checkbox"
                      checked={allFilesSelected}
                      onChange={toggleSelectAll}
                      className="cursor-pointer"
                      title={allFilesSelected ? "Deselect all" : "Select all"}
                    />
                    <span className="flex-1">Name</span>
                    <span className="w-20 text-right">Size</span>
                  </div>
                )}
                {filteredItems.map((item) => {
                  const selected = Boolean(cloudSelected[item.id]);
                  return (
                    <div
                      key={item.id}
                      className={cn(
                        "flex items-center justify-between border-b border-white/[0.07] px-3 py-2.5 text-[13px] last:border-b-0 transition-colors",
                        selected && "bg-blue-500/[0.06]",
                      )}
                    >
                      <label className="flex min-w-0 flex-1 items-center gap-2 text-zinc-300">
                        {item.type === "file" ? (
                          <input
                            type="checkbox"
                            checked={selected}
                            onChange={(e) => {
                              setCloudSelected((prev) => {
                                const next = { ...prev };
                                if (e.target.checked) next[item.id] = item;
                                else delete next[item.id];
                                return next;
                              });
                            }}
                            className="cursor-pointer"
                          />
                        ) : (
                          <span className="inline-block w-4" />
                        )}
                        {item.type === "folder" ? (
                          <FolderOpen className="h-3.5 w-3.5 text-blue-400/70 shrink-0" />
                        ) : (
                          <span className="shrink-0"><FileTypeIcon name={item.name} /></span>
                        )}
                        <button
                          disabled={item.type !== "folder"}
                          onClick={async () => {
                            if (item.type !== "folder") return;
                            setCloudBreadcrumbs((prev) => [...prev, { id: item.id, name: item.name }]);
                            setCloudSelected({});
                            setCloudCursor(null);
                            setCloudHasMore(false);
                            setFilterText("");
                            await loadCloudFolder(cloudModalConn, item.id);
                          }}
                          className={cn(
                            "truncate text-left",
                            item.type === "folder" ? "text-blue-400 hover:text-blue-300" : "text-zinc-300 cursor-default",
                          )}
                        >
                          {item.name}
                        </button>
                      </label>
                      <div className="ml-2 w-20 shrink-0 text-right text-[12px] text-zinc-500 tabular-nums">
                        {item.type === "file" ? formatFileSize(item.size || 0) : ""}
                      </div>
                    </div>
                  );
                })}
                {/* Loading spinner */}
                {cloudLoading && (
                  <div className="flex items-center justify-center gap-2 px-4 py-8 text-[13px] text-zinc-500">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    <span>Loading files...</span>
                  </div>
                )}
                {/* Empty states */}
                {!cloudLoading && cloudItems.length === 0 && !cloudLoadError && (
                  <div className="flex flex-col items-center justify-center gap-2 px-4 py-10 text-center">
                    <FolderOpen className="h-8 w-8 text-zinc-700" />
                    <span className="text-[13px] text-zinc-500">This folder is empty</span>
                  </div>
                )}
                {!cloudLoading && cloudItems.length > 0 && filteredItems.length === 0 && (
                  <div className="flex flex-col items-center justify-center gap-2 px-4 py-10 text-center">
                    <Search className="h-6 w-6 text-zinc-700" />
                    <span className="text-[13px] text-zinc-500">No files matching "{filterText}"</span>
                  </div>
                )}
              </div>
              {!cloudLoading && cloudHasMore && cloudCursor && (
                <button
                  onClick={() =>
                    loadCloudFolder(cloudModalConn, currentCloudFolderId, { append: true, cursor: cloudCursor })
                  }
                  className="mt-3 rounded-lg border border-white/[0.08] px-3 py-1.5 text-[12px] text-zinc-300 hover:bg-white/[0.06] transition-colors"
                >
                  Load more
                </button>
              )}
            </div>

            {/* Footer with import progress and action button */}
            <div className="flex items-center justify-between border-t border-white/10 px-5 py-4">
              <div className="text-[12px] text-zinc-400">
                {importProgress && cloudImporting ? (
                  <span className="flex items-center gap-2">
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    Importing {importProgress.completed} of {importProgress.total}...
                  </span>
                ) : (
                  <span>
                    {selectedFiles.length > 0
                      ? `${selectedFiles.length} file${selectedFiles.length !== 1 ? "s" : ""} selected (${formatFileSize(selectedTotalSize)})`
                      : "No files selected"}
                  </span>
                )}
              </div>
              <button
                onClick={importSelectedCloudFiles}
                disabled={cloudImporting || selectedFiles.length === 0}
                className="rounded-lg bg-blue-500/20 px-4 py-2 text-[13px] font-medium text-blue-300 hover:bg-blue-500/30 transition-colors disabled:cursor-not-allowed disabled:text-zinc-600"
              >
                {cloudImporting ? (
                  <span className="flex items-center gap-1.5">
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    Importing...
                  </span>
                ) : (
                  importButtonLabel
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
