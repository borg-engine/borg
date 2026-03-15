import { useLinkedCredentials, useUsageSummary } from "@/lib/api";

function formatNumber(n: number): string {
  return n.toLocaleString("en-US");
}

function formatCost(n: number): string {
  return `$${n.toFixed(2)}`;
}

export function UsagePanel() {
  const { data: usage, isLoading, error } = useUsageSummary();
  const { data: credentials } = useLinkedCredentials();

  const isSubscription = credentials?.credentials?.some(
    (c) => c.provider === "claude" && c.status === "connected" && c.auth_kind === "claude_code_session",
  );

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-[#2a2520] border-t-amber-400" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-xl border border-red-500/20 bg-red-500/[0.06] px-4 py-3 text-[12px] text-red-300">
        Failed to load usage data.
      </div>
    );
  }

  if (!usage) return null;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="mb-3 text-[12px] font-semibold uppercase tracking-wider text-[#6b6459]">Usage</h3>
        <div className="rounded-xl border border-[#2a2520] bg-[#1c1a17]/50 p-5">
          {isSubscription && (
            <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/[0.06] px-4 py-3 text-[12px] text-amber-300">
              Usage tracking is not available when using Claude subscription auth. Token counts are
              tracked by your Claude account.
            </div>
          )}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-[12px] text-[#6b6459]">Input Tokens</span>
              <span className="text-[12px] font-medium tabular-nums text-[#e8e0d4]">
                {formatNumber(usage.total_input_tokens)}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[12px] text-[#6b6459]">Output Tokens</span>
              <span className="text-[12px] font-medium tabular-nums text-[#e8e0d4]">
                {formatNumber(usage.total_output_tokens)}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[12px] text-[#6b6459]">Total Cost</span>
              <span className="text-[12px] font-medium tabular-nums text-[#e8e0d4]">
                {formatCost(usage.total_cost_usd)}
              </span>
            </div>
            <div className="h-px bg-[#2a2520]" />
            <div className="flex items-center justify-between">
              <span className="text-[12px] text-[#6b6459]">Messages</span>
              <span className="text-[12px] font-medium tabular-nums text-[#e8e0d4]">
                {formatNumber(usage.message_count)}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-[12px] text-[#6b6459]">Tasks</span>
              <span className="text-[12px] font-medium tabular-nums text-[#e8e0d4]">
                {formatNumber(usage.task_count)}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
