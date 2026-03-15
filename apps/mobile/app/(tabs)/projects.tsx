import React, { useState, useMemo, useCallback } from "react";
import {
  View,
  Text,
  FlatList,
  Pressable,
  TextInput,
  StyleSheet,
  RefreshControl,
} from "react-native";
import { router } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, {
  FadeInDown,
  useSharedValue,
  useAnimatedStyle,
  withTiming,
} from "react-native-reanimated";
import { Gesture, GestureDetector } from "react-native-gesture-handler";
import { useProjects } from "@/lib/query";
import { ModeBadge } from "@/components/ModeBadge";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonList } from "@/components/ui/Skeleton";
import { ErrorScreen } from "@/components/ErrorScreen";
import { lightImpact } from "@/lib/haptics";
import { colors, spacing, radius, common } from "@/lib/theme";
import { timeAgo } from "@/lib/utils";
import type { Project } from "@borg/api";

function ProjectCard({ project, index }: { project: Project; index: number }) {
  const counts = project.task_counts;
  const scale = useSharedValue(1);

  const animStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const gesture = Gesture.Tap()
    .onBegin(() => {
      scale.value = withTiming(0.975, { duration: 80 });
    })
    .onFinalize(() => {
      scale.value = withTiming(1, { duration: 120 });
    })
    .onEnd(() => {
      lightImpact();
      router.push(`/project/${project.id}`);
    });

  return (
    <Animated.View entering={FadeInDown.duration(300).delay(Math.min(index * 50, 250))}>
      <GestureDetector gesture={gesture}>
        <Animated.View style={[styles.card, animStyle]}>
          <View style={styles.cardHeader}>
            <View style={styles.iconCircle}>
              <Ionicons name="folder" size={20} color={colors.accent} />
            </View>
            <View style={styles.cardHeaderText}>
              <Text style={styles.projectName} numberOfLines={1}>
                {project.name}
              </Text>
              <View style={common.row}>
                <ModeBadge mode={project.mode} />
                {project.jurisdiction && (
                  <Text style={styles.jurisdiction}>{project.jurisdiction}</Text>
                )}
              </View>
            </View>
            <Ionicons name="chevron-forward" size={18} color={colors.textTertiary} />
          </View>

          {counts && (
            <View style={styles.countsRow}>
              <CountPill label="Active" count={counts.active} color={colors.statusActive} />
              <CountPill label="Review" count={counts.review} color={colors.statusReview} />
              <CountPill label="Done" count={counts.done} color={colors.statusDone} />
              <CountPill label="Failed" count={counts.failed} color={colors.statusFailed} />
            </View>
          )}

          <Text style={styles.createdAt}>Created {timeAgo(project.created_at)}</Text>
        </Animated.View>
      </GestureDetector>
    </Animated.View>
  );
}

function CountPill({ label, count, color }: { label: string; count: number; color: string }) {
  if (count === 0) return null;
  return (
    <View style={styles.countPill}>
      <View style={[styles.countDot, { backgroundColor: color }]} />
      <Text style={styles.countText}>
        {count} {label}
      </Text>
    </View>
  );
}

export default function ProjectsScreen() {
  const { data: projects, isLoading, error, refetch, isRefetching } = useProjects();
  const [search, setSearch] = useState("");

  const filtered = useMemo(() => {
    if (!projects) return [];
    if (!search.trim()) return projects;
    const q = search.toLowerCase();
    return projects.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.mode.toLowerCase().includes(q) ||
        (p.jurisdiction && p.jurisdiction.toLowerCase().includes(q)),
    );
  }, [projects, search]);

  const renderItem = useCallback(
    ({ item, index }: { item: Project; index: number }) => (
      <ProjectCard project={item} index={index} />
    ),
    [],
  );

  if (isLoading) {
    return (
      <View style={common.screen}>
        <View style={styles.skeletonContainer}>
          <SkeletonList count={3} />
        </View>
      </View>
    );
  }

  if (error) return <ErrorScreen message={error.message} onRetry={refetch} />;

  return (
    <View style={common.screen}>
      <View style={styles.searchContainer}>
        <View style={styles.searchBar}>
          <Ionicons name="search" size={18} color={colors.textTertiary} />
          <TextInput
            style={styles.searchInput}
            placeholder="Search projects..."
            placeholderTextColor={colors.textTertiary}
            value={search}
            onChangeText={setSearch}
            autoCapitalize="none"
            autoCorrect={false}
            selectionColor={colors.accent}
          />
          {search.length > 0 && (
            <Pressable onPress={() => setSearch("")} hitSlop={8}>
              <Ionicons name="close-circle" size={18} color={colors.textTertiary} />
            </Pressable>
          )}
        </View>
      </View>

      <FlatList
        data={filtered}
        keyExtractor={(item) => String(item.id)}
        renderItem={renderItem}
        contentContainerStyle={[
          styles.listContent,
          filtered.length === 0 && styles.listEmpty,
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
            icon="folder-open-outline"
            title="No projects"
            subtitle={search ? "No projects match your search" : "Projects will appear here once created"}
          />
        }
        showsVerticalScrollIndicator={false}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  searchContainer: {
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
  },
  searchBar: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: colors.bgInput,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    paddingHorizontal: spacing.md,
    gap: spacing.sm,
  },
  searchInput: {
    flex: 1,
    color: colors.text,
    fontSize: 15,
    paddingVertical: 12,
  },
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
  card: {
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.lg,
    gap: spacing.md,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 4,
    elevation: 2,
  },
  cardHeader: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.md,
  },
  iconCircle: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: colors.accentBg,
    alignItems: "center",
    justifyContent: "center",
  },
  cardHeaderText: {
    flex: 1,
    gap: spacing.xs,
  },
  projectName: {
    fontSize: 16,
    fontWeight: "600",
    color: colors.text,
  },
  jurisdiction: {
    fontSize: 12,
    color: colors.textTertiary,
    marginLeft: spacing.sm,
  },
  countsRow: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: spacing.sm,
  },
  countPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
    borderRadius: radius.sm,
    backgroundColor: colors.bgHover,
  },
  countDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  countText: {
    fontSize: 11,
    color: colors.textSecondary,
    fontWeight: "500",
  },
  createdAt: {
    fontSize: 11,
    color: colors.textTertiary,
  },
});
