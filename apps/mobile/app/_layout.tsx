import React from "react";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { QueryClientProvider } from "@tanstack/react-query";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { queryClient } from "@/lib/query";
import { AuthProvider } from "@/lib/auth-context";
import { colors } from "@/lib/theme";

export default function RootLayout() {
  return (
    <GestureHandlerRootView style={{ flex: 1, backgroundColor: colors.bg }}>
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <StatusBar style="light" />
          <Stack
            screenOptions={{
              headerShown: false,
              contentStyle: { backgroundColor: colors.bg },
              animation: "slide_from_right",
            }}
          >
            <Stack.Screen name="(auth)" options={{ animation: "fade" }} />
            <Stack.Screen name="(tabs)" options={{ animation: "fade" }} />
            <Stack.Screen
              name="task/[id]"
              options={{
                headerShown: true,
                headerTitle: "Task",
                headerStyle: { backgroundColor: colors.bg },
                headerTintColor: colors.text,
                headerShadowVisible: false,
              }}
            />
            <Stack.Screen
              name="project/[id]"
              options={{
                headerShown: true,
                headerTitle: "Project",
                headerStyle: { backgroundColor: colors.bg },
                headerTintColor: colors.text,
                headerShadowVisible: false,
              }}
            />
            <Stack.Screen
              name="chat/[thread]"
              options={{
                headerShown: true,
                headerTitle: "Chat",
                headerStyle: { backgroundColor: colors.bg },
                headerTintColor: colors.text,
                headerShadowVisible: false,
              }}
            />
          </Stack>
        </AuthProvider>
      </QueryClientProvider>
    </GestureHandlerRootView>
  );
}
