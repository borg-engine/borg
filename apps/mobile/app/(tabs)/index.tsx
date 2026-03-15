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
import Animated, { FadeIn } from "react-native-reanimated";
import { useTasks, useStatus } from "@/lib/query";
import { TaskCard } from "@/components/TaskCard";
import { FilterChips } from "@/components/FilterChips";
import { EmptyState } from "@/components/EmptyState";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { colors, spacing, radius, common } from "@/lib/theme";
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

function StatusSummary() {
  const { data: status } = useStatus();
  if (!status) return null;

  return (
    <Animated.View entering={FadeIn.duration(400)}>
      <View style={summaryStyles.container}>
        <SummaryItem
          icon="rocket-outline"
          value={status.active_tasks}
          label="Active"
          color={colors.statusActive}
        />
        <View style={summaryStyles.divider} />
        <SummaryItem
          icon="checkmark-done-outline"
          value={status.merged_tasks}
          label="Merged"
          color={colors.statusDone}
        />
        <View style={summaryStyles.divider} />
        <SummaryItem
          icon="people-outline"
          value={status.dispatched_agents}
          label="Agents"
          color={colors.info}
        />
        <View style={summaryStyles.divider} />
        <SummaryItem
          icon="flash-outline"
          value={status.ai_requests}
          label="Requests"
          color={colors.accent}
        />
      </View>
    </Animated.View>
  );
}

function SummaryItem({
  icon,
  value,
  label,
  color,
}: {
  icon: keyof typeof Ionicons.glyphMap;
  value: number;
  label: string;
  color: string;
}) {
  return (
    <View style={summaryStyles.item}>
      <Ionicons name={icon} size={16} color={color} />
      <Text style={[summaryStyles.value, { color }]}>{value}</Text>
      <Text style={summaryStyles.label}>{label}</Text>
    </View>
  );
}

const summaryStyles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: colors.bgCard,
    marginHorizontal: spacing.lg,
    marginTop: spacing.sm,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    paddingVertical: spacing.md,
  },
  item: {
    flex: 1,
    alignItems: "center",
    gap: 2,
  },
  divider: {
    width: 1,
    height: 32,
    backgroundColor: colors.borderSubtle,
  },
  value: {
    fontSize: 18,
    fontWeight: "700",
    fontVariant: ["tabular-nums"],
  },
  label: {
    fontSize: 10,
    color: colors.textTertiary,
    textTransform: "uppercase",
    letterSpacing: 0.3,
  },
});

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

  const listHeader = useCallback(
    () => (
      <>
        <StatusSummary />
        <FilterChips
          chips={chips}
          selected={filter}
          onSelect={(k) => setFilter(k as FilterKey)}
        />
      </>
    ),
    [chips, filter],
  );

  if (isLoading) return <LoadingScreen />;
  if (error) return <ErrorScreen message={error.message} onRetry={refetch} />;

  return (
    <View style={common.screen}>
      <FlatList
        data={filteredTasks}
        keyExtractor={(item) => String(item.id)}
        renderItem={renderItem}
        ListHeaderComponent={listHeader}
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
