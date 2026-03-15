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
} from "react-native";
import { useLocalSearchParams, Stack } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeIn, SlideInRight, SlideInLeft } from "react-native-reanimated";
import { useChatMessages, useSendChatMessage } from "@/lib/query";
import { createChatStream } from "@/lib/api";
import { LoadingScreen } from "@/components/LoadingScreen";
import { ErrorScreen } from "@/components/ErrorScreen";
import { colors, spacing, radius, common } from "@/lib/theme";
import type { ChatMessage } from "@/lib/api";

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
          {message.content}
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

  return (
    <Animated.View entering={FadeIn.duration(200)} style={styles.messageRow}>
      <View style={styles.botAvatar}>
        <Ionicons name="cube" size={14} color={colors.accent} />
      </View>
      <View style={[styles.bubble, styles.bubbleAssistant]}>
        {text ? (
          <Text style={styles.messageTextAssistant} selectable>
            {text}
          </Text>
        ) : (
          <View style={styles.typingRow}>
            <ActivityIndicator size="small" color={colors.accent} />
            <Text style={styles.typingText}>Thinking...</Text>
          </View>
        )}
      </View>
    </Animated.View>
  );
}

export default function ChatThreadScreen() {
  const { thread } = useLocalSearchParams<{ thread: string }>();
  const threadKey = decodeURIComponent(thread ?? "");
  const { data: messages, isLoading, error, refetch } = useChatMessages(threadKey);
  const sendMutation = useSendChatMessage();
  const [input, setInput] = useState("");
  const [showStreaming, setShowStreaming] = useState(false);
  const listRef = useRef<FlatList>(null);

  const handleSend = useCallback(() => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    setShowStreaming(true);
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
      setTimeout(() => {
        listRef.current?.scrollToEnd({ animated: true });
      }, 100);
    }
  }, [messages?.length]);

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
            listRef.current?.scrollToEnd({ animated: false });
          }}
        />

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
            />
            <Pressable
              style={[
                styles.sendButton,
                (!input.trim() || sendMutation.isPending) && styles.sendButtonDisabled,
              ]}
              onPress={handleSend}
              disabled={!input.trim() || sendMutation.isPending}
            >
              {sendMutation.isPending ? (
                <ActivityIndicator size="small" color={colors.textInverse} />
              ) : (
                <Ionicons name="send" size={18} color={colors.textInverse} />
              )}
            </Pressable>
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
  typingRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.sm,
  },
  typingText: {
    fontSize: 13,
    color: colors.textTertiary,
    fontStyle: "italic",
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
  },
  sendButtonDisabled: {
    opacity: 0.4,
  },
});
