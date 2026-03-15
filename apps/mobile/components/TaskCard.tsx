import React from "react";
import { View, Text, Pressable, StyleSheet } from "react-native";
import { router } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeInDown } from "react-native-reanimated";
import { StatusBadge } from "./StatusBadge";
import { ModeBadge } from "./ModeBadge";
import { colors, spacing, radius, common } from "@/lib/theme";
import { timeAgo } from "@/lib/utils";
import type { Task } from "@borg/api";

interface Props {
  task: Task;
  index?: number;
  compact?: boolean;
}

export function TaskCard({ task, index = 0, compact = false }: Props) {
  return (
    <Animated.View entering={FadeInDown.duration(300).delay(Math.min(index * 50, 300))}>
      <Pressable
        style={[styles.card, compact && styles.cardCompact]}
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
        <Text style={styles.taskTitle} numberOfLines={compact ? 1 : 2}>
          {task.title}
        </Text>
        {!compact && task.description ? (
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

const styles = StyleSheet.create({
  card: {
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.lg,
  },
  cardCompact: {
    padding: spacing.md,
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
});
