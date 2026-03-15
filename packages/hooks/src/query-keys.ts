export const queryKeys = {
  tasks: {
    all: ["tasks"] as const,
    detail: (id: number) => ["task", id] as const,
    outputs: (id: number) => ["task", id, "outputs"] as const,
  },
  projects: {
    all: ["projects"] as const,
    detail: (id: number) => ["project", id] as const,
    tasks: (id: number) => ["project-tasks", id] as const,
    files: (id: number, offset = 0, limit = 50) =>
      ["project-files", id, offset, limit] as const,
  },
  chat: {
    threads: ["chat-threads"] as const,
    messages: (thread: string) => ["chat-messages", thread] as const,
  },
  status: {
    current: ["status"] as const,
  },
  modes: {
    all: ["modes"] as const,
  },
  usage: {
    current: ["usage"] as const,
  },
  proposals: {
    all: ["proposals"] as const,
  },
};
