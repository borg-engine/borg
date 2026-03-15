import React, { useState, useCallback } from "react";
import {
  View,
  Text,
  TextInput,
  Pressable,
  StyleSheet,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  ActivityIndicator,
} from "react-native";
import { router } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, {
  FadeIn,
  FadeInDown,
  useSharedValue,
  useAnimatedStyle,
  withSequence,
  withTiming,
  withSpring,
} from "react-native-reanimated";
import { useAuth } from "@/lib/auth-context";
import { checkConnection } from "@/lib/auth";
import { lightImpact, errorNotification, successNotification } from "@/lib/haptics";
import { colors, spacing, radius, common } from "@/lib/theme";

type ConnectionState = "idle" | "checking" | "connected" | "failed";

export default function LoginScreen() {
  const { login } = useAuth();
  const [serverUrl, setServerUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [connState, setConnState] = useState<ConnectionState>("idle");

  const shakeX = useSharedValue(0);
  const formOpacity = useSharedValue(1);

  const shakeStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: shakeX.value }],
  }));

  const triggerShake = useCallback(() => {
    shakeX.value = withSequence(
      withTiming(-12, { duration: 50 }),
      withTiming(12, { duration: 50 }),
      withTiming(-8, { duration: 50 }),
      withTiming(8, { duration: 50 }),
      withTiming(-4, { duration: 50 }),
      withTiming(0, { duration: 50 }),
    );
  }, []);

  const handleCheckServer = useCallback(async () => {
    if (!serverUrl.trim()) {
      setError("Enter a server URL");
      triggerShake();
      errorNotification();
      return;
    }
    setConnState("checking");
    setError("");
    lightImpact();
    const ok = await checkConnection(serverUrl.trim());
    setConnState(ok ? "connected" : "failed");
    if (!ok) {
      setError("Could not connect to server");
      triggerShake();
      errorNotification();
    } else {
      successNotification();
    }
  }, [serverUrl, triggerShake]);

  const handleLogin = useCallback(async () => {
    if (!serverUrl.trim() || !username.trim() || !password) {
      setError("All fields are required");
      triggerShake();
      errorNotification();
      return;
    }
    setLoading(true);
    setError("");
    lightImpact();
    const result = await login(serverUrl.trim(), username.trim(), password);
    setLoading(false);
    if (result.success) {
      successNotification();
      router.replace("/(tabs)");
    } else {
      setError(result.error || "Login failed");
      triggerShake();
      errorNotification();
    }
  }, [serverUrl, username, password, login, triggerShake]);

  const connIcon = connState === "connected" ? "checkmark-circle" : connState === "failed" ? "close-circle" : null;
  const connColor = connState === "connected" ? colors.success : colors.error;

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === "ios" ? "padding" : "height"}
    >
      <ScrollView
        contentContainerStyle={styles.scrollContent}
        keyboardShouldPersistTaps="handled"
        showsVerticalScrollIndicator={false}
      >
        <Animated.View entering={FadeIn.duration(600)} style={styles.header}>
          <View style={styles.logoContainer}>
            <Ionicons name="cube" size={48} color={colors.accent} />
          </View>
          <Text style={styles.title}>Borg</Text>
          <Text style={styles.subtitle}>Autonomous Agent Orchestrator</Text>
        </Animated.View>

        <Animated.View entering={FadeInDown.duration(500).delay(200)} style={shakeStyle}>
          <View style={styles.card}>
            <View style={styles.field}>
              <Text style={styles.label}>Server URL</Text>
              <View style={styles.inputRow}>
                <TextInput
                  style={[styles.input, styles.inputFlex]}
                  placeholder="https://your-server.example.com"
                  placeholderTextColor={colors.textTertiary}
                  value={serverUrl}
                  onChangeText={(t) => {
                    setServerUrl(t);
                    setConnState("idle");
                  }}
                  autoCapitalize="none"
                  autoCorrect={false}
                  keyboardType="url"
                  returnKeyType="next"
                  selectionColor={colors.accent}
                />
                {connIcon && (
                  <View style={styles.statusIcon}>
                    <Ionicons name={connIcon} size={20} color={connColor} />
                  </View>
                )}
              </View>
              {connState === "idle" && (
                <Pressable
                  style={styles.checkButton}
                  onPress={handleCheckServer}
                >
                  <Text style={styles.checkButtonText}>Check Connection</Text>
                </Pressable>
              )}
              {connState === "checking" && (
                <View style={styles.connectingRow}>
                  <ActivityIndicator size="small" color={colors.accent} />
                  <Text style={styles.connectingText}>Connecting...</Text>
                </View>
              )}
            </View>

            <View style={styles.field}>
              <Text style={styles.label}>Username</Text>
              <TextInput
                style={styles.input}
                placeholder="admin"
                placeholderTextColor={colors.textTertiary}
                value={username}
                onChangeText={setUsername}
                autoCapitalize="none"
                autoCorrect={false}
                returnKeyType="next"
                selectionColor={colors.accent}
              />
            </View>

            <View style={styles.field}>
              <Text style={styles.label}>Password</Text>
              <View style={styles.inputRow}>
                <TextInput
                  style={[styles.input, styles.inputFlex]}
                  placeholder="Password"
                  placeholderTextColor={colors.textTertiary}
                  value={password}
                  onChangeText={setPassword}
                  secureTextEntry={!showPassword}
                  returnKeyType="go"
                  onSubmitEditing={handleLogin}
                  selectionColor={colors.accent}
                />
                <Pressable
                  style={styles.eyeButton}
                  onPress={() => {
                    setShowPassword(!showPassword);
                    lightImpact();
                  }}
                  hitSlop={8}
                >
                  <Ionicons
                    name={showPassword ? "eye-off-outline" : "eye-outline"}
                    size={20}
                    color={colors.textTertiary}
                  />
                </Pressable>
              </View>
            </View>

            {error !== "" && (
              <Animated.View entering={FadeIn.duration(200)} style={styles.errorContainer}>
                <Ionicons name="alert-circle" size={16} color={colors.error} />
                <Text style={styles.errorText}>{error}</Text>
              </Animated.View>
            )}

            <Pressable
              style={[styles.loginButton, loading && styles.buttonDisabled]}
              onPress={handleLogin}
              disabled={loading}
            >
              {loading ? (
                <ActivityIndicator size="small" color={colors.textInverse} />
              ) : (
                <Text style={styles.loginButtonText}>Sign In</Text>
              )}
            </Pressable>
          </View>
        </Animated.View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.bg,
  },
  scrollContent: {
    flexGrow: 1,
    justifyContent: "center",
    paddingHorizontal: spacing.xxl,
    paddingVertical: spacing.xxxl,
  },
  header: {
    alignItems: "center",
    marginBottom: 36,
  },
  logoContainer: {
    width: 88,
    height: 88,
    borderRadius: 22,
    backgroundColor: colors.accentBg,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: spacing.lg,
    shadowColor: colors.accent,
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.2,
    shadowRadius: 12,
    elevation: 4,
  },
  title: {
    fontSize: 32,
    fontWeight: "700",
    color: colors.text,
    letterSpacing: -0.5,
  },
  subtitle: {
    fontSize: 15,
    color: colors.textSecondary,
    marginTop: spacing.xs,
  },
  card: {
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.xl,
    gap: spacing.lg,
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.2,
    shadowRadius: 12,
    elevation: 4,
  },
  field: {
    gap: spacing.sm,
  },
  label: {
    fontSize: 13,
    fontWeight: "600",
    color: colors.textSecondary,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  input: {
    backgroundColor: colors.bgInput,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    paddingHorizontal: spacing.lg,
    paddingVertical: 14,
    color: colors.text,
    fontSize: 16,
  },
  inputRow: {
    flexDirection: "row",
    alignItems: "center",
  },
  inputFlex: {
    flex: 1,
  },
  statusIcon: {
    position: "absolute",
    right: 12,
  },
  eyeButton: {
    position: "absolute",
    right: 12,
    padding: spacing.sm,
  },
  checkButton: {
    alignSelf: "flex-start",
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    borderRadius: radius.sm,
    borderWidth: 1,
    borderColor: colors.accent,
  },
  checkButtonText: {
    fontSize: 13,
    fontWeight: "500",
    color: colors.accent,
  },
  connectingRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.sm,
  },
  connectingText: {
    fontSize: 13,
    color: colors.accent,
    fontWeight: "500",
  },
  errorContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.sm,
    backgroundColor: colors.errorBg,
    padding: spacing.md,
    borderRadius: radius.md,
  },
  errorText: {
    fontSize: 13,
    color: colors.error,
    flex: 1,
  },
  loginButton: {
    backgroundColor: colors.accent,
    borderRadius: radius.md,
    paddingVertical: 16,
    alignItems: "center",
    justifyContent: "center",
    shadowColor: colors.accent,
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.25,
    shadowRadius: 8,
    elevation: 3,
  },
  loginButtonText: {
    color: colors.textInverse,
    fontSize: 16,
    fontWeight: "600",
  },
  buttonDisabled: {
    opacity: 0.6,
  },
});
