export function timeAgo(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);

  if (diffSecs < 60) return "just now";
  if (diffSecs < 3600) {
    const mins = Math.floor(diffSecs / 60);
    return `${mins}m ago`;
  }
  if (diffSecs < 86400) {
    const hours = Math.floor(diffSecs / 3600);
    return `${hours}h ago`;
  }
  if (diffSecs < 604800) {
    const days = Math.floor(diffSecs / 86400);
    return `${days}d ago`;
  }
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return s > 0 ? `${m}m ${s}s` : `${m}m`;
  }
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

export function truncate(str: string, maxLen: number): string {
  if (str.length <= maxLen) return str;
  return str.slice(0, maxLen - 1) + "\u2026";
}

export function capitalize(str: string): string {
  if (!str) return "";
  return str.charAt(0).toUpperCase() + str.slice(1);
}
