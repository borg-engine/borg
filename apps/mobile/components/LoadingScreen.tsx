import React from "react";
import { View, ActivityIndicator, StyleSheet } from "react-native";
import Animated, { FadeIn } from "react-native-reanimated";
import { colors } from "@/lib/theme";

export function LoadingScreen() {
  return (
    <Animated.View entering={FadeIn.duration(300)} style={styles.container}>
      <ActivityIndicator size="large" color={colors.accent} />
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.bg,
    alignItems: "center",
    justifyContent: "center",
  },
});
