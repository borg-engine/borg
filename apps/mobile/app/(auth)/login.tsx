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
import { useAuth } from "@/lib/auth-context";
import { checkConnection } from "@/lib/auth";
import { colors, spacing, radius, common } from "@/lib/theme";

export default function LoginScreen() {
  const { login } = useAuth();
  const [serverUrl, setServerUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [serverChecked, setServerChecked] = useState(false);
  const [serverOk, setServerOk] = useState(false);

  const handleCheckServer = useCallback(async () => {
    if (!serverUrl.trim()) {
      setError("Enter a server URL");
      return;
    }
    setLoading(true);
    setError("");
    const ok = await checkConnection(serverUrl.trim());
    setServerOk(ok);
    setServerChecked(true);
    setLoading(false);
    if (!ok) {
      setError("Could not connect to server");
    }
  }, [serverUrl]);

  const handleLogin = useCallback(async () => {
    if (!serverUrl.trim() || !username.trim() || !password) {
      setError("All fields are required");
      return;
    }
    setLoading(true);
    setError("");
    const result = await login(serverUrl.trim(), username.trim(), password);
    setLoading(false);
    if (result.success) {
      router.replace("/(tabs)");
    } else {
      setError(result.error || "Login failed");
    }
  }, [serverUrl, username, password, login]);

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === "ios" ? "padding" : "height"}
    >
      <ScrollView
        contentContainerStyle={styles.scrollContent}
        keyboardShouldPersistTaps="handled"
      >
        <View style={styles.header}>
          <View style={styles.logoContainer}>
            <Ionicons name="cube" size={48} color={colors.accent} />
          </View>
          <Text style={styles.title}>Borg</Text>
          <Text style={styles.subtitle}>Autonomous Agent Orchestrator</Text>
        </View>

        <View style={styles.form}>
          <View style={styles.fieldGroup}>
            <Text style={styles.label}>Server URL</Text>
            <View style={styles.inputRow}>
              <TextInput
                style={[styles.input, styles.inputFlex]}
                placeholder="https://your-server.example.com"
                placeholderTextColor={colors.textTertiary}
                value={serverUrl}
                onChangeText={(t) => {
                  setServerUrl(t);
                  setServerChecked(false);
                  setServerOk(false);
                }}
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="url"
                returnKeyType="next"
              />
              {serverChecked && (
                <View style={styles.statusDot}>
                  <Ionicons
                    name={serverOk ? "checkmark-circle" : "close-circle"}
                    size={20}
                    color={serverOk ? colors.success : colors.error}
                  />
                </View>
              )}
            </View>
            {!serverChecked && (
              <Pressable
                style={[styles.checkButton, loading && styles.buttonDisabled]}
                onPress={handleCheckServer}
                disabled={loading}
              >
                {loading ? (
                  <ActivityIndicator size="small" color={colors.accent} />
                ) : (
                  <Text style={styles.checkButtonText}>Check Connection</Text>
                )}
              </Pressable>
            )}
          </View>

          <View style={styles.fieldGroup}>
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
            />
          </View>

          <View style={styles.fieldGroup}>
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
              />
              <Pressable
                style={styles.eyeButton}
                onPress={() => setShowPassword(!showPassword)}
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
            <View style={styles.errorContainer}>
              <Ionicons name="alert-circle" size={16} color={colors.error} />
              <Text style={styles.errorText}>{error}</Text>
            </View>
          )}

          <Pressable
            style={[common.buttonPrimary, styles.loginButton, loading && styles.buttonDisabled]}
            onPress={handleLogin}
            disabled={loading}
          >
            {loading ? (
              <ActivityIndicator size="small" color={colors.textInverse} />
            ) : (
              <Text style={common.buttonPrimaryText}>Sign In</Text>
            )}
          </Pressable>
        </View>
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
    marginBottom: 40,
  },
  logoContainer: {
    width: 88,
    height: 88,
    borderRadius: 22,
    backgroundColor: colors.accentBg,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: spacing.lg,
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
  form: {
    gap: spacing.lg,
  },
  fieldGroup: {
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
    fontSize: 15,
  },
  inputRow: {
    flexDirection: "row",
    alignItems: "center",
  },
  inputFlex: {
    flex: 1,
  },
  statusDot: {
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
    marginTop: spacing.sm,
    paddingVertical: 16,
  },
  buttonDisabled: {
    opacity: 0.6,
  },
});
