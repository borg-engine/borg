import React, { useState, useRef, useEffect, useCallback } from "react";
import {
  View,
  Text,
  FlatList,
  TextInput,
  Pressable,
  StyleSheet,
  KeyboardAvoidingView,
  Platform,
  ActivityIndicator,
  Linking,
} from "react-native";
import { useLocalSearchParams, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, {
  FadeIn,
  SlideInRight,
  SlideInLeft,
  SlideInDown,
  useSharedValue,
  useAnimatedStyle,
  withTiming,
  withRepeat,
  withSequence,
  withDelay,
} from "react-native-reanimated";
import { Gesture, GestureDetector } from "react-native-gesture-handler";
import { useChatMessages, useSendChatMessage } from "@/lib/query";
import { createChatStream } from "@/lib/api";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { lightImpact } from "@/lib/haptics";
import { colors, spacing, radius, common } from "@/lib/theme";
import type { ChatMessage } from "@/lib/api";

function renderMarkdown(text: string): React.ReactNode[] {
  const parts: React.ReactNode[] = [];
  let key = 0;

  const codeBlockRe = /```[\s\S]*?```/g;
  let lastIdx = 0;
  let match;

  while ((match = codeBlockRe.exec(text)) !== null) {
    if (match.index > lastIdx) {
      parts.push(...renderInline(text.slice(lastIdx, match.index), key));
      key += 100;
    }
    const code = match[0].replace(/^```\w*\n?/, "").replace(/\n?```$/, "");
    parts.push(
      <View key={`cb-${key++}`} style={mdStyles.codeBlock}>
        <Text style={mdStyles.codeText} selectable>
          {code}
        </Text>
      </View>,
    );
    lastIdx = match.index + match[0].length;
  }

  if (lastIdx < text.length) {
    parts.push(...renderInline(text.slice(lastIdx), key));
  }

  return parts;
}

function renderInline(text: string, startKey: number): React.ReactNode[] {
  const parts: React.ReactNode[] = [];
  let k = startKey;
  const re = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*]+\*)|(\[[^\]]+\]\([^)]+\))/g;
  let lastIdx = 0;
  let match;

  while ((match = re.exec(text)) !== null) {
    if (match.index > lastIdx) {
      parts.push(
        <Text key={`t-${k++}`} style={mdStyles.plainText}>
          {text.slice(lastIdx, match.index)}
        </Text>,
      );
    }

    const m = match[0];
    if (m.startsWith("`")) {
      parts.push(
        <Text key={`ic-${k++}`} style={mdStyles.inlineCode}>
          {m.slice(1, -1)}
        </Text>,
      );
    } else if (m.startsWith("**")) {
      parts.push(
        <Text key={`b-${k++}`} style={mdStyles.bold}>
          {m.slice(2, -2)}
        </Text>,
      );
    } else if (m.startsWith("*")) {
      parts.push(
        <Text key={`i-${k++}`} style={mdStyles.italic}>
          {m.slice(1, -1)}
        </Text>,
      );
    } else if (m.startsWith("[")) {
      const linkMatch = m.match(/\[([^\]]+)\]\(([^)]+)\)/);
      if (linkMatch) {
        parts.push(
          <Text
            key={`l-${k++}`}
            style={mdStyles.link}
            onPress={() => Linking.openURL(linkMatch[2])}
          >
            {linkMatch[1]}
          </Text>,
        );
      }
    }

    lastIdx = match.index + m.length;
  }

  if (lastIdx < text.length) {
    parts.push(
      <Text key={`t-${k++}`} style={mdStyles.plainText}>
        {text.slice(lastIdx)}
      </Text>,
    );
  }

  return parts;
}

const mdStyles = StyleSheet.create({
  plainText: {},
  bold: {
    fontWeight: "700",
  },
  italic: {
    fontStyle: "italic",
  },
  inlineCode: {
    fontFamily: Platform.OS === "ios" ? "Menlo" : "monospace",
    fontSize: 13,
    backgroundColor: "rgba(255,255,255,0.06)",
    borderRadius: 4,
    paddingHorizontal: 4,
  },
  link: {
    color: colors.info,
    textDecorationLine: "underline",
  },
  codeBlock: {
    backgroundColor: "rgba(255,255,255,0.04)",
    borderRadius: radius.sm,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.md,
    marginVertical: spacing.xs,
  },
  codeText: {
    fontFamily: Platform.OS === "ios" ? "Menlo" : "monospace",
    fontSize: 12,
    color: colors.textSecondary,
    lineHeight: 18,
  },
});

function TypingIndicator() {
  const dot1 = useSharedValue(0);
  const dot2 = useSharedValue(0);
  const dot3 = useSharedValue(0);

  useEffect(() => {
    dot1.value = withRepeat(
      withSequence(withTiming(-4, { duration: 300 }), withTiming(0, { duration: 300 })),
      -1,
    );
    dot2.value = withRepeat(
      withDelay(150,
        withSequence(withTiming(-4, { duration: 300 }), withTiming(0, { duration: 300 })),
      ),
      -1,
    );
    dot3.value = withRepeat(
      withDelay(300,
        withSequence(withTiming(-4, { duration: 300 }), withTiming(0, { duration: 300 })),
      ),
      -1,
    );
  }, []);

  const s1 = useAnimatedStyle(() => ({ transform: [{ translateY: dot1.value }] }));
  const s2 = useAnimatedStyle(() => ({ transform: [{ translateY: dot2.value }] }));
  const s3 = useAnimatedStyle(() => ({ transform: [{ translateY: dot3.value }] }));

  return (
    <Animated.View entering={FadeIn.duration(200)} style={styles.messageRow}>
      <View style={styles.botAvatar}>
        <Ionicons name="cube" size={14} color={colors.accent} />
      </View>
      <View style={[styles.bubble, styles.bubbleAssistant]}>
        <View style={styles.typingDots}>
          <Animated.View style={[styles.typingDot, s1]} />
          <Animated.View style={[styles.typingDot, s2]} />
          <Animated.View style={[styles.typingDot, s3]} />
        </View>
      </View>
    </Animated.View>
  );
}

function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";

  if (isSystem) {
    return (
      <View style={styles.systemMessage}>
        <Text style={styles.systemText}>{message.content}</Text>
      </View>
    );
  }

  return (
    <Animated.View
      entering={isUser ? SlideInRight.duration(200) : SlideInLeft.duration(200)}
      style={[styles.messageRow, isUser && styles.messageRowUser]}
    >
      {!isUser && (
        <View style={styles.botAvatar}>
          <Ionicons name="cube" size={14} color={colors.accent} />
        </View>
      )}
      <View
        style={[
          styles.bubble,
          isUser ? styles.bubbleUser : styles.bubbleAssistant,
        ]}
      >
        <Text
          style={[
            styles.messageText,
            isUser ? styles.messageTextUser : styles.messageTextAssistant,
          ]}
          selectable
        >
          {isUser
            ? message.content
            : renderMarkdown(message.content)}
        </Text>
        <Text style={[styles.messageTime, isUser && styles.messageTimeUser]}>
          {new Date(message.created_at).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </Text>
      </View>
    </Animated.View>
  );
}

function StreamingBubble({ thread }: { thread: string }) {
  const [text, setText] = useState("");
  const [streaming, setStreaming] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    setStreaming(true);

    createChatStream(
      thread,
      (data) => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.type === "assistant_delta" && parsed.delta) {
            setText((prev) => prev + parsed.delta);
          } else if (parsed.type === "assistant" && parsed.content) {
            setText(parsed.content);
          }
        } catch {}
      },
      controller.signal,
    )
      .catch(() => {})
      .finally(() => setStreaming(false));

    return () => controller.abort();
  }, [thread]);

  if (!streaming && !text) return null;

  if (!text) return <TypingIndicator />;

  return (
    <Animated.View entering={FadeIn.duration(200)} style={styles.messageRow}>
      <View style={styles.botAvatar}>
        <Ionicons name="cube" size={14} color={colors.accent} />
      </View>
      <View style={[styles.bubble, styles.bubbleAssistant]}>
        <Text style={styles.messageTextAssistant} selectable>
          {renderMarkdown(text)}
        </Text>
      </View>
    </Animated.View>
  );
}

function SendButton({ enabled, loading, onPress }: { enabled: boolean; loading: boolean; onPress: () => void }) {
  const scale = useSharedValue(1);

  const animStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const gesture = Gesture.Tap()
    .enabled(enabled && !loading)
    .onBegin(() => {
      scale.value = withTiming(0.85, { duration: 60 });
    })
    .onFinalize(() => {
      scale.value = withTiming(1, { duration: 100 });
    })
    .onEnd(() => {
      onPress();
    });

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View
        style={[
          styles.sendButton,
          (!enabled || loading) && styles.sendButtonDisabled,
          animStyle,
        ]}
      >
        {loading ? (
          <ActivityIndicator size="small" color={colors.textInverse} />
        ) : (
          <Ionicons name="send" size={18} color={colors.textInverse} />
        )}
      </Animated.View>
    </GestureDetector>
  );
}

export default function ChatThreadScreen() {
  const { thread } = useLocalSearchParams<{ thread: string }>();
  const threadKey = decodeURIComponent(thread ?? "");
  const { data: messages, isLoading, error, refetch } = useChatMessages(threadKey);
  const sendMutation = useSendChatMessage();
  const [input, setInput] = useState("");
  const [showStreaming, setShowStreaming] = useState(false);
  const [showNewMessages, setShowNewMessages] = useState(false);
  const listRef = useRef<FlatList>(null);
  const isAtBottom = useRef(true);

  const handleSend = useCallback(() => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    setShowStreaming(true);
    lightImpact();
    sendMutation.mutate(
      { thread: threadKey, content: text },
      {
        onSettled: () => {
          setShowStreaming(false);
        },
      },
    );
  }, [input, threadKey, sendMutation]);

  useEffect(() => {
    if (messages && messages.length > 0) {
      if (isAtBottom.current) {
        setTimeout(() => {
          listRef.current?.scrollToEnd({ animated: true });
        }, 100);
      } else {
        setShowNewMessages(true);
      }
    }
  }, [messages?.length]);

  const handleScroll = useCallback((e: any) => {
    const { contentOffset, contentSize, layoutMeasurement } = e.nativeEvent;
    const atBottom = contentOffset.y + layoutMeasurement.height >= contentSize.height - 60;
    isAtBottom.current = atBottom;
    if (atBottom) setShowNewMessages(false);
  }, []);

  const scrollToBottom = useCallback(() => {
    listRef.current?.scrollToEnd({ animated: true });
    setShowNewMessages(false);
  }, []);

  if (isLoading) return <LoadingScreen />;
  if (error) return <ErrorScreen message={error.message} onRetry={refetch} />;

  return (
    <>
      <Stack.Screen
        options={{
          headerTitle: threadKey.length > 24 ? threadKey.slice(0, 24) + "..." : threadKey,
        }}
      />
      <KeyboardAvoidingView
        style={common.screen}
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        keyboardVerticalOffset={90}
      >
        <View style={{ flex: 1 }}>
          <FlatList
            ref={listRef}
            data={messages ?? []}
            keyExtractor={(item) => String(item.id)}
            renderItem={({ item }) => <MessageBubble message={item} />}
            contentContainerStyle={styles.messageList}
            showsVerticalScrollIndicator={false}
            ListFooterComponent={
              showStreaming ? <StreamingBubble thread={threadKey} /> : null
            }
            onContentSizeChange={() => {
              if (isAtBottom.current) {
                listRef.current?.scrollToEnd({ animated: false });
              }
            }}
            onScroll={handleScroll}
            scrollEventThrottle={100}
          />

          {showNewMessages && (
            <Animated.View entering={SlideInDown.duration(200)}>
              <Pressable style={styles.newMessagesPill} onPress={scrollToBottom}>
                <Ionicons name="arrow-down" size={14} color={colors.accent} />
                <Text style={styles.newMessagesText}>New messages</Text>
              </Pressable>
            </Animated.View>
          )}
        </View>

        <View style={styles.inputContainer}>
          <View style={styles.inputRow}>
            <TextInput
              style={styles.input}
              placeholder="Type a message..."
              placeholderTextColor={colors.textTertiary}
              value={input}
              onChangeText={setInput}
              multiline
              maxLength={4000}
              returnKeyType="default"
              selectionColor={colors.accent}
            />
            <SendButton
              enabled={!!input.trim()}
              loading={sendMutation.isPending}
              onPress={handleSend}
            />
          </View>
        </View>
      </KeyboardAvoidingView>
    </>
  );
}

const styles = StyleSheet.create({
  messageList: {
    paddingHorizontal: spacing.lg,
    paddingTop: spacing.md,
    paddingBottom: spacing.md,
    gap: spacing.sm,
  },
  messageRow: {
    flexDirection: "row",
    alignItems: "flex-end",
    gap: spacing.sm,
    maxWidth: "85%",
  },
  messageRowUser: {
    alignSelf: "flex-end",
    flexDirection: "row-reverse",
  },
  botAvatar: {
    width: 28,
    height: 28,
    borderRadius: 14,
    backgroundColor: colors.accentBg,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: 2,
  },
  bubble: {
    borderRadius: radius.lg,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.md,
    maxWidth: "100%",
  },
  bubbleUser: {
    backgroundColor: colors.accent,
    borderBottomRightRadius: 4,
  },
  bubbleAssistant: {
    backgroundColor: colors.bgElevated,
    borderWidth: 1,
    borderColor: colors.border,
    borderBottomLeftRadius: 4,
  },
  messageText: {
    fontSize: 15,
    lineHeight: 21,
  },
  messageTextUser: {
    color: colors.textInverse,
  },
  messageTextAssistant: {
    color: colors.text,
    fontSize: 15,
    lineHeight: 21,
  },
  messageTime: {
    fontSize: 10,
    color: colors.textTertiary,
    marginTop: 4,
    alignSelf: "flex-end",
  },
  messageTimeUser: {
    color: "rgba(15, 14, 12, 0.5)",
  },
  systemMessage: {
    alignSelf: "center",
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
    backgroundColor: colors.bgHover,
    borderRadius: radius.full,
    marginVertical: spacing.xs,
  },
  systemText: {
    fontSize: 12,
    color: colors.textTertiary,
    fontStyle: "italic",
  },
  typingDots: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingVertical: 4,
    paddingHorizontal: 4,
  },
  typingDot: {
    width: 7,
    height: 7,
    borderRadius: 3.5,
    backgroundColor: colors.textTertiary,
  },
  newMessagesPill: {
    position: "absolute",
    bottom: 8,
    alignSelf: "center",
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: colors.bgElevated,
    borderRadius: radius.full,
    borderWidth: 1,
    borderColor: colors.accent,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.2,
    shadowRadius: 4,
    elevation: 3,
  },
  newMessagesText: {
    fontSize: 12,
    fontWeight: "600",
    color: colors.accent,
  },
  inputContainer: {
    borderTopWidth: 1,
    borderTopColor: colors.border,
    backgroundColor: colors.bgElevated,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    paddingBottom: Platform.OS === "ios" ? spacing.xxl : spacing.sm,
  },
  inputRow: {
    flexDirection: "row",
    alignItems: "flex-end",
    gap: spacing.sm,
  },
  input: {
    flex: 1,
    backgroundColor: colors.bgInput,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    paddingHorizontal: spacing.lg,
    paddingVertical: 10,
    color: colors.text,
    fontSize: 15,
    maxHeight: 120,
    lineHeight: 20,
  },
  sendButton: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: colors.accent,
    alignItems: "center",
    justifyContent: "center",
    shadowColor: colors.accent,
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.3,
    shadowRadius: 4,
    elevation: 2,
  },
  sendButtonDisabled: {
    opacity: 0.4,
  },
});
