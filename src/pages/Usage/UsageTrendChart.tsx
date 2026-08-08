import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Area,
  AreaChart,
  Bar,
  CartesianGrid,
  ComposedChart,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { DailyStats } from "@/lib/ipc";

const TOKEN_COLORS = {
  input: "rgb(var(--accent))",
  output: "rgb(var(--warn))",
  cacheRead: "#10b981",
  cacheCreation: "#8b5cf6",
  cost: "#f59e0b",
  cumulative: "rgb(var(--accent) / 0.3)",
} as const;

type TokenKey = "input" | "output" | "cacheRead" | "cacheCreation";

export function UsageTrendChart({
  data,
  grouping = "day",
}: {
  data: DailyStats[];
  grouping?: "day" | "hour";
}) {
  const { t } = useTranslation();
  const isHourly = grouping === "hour";

  const [stacked, setStacked] = useState(false);
  const [hidden, setHidden] = useState<Set<string>>(new Set());

  const chartData = useMemo(
    () =>
      data.map((d) => ({
        date: d.date,
        input: d.totalInputTokens,
        output: d.totalOutputTokens,
        cacheRead: d.totalCacheReadTokens,
        cacheCreation: d.totalCacheCreationTokens,
        cost: Number(d.totalCost),
      })),
    [data],
  );

  // 计算累计费用（用于 ComposedChart 的 Bar）
  const costChartData = useMemo(() => {
    let acc = 0;
    return chartData.map((d) => {
      acc += d.cost;
      return { ...d, cumulativeCost: acc };
    });
  }, [chartData]);

  const tickFormatter = (value: string) => (isHourly ? value.slice(-6) : value);
  const dot = isHourly ? false : { r: 2, strokeWidth: 0 };

  const toggleSeries = (key: string) => {
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const tokenLegend = [
    { key: "input", label: t("usage.input_tokens_short"), color: TOKEN_COLORS.input },
    { key: "output", label: t("usage.output_tokens_short"), color: TOKEN_COLORS.output },
    { key: "cacheRead", label: t("usage.cache_read_short"), color: TOKEN_COLORS.cacheRead },
    { key: "cacheCreation", label: t("usage.cache_creation_short"), color: TOKEN_COLORS.cacheCreation },
  ];

  // 自动隐藏全 0 的 series:避免「缓存创建:0」这种没有数据的线干扰阅读
  // 例:today 档 5 个数据源都没 cache_creation → 整条线恒 0,自动折叠图例和图。
  // 用户手动点击图例隐藏(放进 hidden Set)优先级仍最高。
  const visibleTokenLegend = tokenLegend.filter(
    (it) => !hidden.has(it.key) && chartData.some((d) => (d[it.key as TokenKey] ?? 0) > 0),
  );

  const tooltipContentStyle = {
    background: "rgb(var(--bg-elev))",
    border: "1px solid rgb(var(--border) / var(--border-alpha))",
    borderRadius: 8,
    color: "rgb(var(--fg))",
    fontSize: 12,
  } as const;

  return (
    <div>
      {/* 顶部 toolbar：堆叠 toggle + token legend(可点击隐藏) */}
      <div className="mb-2 flex flex-wrap items-center gap-x-4 gap-y-1">
        <label className="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted">
          <input
            type="checkbox"
            checked={stacked}
            onChange={(e) => setStacked(e.target.checked)}
            className="accent-accent"
          />
          {t("usage.stacked")}
        </label>
        <span className="text-2xs text-fg-muted">·</span>
        {visibleTokenLegend.map((item) => {
          const isHidden = hidden.has(item.key);
          return (
            <button
              key={item.key}
              type="button"
              onClick={() => toggleSeries(item.key)}
              className={`flex items-center gap-1.5 text-xs ${
                isHidden ? "text-fg-muted/40 line-through" : "text-fg-muted"
              }`}
              title={isHidden ? t("usage.series_show") : t("usage.series_hide")}
            >
              <span
                className="h-2 w-2 rounded-full"
                style={{
                  background: isHidden ? "transparent" : item.color,
                  border: isHidden ? `1px dashed ${item.color}` : "none",
                }}
              />
              {item.label}
            </button>
          );
        })}
      </div>

      {/* 两图布局：左 token 用量（line / 堆叠 area），右费用（composed: bar+line） */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* Token 用量 */}
        <div>
          <div className="mb-1 text-xs font-medium text-fg-muted">
            {t("usage.tokens_chart_title")}
          </div>
          <div className="h-64 w-full">
            <ResponsiveContainer width="100%" height="100%">
              {stacked ? (
                <AreaChart data={chartData}>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="rgb(var(--border) / var(--border-alpha))"
                  />
                  <XAxis
                    dataKey="date"
                    tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
                    stroke="rgb(var(--border) / var(--border-alpha))"
                    tickFormatter={tickFormatter}
                  />
                  <YAxis
                    tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
                    stroke="rgb(var(--border) / var(--border-alpha))"
                    tickFormatter={(v: number) => (v / 1000).toFixed(0) + "k"}
                  />
                  <Tooltip
                    formatter={(value) => Number(value).toLocaleString()}
                    contentStyle={tooltipContentStyle}
                    labelStyle={{ color: "rgb(var(--fg-muted))" }}
                    labelFormatter={(label) =>
                      isHourly && typeof label === "string" ? label.slice(-6) : label
                    }
                  />
                  {visibleTokenLegend
                    .map((it, idx) => (
                      <Area
                        key={it.key}
                        type="monotone"
                        dataKey={it.key as TokenKey}
                        stackId="tokens"
                        stroke={it.color}
                        fill={it.color}
                        fillOpacity={0.5 - idx * 0.08}
                        name={it.label}
                      />
                    ))}
                </AreaChart>
              ) : (
                <LineChart data={chartData}>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="rgb(var(--border) / var(--border-alpha))"
                  />
                  <XAxis
                    dataKey="date"
                    tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
                    stroke="rgb(var(--border) / var(--border-alpha))"
                    tickFormatter={tickFormatter}
                  />
                  <YAxis
                    tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
                    stroke="rgb(var(--border) / var(--border-alpha))"
                    tickFormatter={(v: number) => (v / 1000).toFixed(0) + "k"}
                  />
                  <Tooltip
                    formatter={(value) => Number(value).toLocaleString()}
                    contentStyle={tooltipContentStyle}
                    labelStyle={{ color: "rgb(var(--fg-muted))" }}
                    labelFormatter={(label) =>
                      isHourly && typeof label === "string" ? label.slice(-6) : label
                    }
                  />
                  {visibleTokenLegend
                    .map((it) => (
                      <Line
                        key={it.key}
                        type="monotone"
                        dataKey={it.key as TokenKey}
                        stroke={it.color}
                        name={it.label}
                        dot={dot}
                      />
                    ))}
                </LineChart>
              )}
            </ResponsiveContainer>
          </div>
        </div>

        {/* 费用 */}
        <div>
          <div className="mb-1 text-xs font-medium text-fg-muted">
            {t("usage.cost_chart_title")}
          </div>
          <div className="h-64 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <ComposedChart data={costChartData}>
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke="rgb(var(--border) / var(--border-alpha))"
                />
                <XAxis
                  dataKey="date"
                  tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
                  stroke="rgb(var(--border) / var(--border-alpha))"
                  tickFormatter={tickFormatter}
                />
                <YAxis
                  tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
                  stroke="rgb(var(--border) / var(--border-alpha))"
                  tickFormatter={(v: number) => "¥" + v.toFixed(2)}
                />
                <Tooltip
                  formatter={(value) => {
                    const n = typeof value === "number" ? value : parseFloat(String(value));
                    return "¥" + (Number.isFinite(n) ? n.toFixed(4) : "0");
                  }}
                  contentStyle={tooltipContentStyle}
                  labelStyle={{ color: "rgb(var(--fg-muted))" }}
                  labelFormatter={(label) =>
                    isHourly && typeof label === "string" ? label.slice(-6) : label
                  }
                />
                <Legend wrapperStyle={{ fontSize: 12, color: "rgb(var(--fg-muted))" }} />
                <Bar
                  dataKey="cumulativeCost"
                  fill={TOKEN_COLORS.cumulative}
                  name={t("usage.cumulative")}
                  radius={[2, 2, 0, 0]}
                />
                <Line
                  type="monotone"
                  dataKey="cost"
                  stroke={TOKEN_COLORS.cost}
                  strokeWidth={2}
                  name={t("usage.total_cost")}
                  dot={dot}
                />
              </ComposedChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>
    </div>
  );
}