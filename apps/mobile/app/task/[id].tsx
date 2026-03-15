import React, { useState, useEffect, useRef, useCallback } from "react";
import {
  View,
  Text,
  ScrollView,
  Pressable,
  StyleSheet,
  ActivityIndicator,
} from "react-native";
import { useLocalSearchParams, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, {
  FadeInDown,
  FadeIn,
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withTiming,
  withSequence,
} from "react-native-reanimated";
import { useTask, useRetryTask, useCancelTask, useApproveTask, useRejectTask } from "@/lib/query";
import { createTaskStream } from "@/lib/api";
import { StatusBadge } from "@/components/StatusBadge";
import { ModeBadge } from "@/components/ModeBadge";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { BottomSheet } from "@/components/ui/BottomSheet";
import { Button } from "@/components/ui/Button";
import { useToast } from "@/components/ui/Toast";
import { lightImpact, mediumImpact, successNotification } from "@/lib/haptics";
import { colors, spacing, radius, common, statusColor } from "@/lib/theme";
import { timeAgo, formatDuration } from "@/lib/utils";
import { isActiveStatus, getDisplayPhases, getPhaseLabel } from "@borg/api";

function PulsingDot({ color }: { color: string }) {
  const scale = useSharedValue(1);
  const opacity = useSharedValue(1);

  useEffect(() => {
    scale.value = withRepeat(
      withSequence(
        withTiming(1.4, { duration: 800 }),
        withTiming(1, { duration: 800 }),
      ),
      -1,
    );
    opacity.value = withRepeat(
      withSequence(
        withTiming(0.4, { duration: 800 }),
        withTiming(1, { duration: 800 }),
      ),
      -1,
    );
  }, []);

  const style = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
    opacity: opacity.value,
  }));

  return (
    <View style={phaseStyles.dotOuter}>
      <Animated.View
        style={[
          {
            position: "absolute",
            width: 22,
            height: 22,
            borderRadius: 11,
            backgroundColor: color + "40",
          },
          style,
        ]}
      />
      <View
        style={[
          phaseStyles.dot,
          { backgroundColor: color },
        ]}
      />
    </View>
  );
}

function PhaseProgress({ status, mode }: { status: string; mode?: string }) {
  const phases = getDisplayPhases(mode);
  const currentIndex = phases.indexOf(status);

  return (
    <View style={phaseStyles.container}>
      {phases.map((phase, i) => {
        const isComplete = i < currentIndex || status === "done" || status === "merged";
        const isCurrent = phase === status;
        const isFailed = status === "failed" && i === Math.max(0, currentIndex);

        let dotColor: string = colors.bgHover;
        if (isComplete) dotColor = colors.success;
        if (isCurrent) dotColor = colors.accent;
        if (isFailed) dotColor = colors.error;

        return (
          <View key={phase} style={phaseStyles.step}>
            {isCurrent && !isComplete ? (
              <PulsingDot color={dotColor} />
            ) : (
              <View style={phaseStyles.dotOuter}>
                <View
                  style={[
                    phaseStyles.dot,
                    { backgroundColor: dotColor },
                  ]}
                >
                  {isComplete && (
                    <Ionicons name="checkmark" size={10} color={colors.bg} />
                  )}
                </View>
              </View>
            )}
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
  dotOuter: {
    width: 22,
    height: 22,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: 4,
  },
  dot: {
    width: 20,
    height: 20,
    borderRadius: 10,
    alignItems: "center",
    justifyContent: "center",
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
  const [autoScroll, setAutoScroll] = useState(true);
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
    if (autoScroll) {
      scrollRef.current?.scrollToEnd({ animated: true });
    }
  }, [lines, autoScroll]);

  const handleScroll = useCallback((e: any) => {
    const { contentOffset, contentSize, layoutMeasurement } = e.nativeEvent;
    const isAtBottom = contentOffset.y + layoutMeasurement.height >= contentSize.height - 40;
    setAutoScroll(isAtBottom);
  }, []);

  const jumpToBottom = useCallback(() => {
    scrollRef.current?.scrollToEnd({ animated: true });
    setAutoScroll(true);
  }, []);

  if (lines.length === 0) {
    return (
      <View style={streamStyles.empty}>
        <ActivityIndicator size="small" color={colors.textTertiary} />
        <Text style={streamStyles.emptyText}>Waiting for output...</Text>
      </View>
    );
  }

  return (
    <View>
      <ScrollView
        ref={scrollRef}
        style={streamStyles.container}
        contentContainerStyle={streamStyles.content}
        showsVerticalScrollIndicator={true}
        onScroll={handleScroll}
        scrollEventThrottle={100}
      >
        {lines.map((line, i) => (
          <Text key={i} style={streamStyles.line}>
            {line}
          </Text>
        ))}
      </ScrollView>
      {!autoScroll && (
        <Pressable style={streamStyles.jumpButton} onPress={jumpToBottom}>
          <Ionicons name="arrow-down" size={14} color={colors.accent} />
          <Text style={streamStyles.jumpText}>Jump to bottom</Text>
        </Pressable>
      )}
    </View>
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
  jumpButton: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: 4,
    paddingVertical: spacing.sm,
    marginTop: spacing.xs,
    backgroundColor: colors.accentBg,
    borderRadius: radius.sm,
  },
  jumpText: {
    fontSize: 12,
    fontWeight: "500",
    color: colors.accent,
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
  const [showCancelSheet, setShowCancelSheet] = useState(false);
  const [showRejectSheet, setShowRejectSheet] = useState(false);
  const toast = useToast();

  const handleRetry = useCallback(() => {
    lightImpact();
    retryMutation.mutate(taskId, {
      onSuccess: () => {
        successNotification();
        toast.show("Task queued for retry", "success");
      },
      onError: (err) => {
        toast.show(err.message || "Failed to retry", "error");
      },
    });
  }, [taskId, retryMutation, toast]);

  const handleCancel = useCallback(() => {
    mediumImpact();
    cancelMutation.mutate(taskId, {
      onSuccess: () => {
        setShowCancelSheet(false);
        toast.show("Task cancelled", "info");
      },
      onError: (err) => {
        toast.show(err.message || "Failed to cancel", "error");
      },
    });
  }, [taskId, cancelMutation, toast]);

  const handleApprove = useCallback(() => {
    lightImpact();
    approveMutation.mutate(taskId, {
      onSuccess: () => {
        successNotification();
        toast.show("Task approved", "success");
      },
      onError: (err) => {
        toast.show(err.message || "Failed to approve", "error");
      },
    });
  }, [taskId, approveMutation, toast]);

  const handleReject = useCallback(() => {
    mediumImpact();
    rejectMutation.mutate({ id: taskId }, {
      onSuccess: () => {
        setShowRejectSheet(false);
        toast.show("Task rejected", "info");
      },
      onError: (err) => {
        toast.show(err.message || "Failed to reject", "error");
      },
    });
  }, [taskId, rejectMutation, toast]);

  if (isLoading) return <LoadingScreen />;
  if (error || !task) return <ErrorScreen message={error?.message} onRetry={refetch} />;

  const active = isActiveStatus(task.status);
  const inReview = task.status === "review" || task.status === "human_review";
  const showActions = active || inReview || task.status === "failed" || task.status === "done";

  return (
    <>
      <Stack.Screen
        options={{
          headerTitle: `Task #${task.id}`,
        }}
      />
      <View style={styles.wrapper}>
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
          </Animated.View>
        </ScrollView>

        {showActions && (
          <Animated.View entering={FadeIn.duration(300).delay(300)}>
            <View style={styles.bottomBar}>
              {inReview && (
                <>
                  <Button
                    title="Approve"
                    variant="secondary"
                    icon={<Ionicons name="checkmark-circle" size={18} color={colors.success} />}
                    onPress={handleApprove}
                    loading={approveMutation.isPending}
                    style={styles.actionFlex}
                  />
                  <Button
                    title="Reject"
                    variant="destructive"
                    icon={<Ionicons name="close-circle" size={18} color={colors.error} />}
                    onPress={() => { mediumImpact(); setShowRejectSheet(true); }}
                    style={styles.actionFlex}
                  />
                </>
              )}
              {active && !inReview && (
                <Button
                  title="Cancel Task"
                  variant="destructive"
                  icon={<Ionicons name="stop-circle" size={18} color={colors.error} />}
                  onPress={() => { mediumImpact(); setShowCancelSheet(true); }}
                  style={styles.actionFull}
                />
              )}
              {(task.status === "failed" || task.status === "done") && (
                <Button
                  title="Retry"
                  variant="secondary"
                  icon={<Ionicons name="refresh" size={18} color={colors.accent} />}
                  onPress={handleRetry}
                  loading={retryMutation.isPending}
                  style={styles.actionFull}
                />
              )}
            </View>
          </Animated.View>
        )}
      </View>

      <BottomSheet
        visible={showCancelSheet}
        onClose={() => setShowCancelSheet(false)}
        title="Cancel Task"
      >
        <Text style={styles.sheetMessage}>
          Are you sure you want to cancel this task? This action cannot be undone.
        </Text>
        <View style={styles.sheetActions}>
          <Button
            title="Keep Running"
            variant="secondary"
            onPress={() => setShowCancelSheet(false)}
            style={styles.actionFlex}
          />
          <Button
            title="Cancel Task"
            variant="destructive"
            onPress={handleCancel}
            loading={cancelMutation.isPending}
            style={styles.actionFlex}
          />
        </View>
      </BottomSheet>

      <BottomSheet
        visible={showRejectSheet}
        onClose={() => setShowRejectSheet(false)}
        title="Reject Task"
      >
        <Text style={styles.sheetMessage}>
          Send this task back for revision? The agent will attempt to fix the issues.
        </Text>
        <View style={styles.sheetActions}>
          <Button
            title="Keep in Review"
            variant="secondary"
            onPress={() => setShowRejectSheet(false)}
            style={styles.actionFlex}
          />
          <Button
            title="Reject"
            variant="destructive"
            onPress={handleReject}
            loading={rejectMutation.isPending}
            style={styles.actionFlex}
          />
        </View>
      </BottomSheet>
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
  wrapper: {
    flex: 1,
    backgroundColor: colors.bg,
  },
  content: {
    padding: spacing.lg,
    paddingBottom: 120,
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
  bottomBar: {
    flexDirection: "row",
    gap: spacing.md,
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
    paddingBottom: 32,
    backgroundColor: colors.bgElevated,
    borderTopWidth: 1,
    borderTopColor: colors.border,
  },
  actionFlex: {
    flex: 1,
  },
  actionFull: {
    flex: 1,
  },
  sheetMessage: {
    fontSize: 15,
    color: colors.textSecondary,
    lineHeight: 22,
    marginBottom: spacing.xl,
  },
  sheetActions: {
    flexDirection: "row",
    gap: spacing.md,
    marginBottom: spacing.sm,
  },
});
