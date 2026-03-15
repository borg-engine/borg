import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { router } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, {
  FadeInDown,
  useSharedValue,
  useAnimatedStyle,
  withTiming,
} from "react-native-reanimated";
import { Gesture, GestureDetector } from "react-native-gesture-handler";
import { StatusBadge } from "./StatusBadge";
import { ModeBadge } from "./ModeBadge";
import { lightImpact } from "@/lib/haptics";
import { colors, spacing, radius, common } from "@/lib/theme";
import { timeAgo } from "@/lib/utils";
import type { Task } from "@borg/api";

interface Props {
  task: Task;
  index?: number;
  compact?: boolean;
}

export function TaskCard({ task, index = 0, compact = false }: Props) {
  const scale = useSharedValue(1);

  const animatedStyle = useAnimatedStyle(() => ({
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
      router.push(`/task/${task.id}`);
    });

  return (
    <Animated.View entering={FadeInDown.duration(300).delay(Math.min(index * 50, 300))}>
      <GestureDetector gesture={gesture}>
        <Animated.View style={[styles.card, compact && styles.cardCompact, animatedStyle]}>
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
        </Animated.View>
      </GestureDetector>
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
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 4,
    elevation: 2,
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
