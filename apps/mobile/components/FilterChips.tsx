import React from "react";
import { View, Text, Pressable, ScrollView, StyleSheet } from "react-native";
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
  FadeIn,
} from "react-native-reanimated";
import { colors, spacing, radius } from "@/lib/theme";

interface Chip {
  key: string;
  label: string;
  count?: number;
}

interface Props {
  chips: Chip[];
  selected: string;
  onSelect: (key: string) => void;
}

function AnimatedChip({
  chip,
  active,
  onPress,
}: {
  chip: Chip;
  active: boolean;
  onPress: () => void;
}) {
  return (
    <Pressable
      style={[styles.chip, active && styles.chipActive]}
      onPress={onPress}
    >
      <Text style={[styles.chipText, active && styles.chipTextActive]}>
        {chip.label}
      </Text>
      {chip.count !== undefined && (
        <Animated.View
          key={`${chip.key}-${chip.count}`}
          entering={FadeIn.duration(200)}
          style={[styles.countBadge, active && styles.countBadgeActive]}
        >
          <Text style={[styles.countText, active && styles.countTextActive]}>
            {chip.count}
          </Text>
        </Animated.View>
      )}
    </Pressable>
  );
}

export function FilterChips({ chips, selected, onSelect }: Props) {
  return (
    <ScrollView
      horizontal
      showsHorizontalScrollIndicator={false}
      contentContainerStyle={styles.container}
    >
      {chips.map((chip) => (
        <AnimatedChip
          key={chip.key}
          chip={chip}
          active={chip.key === selected}
          onPress={() => onSelect(chip.key)}
        />
      ))}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: {
    paddingVertical: spacing.md,
    gap: spacing.sm,
  },
  chip: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    borderRadius: radius.full,
    backgroundColor: colors.bgElevated,
    borderWidth: 1,
    borderColor: colors.border,
    gap: 6,
  },
  chipActive: {
    backgroundColor: colors.accentBg,
    borderColor: colors.accent,
  },
  chipText: {
    fontSize: 13,
    fontWeight: "500",
    color: colors.textSecondary,
  },
  chipTextActive: {
    color: colors.accent,
  },
  countBadge: {
    minWidth: 18,
    height: 18,
    borderRadius: 9,
    backgroundColor: colors.bgHover,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: 5,
  },
  countBadgeActive: {
    backgroundColor: "rgba(245, 158, 11, 0.2)",
  },
  countText: {
    fontSize: 11,
    fontWeight: "600",
    color: colors.textTertiary,
  },
  countTextActive: {
    color: colors.accent,
  },
});
