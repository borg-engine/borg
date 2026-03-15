import React, { useState, useEffect, useRef, useCallback } from "react";
import {
  View,
  Text,
  ScrollView,
  Pressable,
  StyleSheet,
  ActivityIndicator,
  Alert,
} from "react-native";
import { useLocalSearchParams, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeInDown, FadeIn } from "react-native-reanimated";
import { useTask, useRetryTask, useCancelTask, useApproveTask, useRejectTask } from "@/lib/query";
import { createTaskStream } from "@/lib/api";
import { StatusBadge } from "@/components/StatusBadge";
import { ModeBadge } from "@/components/ModeBadge";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { colors, spacing, radius, common, statusColor } from "@/lib/theme";
import { timeAgo, formatDuration } from "@/lib/utils";
import { isActiveStatus, getDisplayPhases, getPhaseLabel } from "@borg/api";

function PhaseProgress({ status, mode }: { status: string; mode?: string }) {
  const phases = getDisplayPhases(mode);

  const currentIndex = phases.indexOf(status);

  return (
    <View style={phaseStyles.container}>
      {phases.map((phase, i) => {
        const isComplete = i < currentIndex || status === "done" || status === "merged";
        const isCurrent = phase === status;
        const isFailed = status === "failed" && i === Math.max(0, currentIndex);

        let dotColor = colors.bgHover;
        if (isComplete) dotColor = colors.success;
        if (isCurrent) dotColor = colors.accent;
        if (isFailed) dotColor = colors.error;

        return (
          <View key={phase} style={phaseStyles.step}>
            <View
              style={[
                phaseStyles.dot,
                { backgroundColor: dotColor },
                isCurrent && phaseStyles.dotActive,
              ]}
            >
              {isComplete && (
                <Ionicons name="checkmark" size={10} color={colors.bg} />
              )}
            </View>
            <Text
              style={[
                phaseStyles.label,
                (isCurrent || isComplete) && phaseStyles.labelActive,
                isFailed && phaseStyles.labelFailed,
              ]}
              numberOfLines={1}
            >
              {getPhaseLabel(phase, mode)}
            </Text>
            {i < phases.length - 1 && (
              <View
                style={[
                  phaseStyles.connector,
                  isComplete && phaseStyles.connectorDone,
                ]}
              />
            )}
          </View>
        );
      })}
    </View>
  );
}

const phaseStyles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "flex-start",
    paddingVertical: spacing.md,
  },
  step: {
    flex: 1,
    alignItems: "center",
    position: "relative",
  },
  dot: {
    width: 20,
    height: 20,
    borderRadius: 10,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: 4,
  },
  dotActive: {
    width: 22,
    height: 22,
    borderRadius: 11,
    borderWidth: 2,
    borderColor: "rgba(245, 158, 11, 0.3)",
  },
  label: {
    fontSize: 10,
    color: colors.textTertiary,
    textAlign: "center",
  },
  labelActive: {
    color: colors.text,
    fontWeight: "500",
  },
  labelFailed: {
    color: colors.error,
  },
  connector: {
    position: "absolute",
    top: 10,
    left: "60%",
    right: "-40%",
    height: 2,
    backgroundColor: colors.bgHover,
    zIndex: -1,
  },
  connectorDone: {
    backgroundColor: colors.success,
  },
});

function StreamViewer({ taskId }: { taskId: number }) {
  const [lines, setLines] = useState<string[]>([]);
  const scrollRef = useRef<ScrollView>(null);

  useEffect(() => {
    const controller = new AbortController();
    createTaskStream(
      taskId,
      (line) => {
        try {
          const parsed = JSON.parse(line);
          if (parsed.type === "assistant" && parsed.message) {
            setLines((prev) => [...prev.slice(-200), parsed.message]);
          } else if (parsed.type === "result" && parsed.result) {
            setLines((prev) => [...prev.slice(-200), `[Result] ${parsed.result}`]);
          }
        } catch {
          setLines((prev) => [...prev.slice(-200), line]);
        }
      },
      controller.signal,
    ).catch(() => {});
    return () => controller.abort();
  }, [taskId]);

  useEffect(() => {
    scrollRef.current?.scrollToEnd({ animated: true });
  }, [lines]);

  if (lines.length === 0) {
    return (
      <View style={streamStyles.empty}>
        <ActivityIndicator size="small" color={colors.textTertiary} />
        <Text style={streamStyles.emptyText}>Waiting for output...</Text>
      </View>
    );
  }

  return (
    <ScrollView
      ref={scrollRef}
      style={streamStyles.container}
      contentContainerStyle={streamStyles.content}
      showsVerticalScrollIndicator={false}
    >
      {lines.map((line, i) => (
        <Text key={i} style={streamStyles.line}>
          {line}
        </Text>
      ))}
    </ScrollView>
  );
}

const streamStyles = StyleSheet.create({
  container: {
    maxHeight: 300,
    backgroundColor: colors.bgInput,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
  },
  content: {
    padding: spacing.md,
  },
  line: {
    fontSize: 12,
    fontFamily: "SpaceMono",
    color: colors.textSecondary,
    lineHeight: 18,
  },
  empty: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: spacing.sm,
    paddingVertical: spacing.xl,
  },
  emptyText: {
    fontSize: 13,
    color: colors.textTertiary,
  },
});

export default function TaskDetailScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const taskId = Number(id);
  const { data: task, isLoading, error, refetch } = useTask(taskId);
  const retryMutation = useRetryTask();
  const cancelMutation = useCancelTask();
  const approveMutation = useApproveTask();
  const rejectMutation = useRejectTask();

  const handleRetry = useCallback(() => {
    Alert.alert("Retry Task", "Retry this task?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Retry",
        onPress: () => retryMutation.mutate(taskId),
      },
    ]);
  }, [taskId, retryMutation]);

  const handleCancel = useCallback(() => {
    Alert.alert("Cancel Task", "Cancel this task?", [
      { text: "No", style: "cancel" },
      {
        text: "Cancel Task",
        style: "destructive",
        onPress: () => cancelMutation.mutate(taskId),
      },
    ]);
  }, [taskId, cancelMutation]);

  const handleApprove = useCallback(() => {
    approveMutation.mutate(taskId);
  }, [taskId, approveMutation]);

  const handleReject = useCallback(() => {
    Alert.alert("Reject Task", "Send this task back for revision?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Reject",
        style: "destructive",
        onPress: () => rejectMutation.mutate({ id: taskId }),
      },
    ]);
  }, [taskId, rejectMutation]);

  if (isLoading) return <LoadingScreen />;
  if (error || !task) return <ErrorScreen message={error?.message} onRetry={refetch} />;

  const active = isActiveStatus(task.status);
  const inReview = task.status === "review" || task.status === "human_review";

  return (
    <>
      <Stack.Screen
        options={{
          headerTitle: `Task #${task.id}`,
        }}
      />
      <ScrollView style={common.screen} contentContainerStyle={styles.content}>
        <Animated.View entering={FadeIn.duration(300)}>
          <View style={styles.headerSection}>
            <View style={common.rowBetween}>
              <StatusBadge status={task.status} size="md" />
              {task.mode && <ModeBadge mode={task.mode} />}
            </View>
            <Text style={styles.title}>{task.title}</Text>
            {task.description ? (
              <Text style={styles.description}>{task.description}</Text>
            ) : null}
          </View>

          <PhaseProgress status={task.status} mode={task.mode} />

          <View style={[common.card, styles.metaCard]}>
            <MetaRow label="Created" value={timeAgo(task.created_at)} icon="calendar-outline" />
            {task.started_at && (
              <MetaRow label="Started" value={timeAgo(task.started_at)} icon="play-outline" />
            )}
            {task.duration_secs !== undefined && task.duration_secs > 0 && (
              <MetaRow label="Duration" value={formatDuration(task.duration_secs)} icon="time-outline" />
            )}
            <MetaRow
              label="Attempts"
              value={`${task.attempt} / ${task.max_attempts}`}
              icon="repeat-outline"
            />
            {task.branch && (
              <MetaRow label="Branch" value={task.branch} icon="git-branch-outline" />
            )}
            {task.created_by && (
              <MetaRow label="Created by" value={task.created_by} icon="person-outline" />
            )}
          </View>

          {task.last_error ? (
            <Animated.View entering={FadeInDown.duration(300).delay(100)}>
              <View style={styles.errorCard}>
                <View style={common.row}>
                  <Ionicons name="alert-circle" size={16} color={colors.error} />
                  <Text style={styles.errorLabel}>Last Error</Text>
                </View>
                <Text style={styles.errorText}>{task.last_error}</Text>
              </View>
            </Animated.View>
          ) : null}

          {active && (
            <Animated.View entering={FadeInDown.duration(300).delay(150)}>
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Live Output</Text>
                <StreamViewer taskId={taskId} />
              </View>
            </Animated.View>
          )}

          {task.outputs && task.outputs.length > 0 && (
            <Animated.View entering={FadeInDown.duration(300).delay(200)}>
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Phase Outputs</Text>
                {task.outputs.map((output) => (
                  <View key={output.id} style={styles.outputCard}>
                    <View style={common.rowBetween}>
                      <Text style={styles.outputPhase}>{output.phase}</Text>
                      <View
                        style={[
                          styles.exitBadge,
                          {
                            backgroundColor:
                              output.exit_code === 0 ? colors.successBg : colors.errorBg,
                          },
                        ]}
                      >
                        <Text
                          style={[
                            styles.exitText,
                            {
                              color: output.exit_code === 0 ? colors.success : colors.error,
                            },
                          ]}
                        >
                          exit {output.exit_code}
                        </Text>
                      </View>
                    </View>
                    {output.output ? (
                      <ScrollView
                        horizontal={false}
                        style={styles.outputScroll}
                        nestedScrollEnabled
                      >
                        <Text style={styles.outputText} selectable>
                          {output.output.slice(0, 2000)}
                          {output.output.length > 2000 ? "\n... (truncated)" : ""}
                        </Text>
                      </ScrollView>
                    ) : null}
                  </View>
                ))}
              </View>
            </Animated.View>
          )}

          <View style={styles.actions}>
            {inReview && (
              <>
                <Pressable
                  style={[styles.actionButton, styles.approveButton]}
                  onPress={handleApprove}
                  disabled={approveMutation.isPending}
                >
                  <Ionicons name="checkmark-circle" size={20} color={colors.success} />
                  <Text style={[styles.actionText, { color: colors.success }]}>Approve</Text>
                </Pressable>
                <Pressable
                  style={[styles.actionButton, styles.rejectButton]}
                  onPress={handleReject}
                  disabled={rejectMutation.isPending}
                >
                  <Ionicons name="close-circle" size={20} color={colors.error} />
                  <Text style={[styles.actionText, { color: colors.error }]}>Reject</Text>
                </Pressable>
              </>
            )}
            {active && !inReview && (
              <Pressable
                style={[styles.actionButton, styles.cancelButton]}
                onPress={handleCancel}
                disabled={cancelMutation.isPending}
              >
                <Ionicons name="stop-circle" size={20} color={colors.error} />
                <Text style={[styles.actionText, { color: colors.error }]}>Cancel</Text>
              </Pressable>
            )}
            {(task.status === "failed" || task.status === "done") && (
              <Pressable
                style={[styles.actionButton, styles.retryButton]}
                onPress={handleRetry}
                disabled={retryMutation.isPending}
              >
                <Ionicons name="refresh" size={20} color={colors.accent} />
                <Text style={[styles.actionText, { color: colors.accent }]}>Retry</Text>
              </Pressable>
            )}
          </View>
        </Animated.View>
      </ScrollView>
    </>
  );
}

function MetaRow({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon: keyof typeof Ionicons.glyphMap;
}) {
  return (
    <View style={styles.metaRow}>
      <View style={styles.metaLabel}>
        <Ionicons name={icon} size={14} color={colors.textTertiary} />
        <Text style={styles.metaLabelText}>{label}</Text>
      </View>
      <Text style={styles.metaValue} numberOfLines={1}>
        {value}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  content: {
    padding: spacing.lg,
    paddingBottom: 100,
    gap: spacing.lg,
  },
  headerSection: {
    gap: spacing.sm,
  },
  title: {
    fontSize: 22,
    fontWeight: "700",
    color: colors.text,
    lineHeight: 28,
    marginTop: spacing.sm,
  },
  description: {
    fontSize: 14,
    color: colors.textSecondary,
    lineHeight: 20,
  },
  metaCard: {
    gap: spacing.md,
  },
  metaRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  metaLabel: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.sm,
  },
  metaLabelText: {
    fontSize: 13,
    color: colors.textTertiary,
  },
  metaValue: {
    fontSize: 13,
    color: colors.text,
    fontWeight: "500",
    maxWidth: "55%",
    textAlign: "right",
  },
  errorCard: {
    backgroundColor: colors.errorBg,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: "rgba(239, 68, 68, 0.2)",
    padding: spacing.lg,
    gap: spacing.sm,
  },
  errorLabel: {
    fontSize: 13,
    fontWeight: "600",
    color: colors.error,
    marginLeft: spacing.sm,
  },
  errorText: {
    fontSize: 13,
    color: colors.error,
    lineHeight: 18,
    opacity: 0.85,
  },
  section: {
    gap: spacing.md,
  },
  sectionTitle: {
    fontSize: 15,
    fontWeight: "600",
    color: colors.text,
  },
  outputCard: {
    backgroundColor: colors.bgElevated,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.md,
    gap: spacing.sm,
  },
  outputPhase: {
    fontSize: 13,
    fontWeight: "600",
    color: colors.text,
    textTransform: "capitalize",
  },
  exitBadge: {
    paddingHorizontal: spacing.sm,
    paddingVertical: 2,
    borderRadius: radius.sm,
  },
  exitText: {
    fontSize: 11,
    fontWeight: "600",
  },
  outputScroll: {
    maxHeight: 200,
  },
  outputText: {
    fontSize: 12,
    fontFamily: "SpaceMono",
    color: colors.textSecondary,
    lineHeight: 18,
  },
  actions: {
    flexDirection: "row",
    gap: spacing.md,
    flexWrap: "wrap",
  },
  actionButton: {
    flex: 1,
    minWidth: 120,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: spacing.sm,
    paddingVertical: spacing.md,
    borderRadius: radius.md,
    borderWidth: 1,
  },
  approveButton: {
    borderColor: "rgba(34, 197, 94, 0.3)",
    backgroundColor: colors.successBg,
  },
  rejectButton: {
    borderColor: "rgba(239, 68, 68, 0.3)",
    backgroundColor: colors.errorBg,
  },
  cancelButton: {
    borderColor: "rgba(239, 68, 68, 0.3)",
    backgroundColor: colors.errorBg,
  },
  retryButton: {
    borderColor: "rgba(245, 158, 11, 0.3)",
    backgroundColor: colors.accentBg,
  },
  actionText: {
    fontSize: 14,
    fontWeight: "600",
  },
});
