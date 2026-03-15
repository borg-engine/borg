import React from "react";
import { View, ActivityIndicator, StyleSheet } from "react-native";
import { colors } from "@/lib/theme";

export function LoadingScreen() {
  return (
    <View style={styles.container}>
      <ActivityIndicator size="large" color={colors.accent} />
    </View>
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
