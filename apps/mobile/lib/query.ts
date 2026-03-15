import { QueryClient, useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchTasks,
  fetchTask,
  fetchProjects,
  fetchProject,
  fetchProjectTasks,
  fetchProjectFiles,
  fetchChatThreads,
  fetchChatMessages,
  sendChatMessage,
  fetchStatus,
  fetchModes,
  fetchUsage,
  createTask,
  retryTask,
  cancelTask,
  approveTask,
  rejectTask,
} from "./api";
import type { Task, TaskDetail, Project, ProjectTask, ProjectFilePage, Status, PipelineMode, UsageSummary } from "@borg/api";
import type { ChatThread, ChatMessage } from "./api";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 2,
      refetchOnWindowFocus: true,
    },
  },
});

// Tasks
export function useTasks() {
  return useQuery<Task[]>({
    queryKey: ["tasks"],
    queryFn: fetchTasks,
  });
}

export function useTask(id: number) {
  return useQuery<TaskDetail>({
    queryKey: ["task", id],
    queryFn: () => fetchTask(id),
    enabled: id > 0,
    refetchInterval: 10_000,
  });
}

// Status
export function useStatus() {
  return useQuery<Status>({
    queryKey: ["status"],
    queryFn: fetchStatus,
    refetchInterval: 30_000,
  });
}

// Projects
export function useProjects() {
  return useQuery<Project[]>({
    queryKey: ["projects"],
    queryFn: fetchProjects,
  });
}

export function useProject(id: number) {
  return useQuery<Project>({
    queryKey: ["project", id],
    queryFn: () => fetchProject(id),
    enabled: id > 0,
  });
}

export function useProjectTasks(id: number) {
  return useQuery<ProjectTask[]>({
    queryKey: ["project-tasks", id],
    queryFn: () => fetchProjectTasks(id),
    enabled: id > 0,
  });
}

export function useProjectFiles(id: number, offset = 0, limit = 50) {
  return useQuery<ProjectFilePage>({
    queryKey: ["project-files", id, offset, limit],
    queryFn: () => fetchProjectFiles(id, offset, limit),
    enabled: id > 0,
  });
}

// Chat
export function useChatThreads() {
  return useQuery<ChatThread[]>({
    queryKey: ["chat-threads"],
    queryFn: fetchChatThreads,
    refetchInterval: 15_000,
  });
}

export function useChatMessages(thread: string) {
  return useQuery<ChatMessage[]>({
    queryKey: ["chat-messages", thread],
    queryFn: () => fetchChatMessages(thread),
    enabled: !!thread,
    refetchInterval: 5_000,
  });
}

// Pipeline modes
export function useModes() {
  return useQuery<PipelineMode[]>({
    queryKey: ["modes"],
    queryFn: fetchModes,
    staleTime: 300_000,
  });
}

// Usage
export function useUsage() {
  return useQuery<UsageSummary>({
    queryKey: ["usage"],
    queryFn: fetchUsage,
    staleTime: 60_000,
  });
}

// Mutations
export function useCreateTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: createTask,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

export function useRetryTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => retryTask(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

export function useCancelTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => cancelTask(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

export function useApproveTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => approveTask(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

export function useRejectTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, reason }: { id: number; reason?: string }) => rejectTask(id, reason),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

export function useSendChatMessage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ thread, content }: { thread: string; content: string }) =>
      sendChatMessage(thread, content),
    onSuccess: (_, variables) => {
      qc.invalidateQueries({ queryKey: ["chat-messages", variables.thread] });
      qc.invalidateQueries({ queryKey: ["chat-threads"] });
    },
  });
}
