import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeIn, FadeInDown } from "react-native-reanimated";
import { Button } from "@/components/ui/Button";
import { colors, spacing, radius } from "@/lib/theme";

interface Props {
  message?: string;
  onRetry?: () => void;
}

export function ErrorScreen({ message, onRetry }: Props) {
  return (
    <View style={styles.container}>
      <Animated.View entering={FadeIn.duration(300)} style={styles.iconWrap}>
        <Ionicons name="alert-circle-outline" size={44} color={colors.error} />
      </Animated.View>
      <Animated.View entering={FadeInDown.duration(300).delay(100)}>
        <Text style={styles.title}>Something went wrong</Text>
      </Animated.View>
      {message && (
        <Animated.View entering={FadeInDown.duration(300).delay(150)}>
          <Text style={styles.message}>{message}</Text>
        </Animated.View>
      )}
      {onRetry && (
        <Animated.View entering={FadeInDown.duration(300).delay(200)}>
          <Button title="Try Again" onPress={onRetry} />
        </Animated.View>
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
    paddingHorizontal: 32,
  },
  iconWrap: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: "rgba(239, 68, 68, 0.12)",
    alignItems: "center",
    justifyContent: "center",
    marginBottom: 16,
  },
  title: {
    fontSize: 17,
    fontWeight: "600",
    color: colors.text,
    marginBottom: 8,
    textAlign: "center",
  },
  message: {
    fontSize: 14,
    color: colors.textSecondary,
    textAlign: "center",
    marginBottom: 20,
    lineHeight: 20,
  },
});
