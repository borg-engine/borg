import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { colors, radius, spacing } from "@/lib/theme";

interface Props {
  mode: string;
}

const MODE_COLORS: Record<string, { bg: string; text: string }> = {
  swe: { bg: "rgba(59, 130, 246, 0.12)", text: "#3b82f6" },
  sweborg: { bg: "rgba(59, 130, 246, 0.12)", text: "#3b82f6" },
  legal: { bg: "rgba(168, 85, 247, 0.12)", text: "#a855f7" },
  lawborg: { bg: "rgba(168, 85, 247, 0.12)", text: "#a855f7" },
  webborg: { bg: "rgba(34, 197, 94, 0.12)", text: "#22c55e" },
  web: { bg: "rgba(34, 197, 94, 0.12)", text: "#22c55e" },
  data: { bg: "rgba(245, 158, 11, 0.12)", text: "#f59e0b" },
  crew: { bg: "rgba(236, 72, 153, 0.12)", text: "#ec4899" },
  sales: { bg: "rgba(14, 165, 233, 0.12)", text: "#0ea5e9" },
  chef: { bg: "rgba(249, 115, 22, 0.12)", text: "#f97316" },
};

export function ModeBadge({ mode }: Props) {
  const c = MODE_COLORS[mode] ?? { bg: "rgba(120, 113, 108, 0.12)", text: "#78716c" };

  return (
    <View style={[styles.badge, { backgroundColor: c.bg }]}>
      <Text style={[styles.text, { color: c.text }]}>{mode}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  badge: {
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
    borderRadius: radius.sm,
  },
  text: {
    fontSize: 11,
    fontWeight: "600",
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
});
