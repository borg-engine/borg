import React, { useState, useMemo, useCallback } from "react";
import {
  View,
  Text,
  FlatList,
  Pressable,
  StyleSheet,
  RefreshControl,
} from "react-native";
import { router } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeInDown } from "react-native-reanimated";
import { useTasks, useCreateTask } from "@/lib/query";
import { FilterChips } from "@/components/FilterChips";
import { StatusBadge } from "@/components/StatusBadge";
import { ModeBadge } from "@/components/ModeBadge";
import { EmptyState } from "@/components/EmptyState";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { colors, spacing, radius, common } from "@/lib/theme";
import { timeAgo } from "@/lib/utils";
import type { Task } from "@borg/api";
import { isActiveStatus } from "@borg/api";

type FilterKey = "all" | "active" | "review" | "done" | "failed";

function filterTasks(tasks: Task[], filter: FilterKey): Task[] {
  switch (filter) {
    case "active":
      return tasks.filter((t) => isActiveStatus(t.status) && t.status !== "review" && t.status !== "human_review");
    case "review":
      return tasks.filter((t) => t.status === "review" || t.status === "human_review");
    case "done":
      return tasks.filter((t) => t.status === "done" || t.status === "merged");
    case "failed":
      return tasks.filter((t) => t.status === "failed");
    default:
      return tasks;
  }
}

function TaskCard({ task, index }: { task: Task; index: number }) {
  return (
    <Animated.View entering={FadeInDown.duration(300).delay(index * 50)}>
      <Pressable
        style={styles.card}
        onPress={() => router.push(`/task/${task.id}`)}
        android_ripple={{ color: colors.bgHover }}
      >
        <View style={styles.cardHeader}>
          <View style={styles.cardTitleRow}>
            <Text style={styles.taskId}>#{task.id}</Text>
            <StatusBadge status={task.status} />
          </View>
          {task.mode && <ModeBadge mode={task.mode} />}
        </View>
        <Text style={styles.taskTitle} numberOfLines={2}>
          {task.title}
        </Text>
        {task.description ? (
          <Text style={styles.taskDesc} numberOfLines={2}>
            {task.description}
          </Text>
        ) : null}
        <View style={styles.cardFooter}>
          <View style={styles.metaRow}>
            <Ionicons name="time-outline" size={12} color={colors.textTertiary} />
            <Text style={styles.metaText}>{timeAgo(task.created_at)}</Text>
          </View>
          {task.duration_secs !== undefined && task.duration_secs > 0 && (
            <View style={styles.metaRow}>
              <Ionicons name="hourglass-outline" size={12} color={colors.textTertiary} />
              <Text style={styles.metaText}>
                {task.duration_secs < 60
                  ? `${task.duration_secs}s`
                  : `${Math.floor(task.duration_secs / 60)}m`}
              </Text>
            </View>
          )}
          <View style={styles.metaRow}>
            <Ionicons name="repeat-outline" size={12} color={colors.textTertiary} />
            <Text style={styles.metaText}>
              {task.attempt}/{task.max_attempts}
            </Text>
          </View>
        </View>
      </Pressable>
    </Animated.View>
  );
}

export default function TasksScreen() {
  const { data: tasks, isLoading, error, refetch, isRefetching } = useTasks();
  const [filter, setFilter] = useState<FilterKey>("all");

  const filteredTasks = useMemo(
    () => filterTasks(tasks ?? [], filter),
    [tasks, filter],
  );

  const chipCounts = useMemo(() => {
    if (!tasks) return {};
    return {
      all: tasks.length,
      active: tasks.filter((t) => isActiveStatus(t.status) && t.status !== "review" && t.status !== "human_review").length,
      review: tasks.filter((t) => t.status === "review" || t.status === "human_review").length,
      done: tasks.filter((t) => t.status === "done" || t.status === "merged").length,
      failed: tasks.filter((t) => t.status === "failed").length,
    };
  }, [tasks]);

  const chips = [
    { key: "all", label: "All", count: chipCounts.all },
    { key: "active", label: "Active", count: chipCounts.active },
    { key: "review", label: "Review", count: chipCounts.review },
    { key: "done", label: "Done", count: chipCounts.done },
    { key: "failed", label: "Failed", count: chipCounts.failed },
  ];

  const renderItem = useCallback(
    ({ item, index }: { item: Task; index: number }) => (
      <TaskCard task={item} index={index} />
    ),
    [],
  );

  if (isLoading) return <LoadingScreen />;
  if (error) return <ErrorScreen message={error.message} onRetry={refetch} />;

  return (
    <View style={common.screen}>
      <FilterChips
        chips={chips}
        selected={filter}
        onSelect={(k) => setFilter(k as FilterKey)}
      />
      <FlatList
        data={filteredTasks}
        keyExtractor={(item) => String(item.id)}
        renderItem={renderItem}
        contentContainerStyle={[
          styles.listContent,
          filteredTasks.length === 0 && styles.listEmpty,
        ]}
        refreshControl={
          <RefreshControl
            refreshing={isRefetching}
            onRefresh={refetch}
            tintColor={colors.accent}
            colors={[colors.accent]}
          />
        }
        ListEmptyComponent={
          <EmptyState
            icon="document-text-outline"
            title="No tasks"
            subtitle={
              filter === "all"
                ? "Tasks created by the pipeline or manually will appear here"
                : `No ${filter} tasks`
            }
          />
        }
        showsVerticalScrollIndicator={false}
      />
      <Pressable
        style={styles.fab}
        onPress={() => router.push("/task/create" as any)}
      >
        <Ionicons name="add" size={28} color={colors.textInverse} />
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  listContent: {
    paddingHorizontal: spacing.lg,
    paddingBottom: 100,
    gap: spacing.md,
  },
  listEmpty: {
    flexGrow: 1,
  },
  card: {
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.lg,
  },
  cardHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: spacing.sm,
  },
  cardTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.sm,
  },
  taskId: {
    fontSize: 12,
    fontWeight: "600",
    color: colors.textTertiary,
    fontVariant: ["tabular-nums"],
  },
  taskTitle: {
    fontSize: 15,
    fontWeight: "600",
    color: colors.text,
    lineHeight: 21,
    marginBottom: spacing.xs,
  },
  taskDesc: {
    fontSize: 13,
    color: colors.textSecondary,
    lineHeight: 18,
    marginBottom: spacing.sm,
  },
  cardFooter: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.md,
    marginTop: spacing.sm,
  },
  metaRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  metaText: {
    fontSize: 11,
    color: colors.textTertiary,
  },
  fab: {
    position: "absolute",
    right: spacing.xl,
    bottom: spacing.xl,
    width: 56,
    height: 56,
    borderRadius: 28,
    backgroundColor: colors.accent,
    alignItems: "center",
    justifyContent: "center",
    elevation: 6,
    shadowColor: colors.accent,
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.3,
    shadowRadius: 8,
  },
});
