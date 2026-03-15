import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { Ionicons } from "@expo/vector-icons";
import { colors, spacing, radius } from "@/lib/theme";

interface Props {
  icon: keyof typeof Ionicons.glyphMap;
  title: string;
  subtitle?: string;
}

export function EmptyState({ icon, title, subtitle }: Props) {
  return (
    <View style={styles.container}>
      <View style={styles.iconContainer}>
        <Ionicons name={icon} size={40} color={colors.textTertiary} />
      </View>
      <Text style={styles.title}>{title}</Text>
      {subtitle && <Text style={styles.subtitle}>{subtitle}</Text>}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: spacing.xxxl,
    paddingVertical: 60,
  },
  iconContainer: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: colors.bgElevated,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: spacing.lg,
  },
  title: {
    fontSize: 17,
    fontWeight: "600",
    color: colors.textSecondary,
    textAlign: "center",
    marginBottom: spacing.sm,
  },
  subtitle: {
    fontSize: 14,
    color: colors.textTertiary,
    textAlign: "center",
    lineHeight: 20,
  },
});
