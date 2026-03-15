import React, { useCallback } from "react";
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
  FadeInDown,
  useSharedValue,
  useAnimatedStyle,
  withTiming,
} from "react-native-reanimated";
import { Gesture, GestureDetector } from "react-native-gesture-handler";
import { useChatThreads } from "@/lib/query";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonList } from "@/components/ui/Skeleton";
import { ErrorScreen } from "@/components/ErrorScreen";
import { lightImpact } from "@/lib/haptics";
import { colors, spacing, radius, common } from "@/lib/theme";
import { timeAgo, truncate } from "@/lib/utils";
import type { ChatThread } from "@/lib/api";

function ThreadCard({ thread, index }: { thread: ChatThread; index: number }) {
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
      router.push(`/chat/${encodeURIComponent(thread.thread)}`);
    });

  return (
    <Animated.View entering={FadeInDown.duration(300).delay(Math.min(index * 40, 200))}>
      <GestureDetector gesture={gesture}>
        <Animated.View style={[styles.card, animStyle]}>
          <View style={styles.cardLeft}>
            <View style={styles.avatar}>
              <Ionicons name="chatbubble" size={18} color={colors.accent} />
            </View>
          </View>
          <View style={styles.cardContent}>
            <View style={common.rowBetween}>
              <Text style={styles.threadName} numberOfLines={1}>
                {thread.project_name || thread.thread}
              </Text>
              {thread.last_at && (
                <Text style={styles.time}>{timeAgo(thread.last_at)}</Text>
              )}
            </View>
            {thread.last_message && (
              <Text style={styles.preview} numberOfLines={2}>
                {truncate(thread.last_message, 120)}
              </Text>
            )}
            <View style={styles.countRow}>
              <Ionicons name="chatbubbles-outline" size={12} color={colors.textTertiary} />
              <Text style={styles.countText}>
                {thread.message_count} message{thread.message_count !== 1 ? "s" : ""}
              </Text>
            </View>
          </View>
          <Ionicons name="chevron-forward" size={16} color={colors.textTertiary} />
        </Animated.View>
      </GestureDetector>
    </Animated.View>
  );
}

export default function ChatScreen() {
  const { data: threads, isLoading, error, refetch, isRefetching } = useChatThreads();

  const renderItem = useCallback(
    ({ item, index }: { item: ChatThread; index: number }) => (
      <ThreadCard thread={item} index={index} />
    ),
    [],
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
        data={threads}
        keyExtractor={(item) => item.thread}
        renderItem={renderItem}
        contentContainerStyle={[
          styles.listContent,
          (!threads || threads.length === 0) && styles.listEmpty,
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
            icon="chatbubbles-outline"
            title="No conversations"
            subtitle="Chat threads from the web dashboard and connected services will appear here"
          />
        }
        showsVerticalScrollIndicator={false}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  listContent: {
    paddingHorizontal: spacing.lg,
    paddingTop: spacing.md,
    paddingBottom: 100,
    gap: spacing.sm,
  },
  listEmpty: {
    flexGrow: 1,
  },
  skeletonContainer: {
    padding: spacing.lg,
  },
  card: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.lg,
    gap: spacing.md,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 1 },
    shadowOpacity: 0.08,
    shadowRadius: 3,
    elevation: 1,
  },
  cardLeft: {},
  avatar: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: colors.accentBg,
    alignItems: "center",
    justifyContent: "center",
  },
  cardContent: {
    flex: 1,
    gap: 4,
  },
  threadName: {
    fontSize: 15,
    fontWeight: "600",
    color: colors.text,
    flex: 1,
  },
  time: {
    fontSize: 11,
    color: colors.textTertiary,
    marginLeft: spacing.sm,
  },
  preview: {
    fontSize: 13,
    color: colors.textSecondary,
    lineHeight: 18,
  },
  countRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    marginTop: 2,
  },
  countText: {
    fontSize: 11,
    color: colors.textTertiary,
  },
});
