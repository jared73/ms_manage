import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getModelStats, getUsageSummary, getUsageSummaryByApp, getUsageTrends,
  type DailyStats, type ModelStats, type UsageSummary, type UsageSummaryByApp } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { Section } from "@/components/Section";
import { UsageHero } from "./UsageHero";
import { UsageTrendChart } from "./UsageTrendChart";
import { ModelStatsTable } from "./ModelStatsTable";
import { RequestLogTable } from "./RequestLogTable";
import { DataSourcesPanel } from "./DataSourcesPanel";

const RANGES = [
  { key: "7d", days: 7 },
  { key: "30d", days: 30 },
  { key: "90d", days: 90 },
  { key: "all", days: 0 },
] as const;

function rangeToEpoch(days: number): number | undefined {
  if (days === 0) return undefined;
  return Math.floor(Date.now() / 1000) - days * 86400;
}

export function Usage() {
  const { t } = useTranslation();
  const [rangeKey, setRangeKey] = useState<(typeof RANGES)[number]["key"]>("30d");
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [byApp, setByApp] = useState<UsageSummaryByApp[]>([]);
  const [trends, setTrends] = useState<DailyStats[]>([]);
  const [models, setModels] = useState<ModelStats[]>([]);
  const [error, setError] = useState<string | null>(null);

  const startDate = useMemo(() => rangeToEpoch((RANGES.find((r) => r.key === rangeKey) ?? RANGES[0]).days), [rangeKey]);

  const requestSeqRef = useRef(0);

  const load = useCallback(async () => {
    const reqId = ++requestSeqRef.current;
    try {
      const [s, b, tr, m] = await Promise.all([
        getUsageSummary(startDate),
        getUsageSummaryByApp(startDate),
        getUsageTrends(startDate),
        getModelStats(startDate),
      ]);
      // 丢弃过期响应（快速切换 range 时旧请求可能晚返回）
      if (reqId !== requestSeqRef.current) return;
      setSummary(s);
      setByApp(b);
      setTrends(tr);
      setModels(m);
      setError(null);
    } catch (e) {
      if (reqId !== requestSeqRef.current) return;
      setError(describeError(e));
    }
  }, [startDate]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-5 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-bold">{t("nav.usage")}</h1>
        <div className="flex gap-1 rounded-lg border border-border p-0.5">
          {RANGES.map((r) => (
            <button
              key={r.key}
              onClick={() => setRangeKey(r.key)}
              className={`rounded-md px-3 py-1 text-xs ${
                rangeKey === r.key ? "bg-accent text-white" : "text-fg-muted hover:text-fg"
              }`}
            >
              {t(`usage.range.${r.key}`)}
            </button>
          ))}
        </div>
      </div>

      {error && <div className="rounded-lg bg-red-500/10 p-3 text-xs text-red-600">{error}</div>}

      <UsageHero summary={summary} />

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <Section title={t("usage.trend")} className="lg:col-span-2">
          <UsageTrendChart data={trends} />
        </Section>
        <DataSourcesPanel onSynced={load} />
      </div>

      {byApp.length > 0 && (
        <Section title={t("usage.by_app")}>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
            {byApp.map((a) => (
              <div key={a.appType} className="rounded-xl border border-border p-3">
                <div className="text-xs font-medium">{a.appType}</div>
                <div className="mt-1 text-lg font-bold">${Number(a.totalCost).toFixed(2)}</div>
                <div className="text-2xs text-fg-muted">{a.requestCount.toLocaleString()} req</div>
              </div>
            ))}
          </div>
        </Section>
      )}

      {models.length > 0 && (
        <Section title={t("usage.by_model")}>
          <ModelStatsTable data={models} />
        </Section>
      )}

      <Section title={t("usage.request_logs")}>
        <RequestLogTable startDate={startDate} />
      </Section>
    </div>
  );
}
