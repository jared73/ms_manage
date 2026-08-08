import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, RefreshCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getModelStats, getProviderStats, getUsageSummary, getUsageTrends,
  getUsageDataSources,
  recalculateCosts,
  type DailyStats, type DataSourceStatus, type ModelStats, type ProviderStats, type UsageSummary } from "@/lib/ipc";
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
  { key: "custom", days: 0 },
] as const;

type RangeKey = (typeof RANGES)[number]["key"];

type TabKey = "logs" | "providers" | "models";

function dateInputToEpoch(d: string, endOfDay = false): number | undefined {
  if (!d) return undefined;
  const [y, m, day] = d.split("-").map(Number);
  if (!y || !m || !day) return undefined;
  const date = new Date(y, m - 1, day, endOfDay ? 23 : 0, endOfDay ? 59 : 0, endOfDay ? 59 : 0);
  return Math.floor(date.getTime() / 1000);
}

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

const APP_FILTERS_STATIC = [
  { key: "", label: "usage.app_all" },
] as const;

export function Usage() {
  const { t } = useTranslation();
  const [rangeKey, setRangeKey] = useState<RangeKey>("today");
  const [customStart, setCustomStart] = useState<string>("");
  const [customEnd, setCustomEnd] = useState<string>("");
  const [tab, setTab] = useState<TabKey>("logs");
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [trends, setTrends] = useState<DailyStats[]>([]);
  const [providers, setProviders] = useState<ProviderStats[]>([]);
  const [models, setModels] = useState<ModelStats[]>([]);
  const [dataSources, setDataSources] = useState<DataSourceStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [refreshMs, setRefreshMs] = useState(0);
  const [pricingOpen, setPricingOpen] = useState(false);
  const [appFilter, setAppFilter] = useState<string>("");
  const [modelFilter, setModelFilter] = useState<string>("");
  const [recalcBusy, setRecalcBusy] = useState(false);
  const [recalcProgress, setRecalcProgress] = useState<number | null>(null);

  const customRange = useMemo(() => {
    const start = dateInputToEpoch(customStart, false);
    const end = dateInputToEpoch(customEnd, true);
    return { start, end };
  }, [customStart, customEnd]);

  const startDate = useMemo(() => {
    if (rangeKey === "custom") return customRange.start;
    return rangeToEpoch((RANGES.find((r) => r.key === rangeKey) ?? RANGES[0]).days);
  }, [rangeKey, customRange]);

  const endDate = rangeKey === "custom" ? customRange.end : undefined;
  const grouping = rangeKey === "today" ? "hour" : "day";
  const appParam = appFilter || undefined;
  const modelParam = modelFilter || undefined;

  // APP_FILTERS 从后端数据源派生（新增数据源自动可见，无需前端硬编码）
  const appFilters = useMemo(
    () => [
      ...APP_FILTERS_STATIC,
      ...dataSources.map((s) => ({ key: s.appType, label: s.label })),
    ],
    [dataSources],
  );

  const requestSeqRef = useRef(0);

  const load = useCallback(async () => {
    const reqId = ++requestSeqRef.current;
    try {
      const [s, tr, p, m, ds] = await Promise.all([
        getUsageSummary(startDate, endDate, appParam),
        getUsageTrends(startDate, endDate, grouping, appParam, modelParam),
        getProviderStats(startDate, endDate),
        getModelStats(startDate, endDate),
        getUsageDataSources(),
      ]);
      // 丢弃过期响应（快速切换 range 时旧请求可能晚返回）
      if (reqId !== requestSeqRef.current) return;
      setSummary(s);
      setTrends(tr);
      setProviders(p);
      setModels(m);
      setDataSources(ds);
      setError(null);
    } catch (e) {
      if (reqId !== requestSeqRef.current) return;
      setError(describeError(e));
    }
  }, [startDate, endDate, grouping, appParam, modelParam]);

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

  const doRecalc = async () => {
    let unlisten: UnlistenFn | null = null;
    setRecalcBusy(true);
    setRecalcProgress(null);
    try {
      // 订阅后端进度事件：每 500 条 emit 一次当前已处理条数
      unlisten = await listen<number>("usage://recalc-progress", (e) => {
        setRecalcProgress(e.payload);
      });
      const n = await recalculateCosts();
      toast.success(t("usage.recalc_done", { count: n }));
      await load();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setRecalcBusy(false);
      setRecalcProgress(null);
      if (unlisten) {
        unlisten();
      }
    }
  };

  return (
    <div className="space-y-5 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-bold">{t("nav.usage")}</h1>
        <div className="flex items-center gap-3">
          <div className="flex gap-1 rounded-lg border border-border p-0.5" role="group">
            {RANGES.map((r) => (
              <button
                key={r.key}
                onClick={() => setRangeKey(r.key)}
                aria-pressed={rangeKey === r.key}
                className={`rounded-md px-3 py-1 text-xs ${
                  rangeKey === r.key ? "bg-accent text-white" : "text-fg-muted hover:text-fg"
                }`}
              >
                {t(`usage.range.${r.key}`)}
              </button>
            ))}
          </div>
          {rangeKey === "custom" && (
            <div className="flex items-center gap-1.5 text-xs">
              <input
                type="date"
                value={customStart}
                onChange={(e) => setCustomStart(e.target.value)}
                className="rounded border border-border bg-bg px-2 py-1 text-xs outline-none focus:border-accent"
                aria-label={t("usage.start_date") as string}
              />
              <span className="text-fg-muted">~</span>
              <input
                type="date"
                value={customEnd}
                onChange={(e) => setCustomEnd(e.target.value)}
                className="rounded border border-border bg-bg px-2 py-1 text-xs outline-none focus:border-accent"
                aria-label={t("usage.end_date") as string}
              />
            </div>
          )}
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
        <Section
          title={t("usage.trend")}
          className="lg:col-span-2"
          action={
            <div className="flex items-center gap-2">
              <select
                value={appFilter}
                onChange={(e) => {
                  setAppFilter(e.target.value);
                  // 切换应用时清空 model 筛选，避免跨应用的孤立选项
                  setModelFilter("");
                }}
                className="rounded border border-border bg-bg px-2 py-1 text-xs text-fg-muted outline-none focus:border-accent"
                aria-label={t("usage.filter_app")}
              >
                {appFilters.map((a) => (
                  <option key={a.key || "all"} value={a.key}>
                    {a.key === "" ? t(a.label as "usage.app_all") : a.label}
                  </option>
                ))}
              </select>
              <select
                value={modelFilter}
                onChange={(e) => setModelFilter(e.target.value)}
                className="max-w-[180px] rounded border border-border bg-bg px-2 py-1 text-xs text-fg-muted outline-none focus:border-accent"
                aria-label={t("usage.filter_model")}
              >
                <option value="">{t("usage.model_all")}</option>
                {models.map((m) => (
                  <option key={m.model} value={m.model}>
                    {m.model}
                  </option>
                ))}
              </select>
            </div>
          }
        >
          <UsageTrendChart data={trends} grouping={grouping} />
        </Section>
        <DataSourcesPanel onSynced={load} />
      </div>

      <div className="flex gap-1 rounded-lg border border-border p-0.5 w-fit" role="tablist">
        {TABS.map((tb) => (
          <button
            key={tb.key}
            role="tab"
            aria-selected={tab === tb.key}
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
        {tab === "logs" && <RequestLogTable startDate={startDate} appFilter={appFilter} modelFilter={modelFilter} />}
        {tab === "providers" && <ProviderStatsTable data={providers} />}
        {tab === "models" && <ModelStatsTable data={models} />}
      </Section>

      <div className="rounded-xl border border-border bg-bg-subtle">
        <button
          onClick={() => setPricingOpen((v) => !v)}
          className="flex w-full items-center justify-between px-4 py-3"
          aria-expanded={pricingOpen}
          aria-controls="pricing-content"
        >
          <span className="text-sm font-semibold">{t("usage.model_pricing")}</span>
          <ChevronDown size={16} className={`text-fg-muted transition-transform ${pricingOpen ? "rotate-180" : ""}`} />
        </button>
        <div id="pricing-content" className={`border-t border-border p-4 ${pricingOpen ? "block" : "hidden"}`}>
          <div className="mb-3 flex justify-end">
            <button
              onClick={doRecalc}
              disabled={recalcBusy}
              className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-overlay-soft disabled:opacity-50"
              title={t("usage.recalc_tip")}
            >
              <RefreshCcw size={13} className={recalcBusy ? "animate-spin" : ""} />
              {recalcBusy
                ? recalcProgress !== null
                  ? t("usage.recalc_progress", { processed: recalcProgress })
                  : t("usage.recalc_running")
                : t("usage.recalc")}
            </button>
          </div>
          <PricingPanel />
        </div>
      </div>
    </div>
  );
}
