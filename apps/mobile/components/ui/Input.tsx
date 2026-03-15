import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  TextInput,
  TextInputProps,
  StyleSheet,
} from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
  interpolateColor,
} from 'react-native-reanimated';
import { colors, spacing, radius, fontSize } from './theme';

interface InputProps extends Omit<TextInputProps, 'style'> {
  label?: string;
  error?: string;
  containerStyle?: any;
}

const AnimatedView = Animated.View;

export function Input({ label, error, containerStyle, ...props }: InputProps) {
  const focusAnim = useSharedValue(0);

  const handleFocus = useCallback(
    (e: any) => {
      focusAnim.value = withTiming(1, { duration: 200 });
      props.onFocus?.(e);
    },
    [props.onFocus],
  );

  const handleBlur = useCallback(
    (e: any) => {
      focusAnim.value = withTiming(0, { duration: 200 });
      props.onBlur?.(e);
    },
    [props.onBlur],
  );

  const borderStyle = useAnimatedStyle(() => {
    const borderColor = error
      ? colors.error
      : focusAnim.value === 1
        ? colors.accent
        : colors.border;
    return {
      borderColor: withTiming(borderColor, { duration: 200 }),
    };
  });

  return (
    <View style={containerStyle}>
      {label && <Text style={styles.label}>{label}</Text>}
      <AnimatedView style={[styles.inputWrapper, borderStyle, error && styles.inputError]}>
        <TextInput
          {...props}
          style={[styles.input, props.multiline && styles.multiline]}
          placeholderTextColor={colors.textTertiary}
          onFocus={handleFocus}
          onBlur={handleBlur}
          selectionColor={colors.accent}
        />
      </AnimatedView>
      {error && <Text style={styles.errorText}>{error}</Text>}
    </View>
  );
}

const styles = StyleSheet.create({
  label: {
    fontSize: fontSize.sm,
    fontWeight: '600',
    color: colors.textSecondary,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.5,
    marginBottom: spacing.sm,
  },
  inputWrapper: {
    backgroundColor: '#262320',
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
  },
  inputError: {
    borderColor: colors.error,
  },
  input: {
    paddingHorizontal: spacing.lg,
    paddingVertical: 14,
    color: colors.text,
    fontSize: 16,
  },
  multiline: {
    minHeight: 100,
    paddingTop: spacing.md,
    textAlignVertical: 'top' as const,
  },
  errorText: {
    fontSize: fontSize.xs,
    color: colors.error,
    marginTop: spacing.xs,
    marginLeft: spacing.xs,
  },
});
