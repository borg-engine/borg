import React, { useEffect } from "react";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { QueryClientProvider } from "@tanstack/react-query";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { queryClient } from "@/lib/query";
import { AuthProvider, useAuth } from "@/lib/auth-context";
import { colors } from "@/lib/theme";
import {
  setupNotificationHandler,
  registerForPushNotifications,
} from "@/lib/notifications";

setupNotificationHandler();

const headerOptions = {
  headerShown: true,
  headerStyle: { backgroundColor: colors.bg },
  headerTintColor: colors.text,
  headerShadowVisible: false,
  headerTitleStyle: { fontWeight: "600" as const, fontSize: 17 },
} as const;

function PushRegistration({ children }: { children: React.ReactNode }) {
  const { authenticated } = useAuth();

  useEffect(() => {
    if (authenticated) {
      registerForPushNotifications();
    }
  }, [authenticated]);

  return <>{children}</>;
}

export default function RootLayout() {
  return (
    <GestureHandlerRootView style={{ flex: 1, backgroundColor: colors.bg }}>
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <PushRegistration>
            <StatusBar style="light" />
            <Stack
              screenOptions={{
                headerShown: false,
                contentStyle: { backgroundColor: colors.bg },
                animation: "slide_from_right",
              }}
            >
              <Stack.Screen name="index" options={{ animation: "none" }} />
              <Stack.Screen name="(auth)" options={{ animation: "fade" }} />
              <Stack.Screen name="(tabs)" options={{ animation: "fade" }} />
              <Stack.Screen
                name="task/[id]"
                options={{ ...headerOptions, headerTitle: "Task" }}
              />
              <Stack.Screen
                name="task/create"
                options={{
                  ...headerOptions,
                  headerTitle: "New Task",
                  presentation: "modal",
                  animation: "slide_from_bottom",
                }}
              />
              <Stack.Screen
                name="project/[id]"
                options={{ ...headerOptions, headerTitle: "Project" }}
              />
              <Stack.Screen
                name="chat/[thread]"
                options={{ ...headerOptions, headerTitle: "Chat" }}
              />
            </Stack>
          </PushRegistration>
        </AuthProvider>
      </QueryClientProvider>
    </GestureHandlerRootView>
  );
}
