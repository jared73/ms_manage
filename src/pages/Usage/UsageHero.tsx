import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { UsageSummary } from "@/lib/ipc";

export function UsageHero({ summary }: { summary: UsageSummary | null }) {
  const { t, i18n } = useTranslation();
  const cost = summary ? Number(summary.totalCost) : 0;
  const tokens = summary?.totalTokens ?? 0;

  // 紧凑数字格式：zh 自动 "1.2万 / 1.2亿"，en 自动 "12K / 1.2M"
  const compactFmt = useMemo(
    () =>
      new Intl.NumberFormat(i18n.language, {
        notation: "compact",
        maximumFractionDigits: 1,
      }),
    [i18n.language],
  );
  // 千分位数字：按 locale 自动加千分位（zh: 1,234；en: 1,234；de: 1.234）
  const intFmt = useMemo(() => new Intl.NumberFormat(i18n.language), [i18n.language]);
  // 货币：始终 CNY（定价表写死人民币）；按 locale 选小数点 / 千分位格式
  const costFmt = useMemo(
    () =>
      new Intl.NumberFormat(i18n.language, {
        minimumFractionDigits: 4,
        maximumFractionDigits: 4,
      }),
    [i18n.language],
  );

  const formatTokens = (n: number): string => compactFmt.format(n);

  const cards: { key: string; value: string; label: string; raw?: string }[] = [
    { key: "cost", value: `¥${costFmt.format(cost)}`, label: t("usage.total_cost") },
    { key: "tokens", value: formatTokens(tokens), label: t("usage.total_tokens"), raw: intFmt.format(tokens) },
    { key: "requests", value: intFmt.format(summary?.requestCount ?? 0), label: t("usage.total_requests") },
    {
      key: "hitRate",
      value: summary ? `${((summary.cacheHitRate ?? 0) * 100).toFixed(1)}%` : "0%",
      label: t("usage.cache_hit_rate"),
      raw: intFmt.format(summary?.totalCacheReadTokens ?? 0),
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
