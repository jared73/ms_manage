import { useTranslation } from "react-i18next";
import type { UsageSummary } from "@/lib/ipc";

export function UsageHero({ summary }: { summary: UsageSummary | null }) {
  const { t } = useTranslation();
  const cost = summary ? Number(summary.totalCost) : 0;
  const tokens = summary?.totalTokens ?? 0;

  const cards = [
    { key: "cost", value: `$${cost.toFixed(4)}`, label: t("usage.total_cost") },
    { key: "tokens", value: tokens.toLocaleString(), label: t("usage.total_tokens") },
    { key: "requests", value: (summary?.requestCount ?? 0).toLocaleString(), label: t("usage.total_requests") },
  ];

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
      {cards.map((c) => (
        <div key={c.key} className="rounded-xl border border-border bg-bg-subtle p-4">
          <div className="text-2xs text-fg-muted">{c.label}</div>
          <div className="mt-1 text-2xl font-bold">{c.value}</div>
        </div>
      ))}
    </div>
  );
}
