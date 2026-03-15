export const endpoints = {
  // Tasks
  tasks: "/api/tasks",
  task: (id: number) => `/api/tasks/${id}`,
  taskRetry: (id: number) => `/api/tasks/${id}/retry`,
  taskCancel: (id: number) => `/api/tasks/${id}/cancel`,
  taskApprove: (id: number) => `/api/tasks/${id}/approve`,
  taskReject: (id: number) => `/api/tasks/${id}/reject`,
  taskStream: (id: number) => `/api/tasks/${id}/stream`,

  // Projects
  projects: "/api/projects",
  project: (id: number) => `/api/projects/${id}`,
  projectTasks: (id: number) => `/api/projects/${id}/tasks`,
  projectFiles: (id: number, offset = 0, limit = 50) =>
    `/api/projects/${id}/files?offset=${offset}&limit=${limit}`,

  // Chat
  chat: "/api/chat",
  chatThreads: "/api/chat/threads",
  chatMessages: (thread: string) =>
    `/api/chat/messages?thread=${encodeURIComponent(thread)}`,
  chatEvents: (thread: string) =>
    `/api/chat/events?thread=${encodeURIComponent(thread)}`,

  // Status / modes / usage
  status: "/api/status",
  modes: "/api/modes",
  usage: "/api/usage",
  proposals: "/api/proposals",

  // Auth
  authStatus: "/api/auth/status",
  authLogin: "/api/auth/login",
  authSetup: "/api/auth/setup",
  authToken: "/api/auth/token",
};
