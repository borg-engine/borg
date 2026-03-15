import { getServerUrl, getToken } from "./auth";
import type {
  Task,
  TaskDetail,
  Project,
  ProjectTask,
  ProjectFile,
  ProjectFilePage,
  Status,
  Proposal,
  PipelineMode,
  UsageSummary,
} from "@borg/api";

async function baseUrl(): Promise<string> {
  const url = await getServerUrl();
  if (!url) throw new Error("Not connected to a server");
  return url;
}

async function authHeaders(): Promise<Record<string, string>> {
  const token = await getToken();
  const headers: Record<string, string> = {};
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }
  return headers;
}

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
    this.name = "ApiError";
  }
}

async function fetchJson<T>(path: string): Promise<T> {
  const base = await baseUrl();
  const headers = await authHeaders();
  const res = await fetch(`${base}${path}`, { headers });
  if (!res.ok) {
    let body = "";
    try {
      body = (await res.text()).trim().slice(0, 300);
    } catch {}
    throw new ApiError(res.status, body || `HTTP ${res.status}`);
  }
  return res.json();
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  const base = await baseUrl();
  const headers = await authHeaders();
  const res = await fetch(`${base}${path}`, {
    method: "POST",
    headers: { ...headers, "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    let text = "";
    try {
      text = (await res.text()).trim().slice(0, 300);
    } catch {}
    throw new ApiError(res.status, text || `HTTP ${res.status}`);
  }
  return res.json();
}

async function putJson<T>(path: string, body?: unknown): Promise<T> {
  const base = await baseUrl();
  const headers = await authHeaders();
  const res = await fetch(`${base}${path}`, {
    method: "PUT",
    headers: { ...headers, "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) {
    let text = "";
    try {
      text = (await res.text()).trim().slice(0, 300);
    } catch {}
    throw new ApiError(res.status, text || `HTTP ${res.status}`);
  }
  return res.json();
}

// Tasks
export function fetchTasks(): Promise<Task[]> {
  return fetchJson("/api/tasks");
}

export function fetchTask(id: number): Promise<TaskDetail> {
  return fetchJson(`/api/tasks/${id}`);
}

export function createTask(data: {
  title: string;
  description: string;
  repo_path?: string;
  mode?: string;
  project_id?: number;
}): Promise<Task> {
  return postJson("/api/tasks", data);
}

export function retryTask(id: number): Promise<{ ok: boolean }> {
  return postJson(`/api/tasks/${id}/retry`, {});
}

export function cancelTask(id: number): Promise<{ ok: boolean }> {
  return postJson(`/api/tasks/${id}/cancel`, {});
}

export function approveTask(id: number): Promise<{ ok: boolean }> {
  return postJson(`/api/tasks/${id}/approve`, {});
}

export function rejectTask(id: number, reason?: string): Promise<{ ok: boolean }> {
  return postJson(`/api/tasks/${id}/reject`, { reason });
}

// Status
export function fetchStatus(): Promise<Status> {
  return fetchJson("/api/status");
}

// Projects
export function fetchProjects(): Promise<Project[]> {
  return fetchJson("/api/projects");
}

export function fetchProject(id: number): Promise<Project> {
  return fetchJson(`/api/projects/${id}`);
}

export function fetchProjectTasks(id: number): Promise<ProjectTask[]> {
  return fetchJson(`/api/projects/${id}/tasks`);
}

export function fetchProjectFiles(id: number, offset = 0, limit = 50): Promise<ProjectFilePage> {
  return fetchJson(`/api/projects/${id}/files?offset=${offset}&limit=${limit}`);
}

// Pipeline modes
export function fetchModes(): Promise<PipelineMode[]> {
  return fetchJson("/api/modes");
}

// Proposals
export function fetchProposals(): Promise<Proposal[]> {
  return fetchJson("/api/proposals");
}

// Chat
export interface ChatThread {
  thread: string;
  project_id?: number;
  project_name?: string;
  last_message?: string;
  last_at?: string;
  message_count: number;
}

export interface ChatMessage {
  id: number;
  thread: string;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: string;
}

export function fetchChatThreads(): Promise<ChatThread[]> {
  return fetchJson("/api/chat/threads");
}

export function fetchChatMessages(thread: string): Promise<ChatMessage[]> {
  return fetchJson(`/api/chat/messages?thread=${encodeURIComponent(thread)}`);
}

export function sendChatMessage(thread: string, content: string): Promise<ChatMessage> {
  return postJson("/api/chat", { thread, content });
}

// Usage
export function fetchUsage(): Promise<UsageSummary> {
  return fetchJson("/api/usage");
}

// Streaming support for chat
export async function createChatStream(
  thread: string,
  onEvent: (data: string) => void,
  signal?: AbortSignal,
): Promise<void> {
  const base = await baseUrl();
  const headers = await authHeaders();

  const res = await fetch(`${base}/api/chat/events?thread=${encodeURIComponent(thread)}`, {
    headers,
    signal,
  });

  if (!res.ok || !res.body) return;

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
        onEvent(data);
        data = "";
      }
    }
  }
}

// Task stream
export async function createTaskStream(
  taskId: number,
  onLine: (line: string) => void,
  signal?: AbortSignal,
): Promise<void> {
  const base = await baseUrl();
  const headers = await authHeaders();

  const res = await fetch(`${base}/api/tasks/${taskId}/stream`, {
    headers,
    signal,
  });

  if (!res.ok || !res.body) return;

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    const lines = buf.split("\n");
    buf = lines.pop() ?? "";
    for (const line of lines) {
      if (line.trim()) onLine(line);
    }
  }
}
