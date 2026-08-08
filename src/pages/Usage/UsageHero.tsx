import { useTranslation } from "react-i18next";
import type { UsageSummary } from "@/lib/ipc";

export function UsageHero({ summary }: { summary: UsageSummary | null }) {
  const { t } = useTranslation();
  const cost = summary ? Number(summary.totalCost) : 0;
  const tokens = summary?.totalTokens ?? 0;

  const formatTokens = (n: number): string => {
    if (n >= 100_000_000) return `${t("usage.about")} ${(n / 100_000_000).toFixed(2)} ${t("usage.hundred_million")}`;
    if (n >= 10_000) return `${t("usage.about")} ${(n / 10_000).toFixed(1)} ${t("usage.ten_thousand")}`;
    return n.toLocaleString();
  };

  const cards: { key: string; value: string; label: string; raw?: string }[] = [
    { key: "cost", value: `¥${cost.toFixed(4)}`, label: t("usage.total_cost") },
    { key: "tokens", value: formatTokens(tokens), label: t("usage.total_tokens"), raw: tokens.toLocaleString() },
    { key: "requests", value: (summary?.requestCount ?? 0).toLocaleString(), label: t("usage.total_requests") },
    {
      key: "hitRate",
      value: summary ? `${((summary.cacheHitRate ?? 0) * 100).toFixed(1)}%` : "0%",
      label: t("usage.cache_hit_rate"),
      raw: (summary?.totalCacheReadTokens ?? 0).toLocaleString(),
    },
    {
      key: "failRate",
      value: summary ? `${((summary.failureRate ?? 0) * 100).toFixed(2)}%` : "0%",
      label: t("usage.failure_rate"),
      raw: `${summary?.failedCount ?? 0} / ${summary?.requestCount ?? 0}`,
    },
  ];

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
      {cards.map((c) => (
        <div key={c.key} className="rounded-xl border border-border bg-bg-subtle p-4">
          <div className="text-2xs text-fg-muted">{c.label}</div>
          <div className="mt-1 text-2xl font-bold">{c.value}</div>
          {c.raw != null && <div className="text-2xs text-fg-muted">{c.raw}</div>}
        </div>
      ))}
    </div>
  );
}
