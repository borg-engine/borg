import React from "react";
import { View, Text, Pressable, StyleSheet } from "react-native";
import { Ionicons } from "@expo/vector-icons";
import { colors, spacing, radius, common } from "@/lib/theme";

interface Props {
  message?: string;
  onRetry?: () => void;
}

export function ErrorScreen({ message, onRetry }: Props) {
  return (
    <View style={styles.container}>
      <View style={styles.iconWrap}>
        <Ionicons name="alert-circle-outline" size={44} color={colors.error} />
      </View>
      <Text style={styles.title}>Something went wrong</Text>
      {message && <Text style={styles.message}>{message}</Text>}
      {onRetry && (
        <Pressable style={common.buttonPrimary} onPress={onRetry}>
          <Text style={common.buttonPrimaryText}>Try Again</Text>
        </Pressable>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.bg,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: spacing.xxxl,
  },
  iconWrap: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: colors.errorBg,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: spacing.lg,
  },
  title: {
    fontSize: 17,
    fontWeight: "600",
    color: colors.text,
    marginBottom: spacing.sm,
  },
  message: {
    fontSize: 14,
    color: colors.textSecondary,
    textAlign: "center",
    marginBottom: spacing.xl,
    lineHeight: 20,
  },
});
