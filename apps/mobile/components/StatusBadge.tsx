import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { colors, radius, spacing } from "@/lib/theme";
import { statusColor, statusBgColor } from "@/lib/theme";

interface Props {
  status: string;
  size?: "sm" | "md";
}

export function StatusBadge({ status, size = "sm" }: Props) {
  const color = statusColor(status);
  const bg = statusBgColor(status);

  return (
    <View style={[styles.badge, { backgroundColor: bg }, size === "md" && styles.badgeMd]}>
      <View style={[styles.dot, { backgroundColor: color }]} />
      <Text style={[styles.text, { color }, size === "md" && styles.textMd]}>
        {status.replace(/_/g, " ")}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  badge: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
    borderRadius: radius.sm,
    gap: 5,
  },
  badgeMd: {
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  text: {
    fontSize: 11,
    fontWeight: "600",
    textTransform: "capitalize",
    letterSpacing: 0.3,
  },
  textMd: {
    fontSize: 13,
  },
});
