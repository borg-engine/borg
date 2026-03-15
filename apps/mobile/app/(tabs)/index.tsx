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
import Animated, {
  FadeIn,
  FadeInUp,
  useSharedValue,
  useAnimatedStyle,
  withTiming,
  withSpring,
} from "react-native-reanimated";
import { Gesture, GestureDetector } from "react-native-gesture-handler";
import { useTasks, useStatus } from "@/lib/query";
import { TaskCard } from "@/components/TaskCard";
import { FilterChips } from "@/components/FilterChips";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonList } from "@/components/ui/Skeleton";
import { ErrorScreen } from "@/components/ErrorScreen";
import { lightImpact, selectionFeedback } from "@/lib/haptics";
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

function CountBadge({ count, color }: { count: number; color: string }) {
  if (count === 0) return null;
  return (
    <View style={[summaryStyles.countBadge, { backgroundColor: color + '18' }]}>
      <Text style={[summaryStyles.countText, { color }]}>{count}</Text>
    </View>
  );
}

function StatusSummary() {
  const { data: status } = useStatus();
  if (!status) return null;

  return (
    <Animated.View entering={FadeInUp.duration(400)}>
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
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    paddingVertical: spacing.md,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 4,
    elevation: 2,
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
  countBadge: {
    minWidth: 20,
    height: 20,
    borderRadius: 10,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: 6,
  },
  countText: {
    fontSize: 11,
    fontWeight: "700",
    fontVariant: ["tabular-nums"],
  },
});

function AnimatedFAB() {
  const scale = useSharedValue(1);

  const animStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const gesture = Gesture.Tap()
    .onBegin(() => {
      scale.value = withTiming(0.9, { duration: 80 });
    })
    .onFinalize(() => {
      scale.value = withSpring(1, { damping: 12, stiffness: 200 });
    })
    .onEnd(() => {
      router.push("/task/create" as any);
    });

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View style={[styles.fab, animStyle]}>
        <Ionicons name="add" size={28} color={colors.textInverse} />
      </Animated.View>
    </GestureDetector>
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

  const handleFilterSelect = useCallback((key: string) => {
    setFilter(key as FilterKey);
    selectionFeedback();
  }, []);

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
          onSelect={handleFilterSelect}
        />
      </>
    ),
    [chips, filter, handleFilterSelect],
  );

  if (isLoading) {
    return (
      <View style={common.screen}>
        <View style={styles.skeletonContainer}>
          <SkeletonList count={4} />
        </View>
      </View>
    );
  }

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
            progressBackgroundColor={colors.bgCard}
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
      <AnimatedFAB />
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
  skeletonContainer: {
    padding: spacing.lg,
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
    shadowOpacity: 0.35,
    shadowRadius: 10,
  },
});
