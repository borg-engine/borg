import React from 'react';
import { View, Text, StyleSheet } from 'react-native';
import { colors, spacing, fontSize } from './theme';

type BadgeVariant = 'success' | 'error' | 'warning' | 'info' | 'default';

interface BadgeProps {
  label: string;
  variant?: BadgeVariant;
  dot?: boolean;
}

const VARIANT_COLORS: Record<BadgeVariant, { bg: string; fg: string }> = {
  success: { bg: 'rgba(34, 197, 94, 0.12)', fg: colors.success },
  error: { bg: 'rgba(239, 68, 68, 0.12)', fg: colors.error },
  warning: { bg: 'rgba(245, 158, 11, 0.12)', fg: colors.warning },
  info: { bg: 'rgba(59, 130, 246, 0.12)', fg: colors.info },
  default: { bg: 'rgba(120, 113, 108, 0.12)', fg: colors.textTertiary },
};

export function Badge({ label, variant = 'default', dot = false }: BadgeProps) {
  const c = VARIANT_COLORS[variant];

  return (
    <View style={[styles.badge, { backgroundColor: c.bg }]}>
      {dot && <View style={[styles.dot, { backgroundColor: c.fg }]} />}
      <Text style={[styles.text, { color: c.fg }]}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  badge: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 100,
    gap: 5,
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  text: {
    fontSize: fontSize.xs,
    fontWeight: '600',
    textTransform: 'capitalize',
    letterSpacing: 0.3,
  },
});
