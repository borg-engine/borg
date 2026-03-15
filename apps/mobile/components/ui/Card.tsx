import React, { useCallback } from 'react';
import { StyleSheet, ViewStyle } from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
} from 'react-native-reanimated';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import { colors, spacing, radius } from './theme';

interface CardProps {
  children: React.ReactNode;
  onPress?: () => void;
  style?: ViewStyle;
  disabled?: boolean;
}

export function Card({ children, onPress, style, disabled }: CardProps) {
  const scale = useSharedValue(1);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const gesture = Gesture.Tap()
    .enabled(!disabled && !!onPress)
    .onBegin(() => {
      scale.value = withTiming(0.975, { duration: 100 });
    })
    .onFinalize(() => {
      scale.value = withTiming(1, { duration: 150 });
    })
    .onEnd(() => {
      if (onPress) onPress();
    });

  if (!onPress) {
    return (
      <Animated.View style={[styles.card, style]}>
        {children}
      </Animated.View>
    );
  }

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View style={[styles.card, animatedStyle, disabled && styles.disabled, style]}>
        {children}
      </Animated.View>
    </GestureDetector>
  );
}

const styles = StyleSheet.create({
  card: {
    backgroundColor: colors.card,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.lg,
    shadowColor: '#000',
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.15,
    shadowRadius: 6,
    elevation: 3,
  },
  disabled: {
    opacity: 0.5,
  },
});
