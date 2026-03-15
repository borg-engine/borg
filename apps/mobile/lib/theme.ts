import { StyleSheet, Platform } from "react-native";

export const colors = {
  bg: "#0f0e0c",
  bgElevated: "#1c1a17",
  bgCard: "#1c1a17",
  card: "#1c1a17",
  bgInput: "#262320",
  bgHover: "#2a2723",
  border: "#33302b",
  borderSubtle: "#2a2723",

  accent: "#f59e0b",
  accentDim: "#b45309",
  accentBg: "rgba(245, 158, 11, 0.12)",

  text: "#fafaf9",
  textSecondary: "#a8a29e",
  textTertiary: "#78716c",
  textInverse: "#0f0e0c",

  success: "#22c55e",
  successBg: "rgba(34, 197, 94, 0.12)",
  error: "#ef4444",
  errorBg: "rgba(239, 68, 68, 0.12)",
  warning: "#f59e0b",
  warningBg: "rgba(245, 158, 11, 0.12)",
  info: "#3b82f6",
  infoBg: "rgba(59, 130, 246, 0.12)",

  statusActive: "#f59e0b",
  statusDone: "#22c55e",
  statusFailed: "#ef4444",
  statusQueued: "#78716c",
  statusReview: "#3b82f6",
  statusMerged: "#a855f7",
} as const;

export const spacing = {
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 20,
  xxl: 24,
  xxxl: 32,
} as const;

export const radius = {
  sm: 6,
  md: 10,
  lg: 14,
  xl: 20,
  full: 9999,
} as const;

export const fontSize = { xs: 11, sm: 13, md: 15, lg: 17, xl: 20, xxl: 28 } as const;

export const fonts = {
  body: { fontSize: 15, lineHeight: 22 },
  bodySmall: { fontSize: 13, lineHeight: 18 },
  caption: { fontSize: 11, lineHeight: 16 },
  heading: { fontSize: 22, lineHeight: 28, fontWeight: "700" as const },
  subheading: { fontSize: 17, lineHeight: 24, fontWeight: "600" as const },
  label: { fontSize: 13, lineHeight: 18, fontWeight: "600" as const },
  mono: { fontSize: 13, lineHeight: 18, fontFamily: Platform.OS === "ios" ? "Menlo" : "monospace" },
} as const;

export function statusColor(status: string): string {
  switch (status) {
    case "done":
    case "complete":
      return colors.statusDone;
    case "merged":
      return colors.statusMerged;
    case "failed":
    case "error":
      return colors.statusFailed;
    case "review":
    case "human_review":
      return colors.statusReview;
    case "queued":
    case "backlog":
      return colors.statusQueued;
    default:
      return colors.statusActive;
  }
}

export function statusBgColor(status: string): string {
  switch (status) {
    case "done":
    case "complete":
      return colors.successBg;
    case "merged":
      return "rgba(168, 85, 247, 0.12)";
    case "failed":
    case "error":
      return colors.errorBg;
    case "review":
    case "human_review":
      return colors.infoBg;
    case "queued":
    case "backlog":
      return "rgba(120, 113, 108, 0.12)";
    default:
      return colors.warningBg;
  }
}

export const common = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg,
  },
  screenPadded: {
    flex: 1,
    backgroundColor: colors.bg,
    paddingHorizontal: spacing.lg,
  },
  card: {
    backgroundColor: colors.bgCard,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: colors.border,
    padding: spacing.lg,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
  },
  rowBetween: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  separator: {
    height: 1,
    backgroundColor: colors.border,
  },
  badge: {
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.xs,
    borderRadius: radius.sm,
  },
  badgeText: {
    fontSize: 11,
    fontWeight: "600",
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  input: {
    backgroundColor: colors.bgInput,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
    color: colors.text,
    fontSize: 15,
  },
  buttonPrimary: {
    backgroundColor: colors.accent,
    borderRadius: radius.md,
    paddingHorizontal: spacing.xl,
    paddingVertical: spacing.md,
    alignItems: "center",
    justifyContent: "center",
  },
  buttonPrimaryText: {
    color: colors.textInverse,
    fontSize: 15,
    fontWeight: "600",
  },
  buttonSecondary: {
    backgroundColor: "transparent",
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: colors.border,
    paddingHorizontal: spacing.xl,
    paddingVertical: spacing.md,
    alignItems: "center",
    justifyContent: "center",
  },
  buttonSecondaryText: {
    color: colors.text,
    fontSize: 15,
    fontWeight: "500",
  },
});
