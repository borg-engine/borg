import React, { useState, useEffect, useCallback } from "react";
import {
  View,
  Text,
  ScrollView,
  Pressable,
  StyleSheet,
  Alert,
  Switch,
} from "react-native";
import { router } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import Animated, { FadeInDown } from "react-native-reanimated";
import { useAuth } from "@/lib/auth-context";
import { useStatus, useUsage } from "@/lib/query";
import { getServerUrl } from "@/lib/auth";
import { colors, spacing, radius, common } from "@/lib/theme";
import { formatDuration } from "@/lib/utils";

function SettingsSection({
  title,
  children,
  delay = 0,
}: {
  title: string;
  children: React.ReactNode;
  delay?: number;
}) {
  return (
    <Animated.View entering={FadeInDown.duration(300).delay(delay)}>
      <Text style={styles.sectionTitle}>{title}</Text>
      <View style={styles.sectionCard}>{children}</View>
    </Animated.View>
  );
}

function SettingsRow({
  icon,
  label,
  value,
  valueColor,
  onPress,
  rightElement,
  last = false,
}: {
  icon: keyof typeof Ionicons.glyphMap;
  label: string;
  value?: string;
  valueColor?: string;
  onPress?: () => void;
  rightElement?: React.ReactNode;
  last?: boolean;
}) {
  const content = (
    <View style={[styles.row, !last && styles.rowBorder]}>
      <View style={styles.rowLeft}>
        <Ionicons name={icon} size={18} color={colors.textSecondary} />
        <Text style={styles.rowLabel}>{label}</Text>
      </View>
      <View style={styles.rowRight}>
        {value && (
          <Text
            style={[styles.rowValue, valueColor ? { color: valueColor } : null]}
            numberOfLines={1}
          >
            {value}
          </Text>
        )}
        {rightElement}
        {onPress && (
          <Ionicons name="chevron-forward" size={16} color={colors.textTertiary} />
        )}
      </View>
    </View>
  );

  if (onPress) {
    return (
      <Pressable onPress={onPress} android_ripple={{ color: colors.bgHover }}>
        {content}
      </Pressable>
    );
  }

  return content;
}

export default function SettingsScreen() {
  const { user, logout } = useAuth();
  const { data: status } = useStatus();
  const { data: usage } = useUsage();
  const [serverUrl, setServerUrl] = useState<string>("");

  useEffect(() => {
    getServerUrl().then((url) => setServerUrl(url ?? ""));
  }, []);

  const handleLogout = useCallback(() => {
    Alert.alert("Sign Out", "Are you sure you want to sign out?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Sign Out",
        style: "destructive",
        onPress: async () => {
          await logout();
          router.replace("/(auth)/login");
        },
      },
    ]);
  }, [logout]);

  const connected = !!status;

  return (
    <ScrollView
      style={common.screen}
      contentContainerStyle={styles.content}
      showsVerticalScrollIndicator={false}
    >
      <SettingsSection title="Connection" delay={0}>
        <SettingsRow
          icon="server-outline"
          label="Server"
          value={serverUrl ? new URL(serverUrl).hostname : "Not configured"}
        />
        <SettingsRow
          icon="pulse-outline"
          label="Status"
          value={connected ? "Connected" : "Disconnected"}
          valueColor={connected ? colors.success : colors.error}
          rightElement={
            <View
              style={[
                styles.statusDot,
                { backgroundColor: connected ? colors.success : colors.error },
              ]}
            />
          }
        />
        {status && (
          <>
            <SettingsRow
              icon="time-outline"
              label="Uptime"
              value={formatDuration(status.uptime_s)}
            />
            <SettingsRow
              icon="hardware-chip-outline"
              label="Version"
              value={status.version}
              last
            />
          </>
        )}
      </SettingsSection>

      {user && (
        <SettingsSection title="Account" delay={50}>
          <SettingsRow
            icon="person-outline"
            label="Username"
            value={user.username}
          />
          {user.display_name && (
            <SettingsRow
              icon="id-card-outline"
              label="Display Name"
              value={user.display_name}
            />
          )}
          <SettingsRow
            icon="shield-outline"
            label="Role"
            value={user.is_admin ? "Admin" : "User"}
            valueColor={user.is_admin ? colors.accent : undefined}
            last
          />
        </SettingsSection>
      )}

      {status && (
        <SettingsSection title="Pipeline" delay={100}>
          <SettingsRow
            icon="rocket-outline"
            label="Active Tasks"
            value={String(status.active_tasks)}
            valueColor={status.active_tasks > 0 ? colors.statusActive : undefined}
          />
          <SettingsRow
            icon="checkmark-done-outline"
            label="Merged Tasks"
            value={String(status.merged_tasks)}
            valueColor={colors.statusDone}
          />
          <SettingsRow
            icon="close-circle-outline"
            label="Failed Tasks"
            value={String(status.failed_tasks)}
            valueColor={status.failed_tasks > 0 ? colors.error : undefined}
          />
          <SettingsRow
            icon="people-outline"
            label="Dispatched Agents"
            value={String(status.dispatched_agents)}
          />
          <SettingsRow
            icon="flash-outline"
            label="AI Requests"
            value={String(status.ai_requests)}
            last
          />
        </SettingsSection>
      )}

      {status?.available_models && status.available_models.length > 0 && (
        <SettingsSection title="Models" delay={150}>
          {status.available_models.map((m, i) => (
            <SettingsRow
              key={m.model}
              icon="cube-outline"
              label={m.label || m.model}
              value={m.backend}
              last={i === status.available_models!.length - 1}
            />
          ))}
        </SettingsSection>
      )}

      {usage && (
        <SettingsSection title="Usage" delay={200}>
          <SettingsRow
            icon="analytics-outline"
            label="Total Tasks"
            value={String(usage.task_count)}
          />
          <SettingsRow
            icon="chatbubble-outline"
            label="Messages"
            value={String(usage.message_count)}
          />
          <SettingsRow
            icon="cash-outline"
            label="Estimated Cost"
            value={`$${usage.total_cost_usd.toFixed(2)}`}
            last
          />
        </SettingsSection>
      )}

      {status?.watched_repos && status.watched_repos.length > 0 && (
        <SettingsSection title="Watched Repos" delay={250}>
          {status.watched_repos.map((repo, i) => {
            const name = repo.path.split("/").pop() || repo.path;
            return (
              <SettingsRow
                key={repo.path}
                icon="git-branch-outline"
                label={name}
                value={repo.mode}
                last={i === status.watched_repos.length - 1}
              />
            );
          })}
        </SettingsSection>
      )}

      <Animated.View entering={FadeInDown.duration(300).delay(300)}>
        <Pressable style={styles.logoutButton} onPress={handleLogout}>
          <Ionicons name="log-out-outline" size={20} color={colors.error} />
          <Text style={styles.logoutText}>Sign Out</Text>
        </Pressable>
      </Animated.View>

      <Text style={styles.versionText}>Borg Mobile v0.1.0</Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  content: {
    padding: spacing.lg,
    paddingBottom: 100,
    gap: spacing.xl,
  },
  sectionTitle: {
    fontSize: 13,
    fontWeight: "600",
    color: colors.textTertiary,
    textTransform: "uppercase",
    letterSpacing: 0.5,
    marginBottom: spacing.sm,
    marginLeft: spacing.xs,
  },
  sectionCard: {
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    overflow: "hidden",
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
  },
  rowBorder: {
    borderBottomWidth: 1,
    borderBottomColor: colors.borderSubtle,
  },
  rowLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.md,
    flex: 1,
  },
  rowLabel: {
    fontSize: 15,
    color: colors.text,
  },
  rowRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: spacing.sm,
    maxWidth: "50%",
  },
  rowValue: {
    fontSize: 14,
    color: colors.textSecondary,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  logoutButton: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: spacing.sm,
    backgroundColor: colors.errorBg,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: "rgba(239, 68, 68, 0.2)",
    paddingVertical: 16,
  },
  logoutText: {
    fontSize: 15,
    fontWeight: "600",
    color: colors.error,
  },
  versionText: {
    fontSize: 12,
    color: colors.textTertiary,
    textAlign: "center",
    marginTop: spacing.sm,
  },
});
