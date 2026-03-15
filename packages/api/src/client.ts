// Framework-agnostic API client for Borg.
// No React imports — just async functions and classes.

// Runtime base URL: set window.__API_BASE_URL__ = "https://api.example.com" before the app loads.
// Falls back to same-origin (empty string) which works in dev via a proxy.
export function apiBase(): string {
  if (typeof window !== "undefined") {
    return (window as any).__API_BASE_URL__ || "";
  }
  return "";
}

let authToken: string | null = null;
const WORKSPACE_STORAGE_KEY = "borg_workspace_id";

function getStorage(): Storage | null {
  if (typeof localStorage !== "undefined") return localStorage;
  return null;
}

// Try to restore JWT from localStorage, then fall back to shared token
export const tokenReady: Promise<void> = (async () => {
  const storage = getStorage();
  const stored = storage?.getItem("borg_jwt");
  if (stored) {
    authToken = stored;
    return;
  }
  try {
    const r = await fetch(`${apiBase()}/api/auth/token`);
    if (r.ok) {
      const data = await r.json();
      if (data?.token) authToken = data.token;
    }
  } catch {}
})();

export function setAuthToken(token: string | null) {
  authToken = token;
  const storage = getStorage();
  if (token) {
    storage?.setItem("borg_jwt", token);
  } else {
    storage?.removeItem("borg_jwt");
  }
}

export function getAuthToken(): string | null {
  return authToken;
}

export function getSelectedWorkspaceId(): number | null {
  const raw = getStorage()?.getItem(WORKSPACE_STORAGE_KEY);
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

export function setSelectedWorkspaceId(workspaceId: number | null) {
  const storage = getStorage();
  if (workspaceId && workspaceId > 0) {
    storage?.setItem(WORKSPACE_STORAGE_KEY, String(workspaceId));
  } else {
    storage?.removeItem(WORKSPACE_STORAGE_KEY);
  }
}

function authOnlyHeaders(): Record<string, string> {
  return authToken ? { Authorization: `Bearer ${authToken}` } : {};
}

export function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = { ...authOnlyHeaders() };
  const workspaceId = getSelectedWorkspaceId();
  if (workspaceId) {
    headers["X-Workspace-Id"] = String(workspaceId);
  }
  return headers;
}

const HTTP_STATUS_TEXT: Record<number, string> = {
  400: "Bad Request",
  401: "Unauthorized",
  403: "Forbidden",
  404: "Not Found",
  408: "Request Timeout",
  429: "Too Many Requests",
  500: "Internal Server Error",
  502: "Bad Gateway",
  503: "Service Unavailable",
  504: "Gateway Timeout",
};

export async function fetchJson<T>(path: string): Promise<T> {
  await tokenReady;
  const res = await fetch(`${apiBase()}${path}`, { headers: authHeaders() });
  if (!res.ok) {
    const reason = HTTP_STATUS_TEXT[res.status] ?? res.statusText ?? "Unknown";
    let body = "";
    try {
      body = (await res.text()).trim().slice(0, 300);
    } catch {}
    const detail = body ? `\n${body}` : "";
    throw new Error(`Error ${res.status}: ${reason}${detail}`);
  }
  return res.json();
}

export async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  await tokenReady;
  const { headers: extraHeaders, ...rest } = init ?? {};
  return fetch(`${apiBase()}${path}`, {
    headers: { ...authHeaders(), ...(extraHeaders as Record<string, string> | undefined) },
    ...rest,
  });
}

// AuthEventSource replaces native EventSource with a fetch-based connection
// that sends the token in Authorization header instead of a query parameter.
export class AuthEventSource {
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;

  private controller = new AbortController();

  constructor(path: string) {
    this._connect(path);
  }

  private async _connect(path: string) {
    if (this.controller.signal.aborted) return;
    try {
      const res = await fetch(`${apiBase()}${path}`, {
        headers: authHeaders(),
        signal: this.controller.signal,
      });
      if (!res.ok || !res.body) {
        this.onerror?.();
        return;
      }
      this.onopen?.();
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buf = "";
      let data = "";
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        const lines = buf.split("\n");
        buf = lines.pop() ?? "";
        for (const line of lines) {
          if (line.startsWith("data:")) {
            data = line.slice(5).trimStart();
          } else if (line === "" && data) {
            this.onmessage?.({ data });
            data = "";
          }
        }
      }
      if (!this.controller.signal.aborted) this.onerror?.();
    } catch {
      if (!this.controller.signal.aborted) this.onerror?.();
    }
  }

  close() {
    this.controller.abort();
  }
}

export function normalizeLogEvent(raw: unknown): import("./types").LogEvent | null {
  if (!raw || typeof raw !== "object") return null;
  const data = raw as Record<string, unknown>;
  const level = typeof data.level === "string" && data.level.length > 0 ? data.level : "info";
  const message = typeof data.message === "string" ? data.message : "";

  let ts: number | null = null;
  if (typeof data.ts === "number" && Number.isFinite(data.ts)) ts = data.ts;
  if (typeof data.ts === "string") {
    const parsed = Number(data.ts);
    if (Number.isFinite(parsed)) ts = parsed;
  }
  if (ts === null) ts = Math.floor(Date.now() / 1000);

  return {
    level,
    message,
    ts,
    category: typeof data.category === "string" ? data.category : undefined,
    metadata: typeof data.metadata === "string" ? data.metadata : undefined,
  };
}

// ── Auth API ─────────────────────────────────────────────────────────────

export interface AuthStatus {
  needs_setup: boolean;
  user_count: number;
  auth_disabled?: boolean;
  auth_mode?: "disabled" | "local" | "cloudflare_access";
  sso_providers?: ("google" | "microsoft")[];
}

export interface AuthUser {
  id: number;
  username: string;
  display_name?: string;
  is_admin: boolean;
  default_workspace_id?: number;
  workspace?: {
    id: number;
    name: string;
    kind: string;
    role: string;
    is_default: boolean;
  };
}

export interface LoginResponse {
  token: string;
  user: AuthUser;
  error?: string;
}

export async function fetchAuthStatus(): Promise<AuthStatus> {
  const r = await fetch(`${apiBase()}/api/auth/status`);
  return r.json();
}

export function startSsoLogin(provider: "google" | "microsoft") {
  if (typeof window !== "undefined") {
    window.location.href = `${apiBase()}/api/auth/sso/${provider}/start`;
  }
}

export async function loginUser(username: string, password: string): Promise<LoginResponse> {
  const r = await fetch(`${apiBase()}/api/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username, password }),
  });
  return r.json();
}

export async function setupAdmin(username: string, password: string, display_name?: string): Promise<LoginResponse> {
  const r = await fetch(`${apiBase()}/api/auth/setup`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username, password, display_name }),
  });
  return r.json();
}

export async function fetchMe(): Promise<AuthUser | null> {
  try {
    const r = await fetch(`${apiBase()}/api/auth/me`, { headers: authOnlyHeaders() });
    if (!r.ok) return null;
    return r.json();
  } catch {
    return null;
  }
}

export interface WorkspaceSummary {
  workspace_id: number;
  name: string;
  slug: string;
  kind: string;
  role: string;
  is_default: boolean;
  created_at: string;
}

export interface WorkspaceListResponse {
  workspaces: WorkspaceSummary[];
  default_workspace_id: number;
}

export async function fetchWorkspaces(): Promise<WorkspaceListResponse> {
  return fetchJson("/api/workspaces");
}

export async function switchWorkspace(workspaceId: number): Promise<{ ok: boolean; workspace_id: number }> {
  const r = await apiFetch(`/api/workspaces/${workspaceId}/select`, { method: "PUT" });
  const data = await r.json();
  if (r.ok) {
    setSelectedWorkspaceId(data.workspace_id ?? workspaceId);
  }
  return data;
}
