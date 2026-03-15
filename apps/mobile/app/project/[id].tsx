import React, { useState, useCallback } from "react";
import {
  View,
  Text,
  FlatList,
  Pressable,
  ScrollView,
  StyleSheet,
  RefreshControl,
} from "react-native";
import { useLocalSearchParams, router, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeIn, FadeInDown } from "react-native-reanimated";
import { useProject, useProjectTasks, useProjectFiles } from "@/lib/query";
import { StatusBadge } from "@/components/StatusBadge";
import { ModeBadge } from "@/components/ModeBadge";
import { EmptyState } from "@/components/EmptyState";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { colors, spacing, radius, common } from "@/lib/theme";
import { timeAgo } from "@/lib/utils";
import type { ProjectTask, ProjectFile } from "@borg/api";

type Tab = "tasks" | "files";

function TaskItem({ task }: { task: ProjectTask }) {
  return (
    <Pressable
      style={styles.taskItem}
      onPress={() => router.push(`/task/${task.id}`)}
    >
      <View style={common.rowBetween}>
        <Text style={styles.taskTitle} numberOfLines={1}>
          {task.title}
        </Text>
        <StatusBadge status={task.status} />
      </View>
      <View style={styles.taskMeta}>
        <Text style={styles.taskMetaText}>#{task.id}</Text>
        <Text style={styles.taskMetaText}>{timeAgo(task.created_at)}</Text>
      </View>
    </Pressable>
  );
}

function FileItem({ file }: { file: ProjectFile }) {
  const sizeStr =
    file.size_bytes < 1024
      ? `${file.size_bytes} B`
      : file.size_bytes < 1048576
        ? `${(file.size_bytes / 1024).toFixed(1)} KB`
        : `${(file.size_bytes / 1048576).toFixed(1)} MB`;

  const icon = file.mime_type.startsWith("image/")
    ? "image-outline"
    : file.mime_type === "application/pdf"
      ? "document-outline"
      : "document-text-outline";

  return (
    <View style={styles.fileItem}>
      <Ionicons name={icon as any} size={20} color={colors.textSecondary} />
      <View style={styles.fileInfo}>
        <Text style={styles.fileName} numberOfLines={1}>
          {file.file_name}
        </Text>
        <View style={common.row}>
          <Text style={styles.fileMeta}>{sizeStr}</Text>
          {file.privileged && (
            <View style={styles.privilegedBadge}>
              <Ionicons name="lock-closed" size={10} color={colors.warning} />
              <Text style={styles.privilegedText}>Privileged</Text>
            </View>
          )}
        </View>
      </View>
    </View>
  );
}

export default function ProjectDetailScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const projectId = Number(id);
  const [tab, setTab] = useState<Tab>("tasks");

  const { data: project, isLoading: loadingProject, error: projectError, refetch: refetchProject } = useProject(projectId);
  const { data: tasks, isLoading: loadingTasks, refetch: refetchTasks } = useProjectTasks(projectId);
  const { data: filePage, isLoading: loadingFiles, refetch: refetchFiles } = useProjectFiles(projectId);

  const files = filePage?.items ?? [];

  const isLoading = loadingProject;
  const isRefreshing = false;

  const refetchAll = useCallback(() => {
    refetchProject();
    refetchTasks();
    refetchFiles();
  }, [refetchProject, refetchTasks, refetchFiles]);

  if (isLoading) return <LoadingScreen />;
  if (projectError || !project) {
    return <ErrorScreen message={projectError?.message} onRetry={refetchProject} />;
  }

  return (
    <>
      <Stack.Screen
        options={{
          headerTitle: project.name,
        }}
      />
      <View style={common.screen}>
        <Animated.View entering={FadeIn.duration(300)}>
          <View style={styles.header}>
            <View style={common.rowBetween}>
              <ModeBadge mode={project.mode} />
              {project.jurisdiction && (
                <Text style={styles.jurisdiction}>{project.jurisdiction}</Text>
              )}
            </View>
            {project.task_counts && (
              <View style={styles.statsRow}>
                <StatBox label="Total" value={project.task_counts.total} />
                <StatBox label="Active" value={project.task_counts.active} color={colors.statusActive} />
                <StatBox label="Done" value={project.task_counts.done} color={colors.statusDone} />
                <StatBox label="Failed" value={project.task_counts.failed} color={colors.statusFailed} />
              </View>
            )}
          </View>
        </Animated.View>

        <View style={styles.tabBar}>
          <Pressable
            style={[styles.tabItem, tab === "tasks" && styles.tabItemActive]}
            onPress={() => setTab("tasks")}
          >
            <Ionicons
              name="list"
              size={16}
              color={tab === "tasks" ? colors.accent : colors.textTertiary}
            />
            <Text style={[styles.tabText, tab === "tasks" && styles.tabTextActive]}>
              Tasks {tasks ? `(${tasks.length})` : ""}
            </Text>
          </Pressable>
          <Pressable
            style={[styles.tabItem, tab === "files" && styles.tabItemActive]}
            onPress={() => setTab("files")}
          >
            <Ionicons
              name="document-text"
              size={16}
              color={tab === "files" ? colors.accent : colors.textTertiary}
            />
            <Text style={[styles.tabText, tab === "files" && styles.tabTextActive]}>
              Files {filePage ? `(${filePage.total})` : ""}
            </Text>
          </Pressable>
        </View>

        {tab === "tasks" && (
          <FlatList
            data={tasks ?? []}
            keyExtractor={(item) => String(item.id)}
            renderItem={({ item }) => <TaskItem task={item} />}
            contentContainerStyle={[
              styles.listContent,
              (!tasks || tasks.length === 0) && styles.listEmpty,
            ]}
            refreshControl={
              <RefreshControl
                refreshing={isRefreshing}
                onRefresh={refetchAll}
                tintColor={colors.accent}
                colors={[colors.accent]}
              />
            }
            ListEmptyComponent={
              loadingTasks ? (
                <LoadingScreen />
              ) : (
                <EmptyState
                  icon="document-text-outline"
                  title="No tasks yet"
                  subtitle="Tasks for this project will appear here"
                />
              )
            }
            showsVerticalScrollIndicator={false}
            ItemSeparatorComponent={() => <View style={common.separator} />}
          />
        )}

        {tab === "files" && (
          <FlatList
            data={files}
            keyExtractor={(item) => String(item.id)}
            renderItem={({ item }) => <FileItem file={item} />}
            contentContainerStyle={[
              styles.listContent,
              files.length === 0 && styles.listEmpty,
            ]}
            refreshControl={
              <RefreshControl
                refreshing={isRefreshing}
                onRefresh={refetchAll}
                tintColor={colors.accent}
                colors={[colors.accent]}
              />
            }
            ListEmptyComponent={
              loadingFiles ? (
                <LoadingScreen />
              ) : (
                <EmptyState
                  icon="folder-open-outline"
                  title="No files"
                  subtitle="Project files will appear here"
                />
              )
            }
            showsVerticalScrollIndicator={false}
            ItemSeparatorComponent={() => <View style={common.separator} />}
          />
        )}
      </View>
    </>
  );
}

function StatBox({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color?: string;
}) {
  return (
    <View style={styles.statBox}>
      <Text style={[styles.statValue, color ? { color } : null]}>{value}</Text>
      <Text style={styles.statLabel}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  header: {
    padding: spacing.lg,
    gap: spacing.md,
  },
  jurisdiction: {
    fontSize: 13,
    color: colors.textTertiary,
  },
  statsRow: {
    flexDirection: "row",
    gap: spacing.sm,
  },
  statBox: {
    flex: 1,
    backgroundColor: colors.bgElevated,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    paddingVertical: spacing.md,
    alignItems: "center",
  },
  statValue: {
    fontSize: 20,
    fontWeight: "700",
    color: colors.text,
    fontVariant: ["tabular-nums"],
  },
  statLabel: {
    fontSize: 11,
    color: colors.textTertiary,
    marginTop: 2,
  },
  tabBar: {
    flexDirection: "row",
    borderBottomWidth: 1,
    borderBottomColor: colors.border,
    marginHorizontal: spacing.lg,
  },
  tabItem: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: spacing.sm,
    paddingVertical: spacing.md,
    borderBottomWidth: 2,
    borderBottomColor: "transparent",
  },
  tabItemActive: {
    borderBottomColor: colors.accent,
  },
  tabText: {
    fontSize: 14,
    fontWeight: "500",
    color: colors.textTertiary,
  },
  tabTextActive: {
    color: colors.accent,
  },
  listContent: {
    paddingHorizontal: spacing.lg,
    paddingBottom: 100,
  },
  listEmpty: {
    flexGrow: 1,
  },
  taskItem: {
    paddingVertical: spacing.md,
    gap: spacing.xs,
  },
  taskTitle: {
    fontSize: 14,
    fontWeight: "500",
    color: colors.text,
    flex: 1,
    marginRight: spacing.sm,
  },
  taskMeta: {
    flexDirection: "row",
    gap: spacing.md,
  },
  taskMetaText: {
    fontSize: 12,
    color: colors.textTertiary,
  },
  fileItem: {
    flexDirection: "row",
    alignItems: "center",
    paddingVertical: spacing.md,
    gap: spacing.md,
  },
  fileInfo: {
    flex: 1,
    gap: 2,
  },
  fileName: {
    fontSize: 14,
    fontWeight: "500",
    color: colors.text,
  },
  fileMeta: {
    fontSize: 12,
    color: colors.textTertiary,
  },
  privilegedBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
    marginLeft: spacing.sm,
  },
  privilegedText: {
    fontSize: 10,
    color: colors.warning,
    fontWeight: "500",
  },
});
