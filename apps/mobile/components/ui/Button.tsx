import React from 'react';
import { Text, StyleSheet, ActivityIndicator, ViewStyle, TextStyle } from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
} from 'react-native-reanimated';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import { colors, spacing, radius, fontSize } from './theme';

type ButtonVariant = 'primary' | 'secondary' | 'destructive' | 'ghost';

interface ButtonProps {
  title: string;
  onPress: () => void;
  variant?: ButtonVariant;
  disabled?: boolean;
  loading?: boolean;
  icon?: React.ReactNode;
  style?: ViewStyle;
}

export function Button({
  title,
  onPress,
  variant = 'primary',
  disabled = false,
  loading = false,
  icon,
  style,
}: ButtonProps) {
  const scale = useSharedValue(1);
  const isDisabled = disabled || loading;

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const gesture = Gesture.Tap()
    .enabled(!isDisabled)
    .onBegin(() => {
      scale.value = withTiming(0.965, { duration: 80 });
    })
    .onFinalize(() => {
      scale.value = withTiming(1, { duration: 120 });
    })
    .onEnd(() => {
      onPress();
    });

  const variantStyles = getVariantStyles(variant);

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View
        style={[
          styles.base,
          variantStyles.container,
          animatedStyle,
          isDisabled && styles.disabled,
          style,
        ]}
      >
        {loading ? (
          <ActivityIndicator
            size="small"
            color={variant === 'primary' ? colors.bg : colors.accent}
          />
        ) : (
          <>
            {icon}
            <Text style={[styles.text, variantStyles.text]}>{title}</Text>
          </>
        )}
      </Animated.View>
    </GestureDetector>
  );
}

function getVariantStyles(variant: ButtonVariant): {
  container: ViewStyle;
  text: TextStyle;
} {
  switch (variant) {
    case 'primary':
      return {
        container: {
          backgroundColor: colors.accent,
        },
        text: {
          color: colors.bg,
        },
      };
    case 'secondary':
      return {
        container: {
          backgroundColor: colors.card,
          borderWidth: 1,
          borderColor: colors.border,
        },
        text: {
          color: colors.text,
        },
      };
    case 'destructive':
      return {
        container: {
          backgroundColor: 'rgba(239, 68, 68, 0.12)',
          borderWidth: 1,
          borderColor: 'rgba(239, 68, 68, 0.25)',
        },
        text: {
          color: colors.error,
        },
      };
    case 'ghost':
      return {
        container: {
          backgroundColor: 'transparent',
        },
        text: {
          color: colors.textSecondary,
        },
      };
  }
}

const styles = StyleSheet.create({
  base: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    paddingVertical: 14,
    paddingHorizontal: spacing.xl,
    borderRadius: radius.md,
    gap: spacing.sm,
    minHeight: 48,
  },
  text: {
    fontSize: fontSize.md,
    fontWeight: '600',
  },
  disabled: {
    opacity: 0.5,
  },
});
