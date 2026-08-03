import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getModelStats, getProviderStats, getUsageSummary, getUsageTrends,
  type DailyStats, type ModelStats, type ProviderStats, type UsageSummary } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { Section } from "@/components/Section";
import { UsageHero } from "./UsageHero";
import { UsageTrendChart } from "./UsageTrendChart";
import { ModelStatsTable } from "./ModelStatsTable";
import { ProviderStatsTable } from "./ProviderStatsTable";
import { RequestLogTable } from "./RequestLogTable";
import { DataSourcesPanel } from "./DataSourcesPanel";
import { PricingPanel } from "./PricingPanel";

const RANGES = [
  { key: "today", days: 0 },
  { key: "7d", days: 7 },
  { key: "30d", days: 30 },
  { key: "90d", days: 90 },
  { key: "all", days: -1 },
] as const;

type TabKey = "logs" | "providers" | "models";

const TABS: { key: TabKey; label: string }[] = [
  { key: "logs", label: "usage.request_logs" },
  { key: "providers", label: "usage.provider_stats" },
  { key: "models", label: "usage.model_stats" },
];

function rangeToEpoch(days: number): number | undefined {
  if (days === -1) return undefined;
  if (days === 0) {
    // 当天：今天本地时区 0 点
    const now = new Date();
    return Math.floor(new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000);
  }
  return Math.floor(Date.now() / 1000) - days * 86400;
}

export function Usage() {
  const { t } = useTranslation();
  const [rangeKey, setRangeKey] = useState<(typeof RANGES)[number]["key"]>("30d");
  const [tab, setTab] = useState<TabKey>("logs");
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [trends, setTrends] = useState<DailyStats[]>([]);
  const [providers, setProviders] = useState<ProviderStats[]>([]);
  const [models, setModels] = useState<ModelStats[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [refreshMs, setRefreshMs] = useState(0);

  const startDate = useMemo(() => rangeToEpoch((RANGES.find((r) => r.key === rangeKey) ?? RANGES[0]).days), [rangeKey]);
  const grouping = rangeKey === "today" ? "hour" : "day";

  const requestSeqRef = useRef(0);

  const load = useCallback(async () => {
    const reqId = ++requestSeqRef.current;
    try {
      const [s, tr, p, m] = await Promise.all([
        getUsageSummary(startDate),
        getUsageTrends(startDate, undefined, grouping),
        getProviderStats(startDate),
        getModelStats(startDate),
      ]);
      // 丢弃过期响应（快速切换 range 时旧请求可能晚返回）
      if (reqId !== requestSeqRef.current) return;
      setSummary(s);
      setTrends(tr);
      setProviders(p);
      setModels(m);
      setError(null);
    } catch (e) {
      if (reqId !== requestSeqRef.current) return;
      setError(describeError(e));
    }
  }, [startDate, grouping]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (refreshMs <= 0) return;
    const id = setInterval(() => {
      void load();
    }, refreshMs);
    return () => clearInterval(id);
  }, [refreshMs, load]);

  return (
    <div className="space-y-5 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-bold">{t("nav.usage")}</h1>
        <div className="flex items-center gap-3">
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
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            {t("usage.refresh")}
            <select
              value={refreshMs}
              onChange={(e) => setRefreshMs(Number(e.target.value))}
              className="rounded border border-border bg-bg px-2 py-1 text-xs text-fg-muted outline-none focus:border-accent"
            >
              <option value={0}>{t("usage.refresh_off")}</option>
              <option value={5000}>5s</option>
              <option value={10000}>10s</option>
              <option value={30000}>30s</option>
              <option value={60000}>60s</option>
            </select>
          </label>
        </div>
      </div>

      {error && <div className="rounded-lg bg-red-500/10 p-3 text-xs text-red-600">{error}</div>}

      <UsageHero summary={summary} />

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <Section title={t("usage.trend")} className="lg:col-span-2">
          <UsageTrendChart data={trends} grouping={grouping} />
        </Section>
        <DataSourcesPanel onSynced={load} />
      </div>

      <div className="flex gap-1 rounded-lg border border-border p-0.5 w-fit">
        {TABS.map((tb) => (
          <button
            key={tb.key}
            onClick={() => setTab(tb.key)}
            className={`rounded-md px-3 py-1 text-xs ${
              tab === tb.key ? "bg-accent text-white" : "text-fg-muted hover:text-fg"
            }`}
          >
            {t(tb.label)}
          </button>
        ))}
      </div>

      <Section>
        {tab === "logs" && <RequestLogTable startDate={startDate} />}
        {tab === "providers" && <ProviderStatsTable data={providers} />}
        {tab === "models" && <ModelStatsTable data={models} />}
      </Section>

      <Section title={t("usage.model_pricing")}>
        <PricingPanel />
      </Section>
    </div>
  );
}
