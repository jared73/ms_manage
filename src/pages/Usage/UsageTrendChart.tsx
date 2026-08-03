import { useTranslation } from "react-i18next";
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { DailyStats } from "@/lib/ipc";

const TOKEN_COLORS = {
  input: "rgb(var(--accent))",
  output: "rgb(var(--warn))",
  cacheRead: "#10b981",
  cacheCreation: "#8b5cf6",
  cost: "#f59e0b",
} as const;

export function UsageTrendChart({
  data,
  grouping = "day",
}: {
  data: DailyStats[];
  grouping?: "day" | "hour";
}) {
  const { t } = useTranslation();
  const isHourly = grouping === "hour";

  const chartData = data.map((d) => ({
    date: d.date,
    input: d.totalInputTokens,
    output: d.totalOutputTokens,
    cacheRead: d.totalCacheReadTokens,
    cacheCreation: d.totalCacheCreationTokens,
    cost: Number(d.totalCost),
  }));

  const legendItems = [
    { key: "input", label: t("usage.input_tokens_short"), color: TOKEN_COLORS.input },
    { key: "output", label: t("usage.output_tokens_short"), color: TOKEN_COLORS.output },
    { key: "cacheRead", label: t("usage.cache_read_short"), color: TOKEN_COLORS.cacheRead },
    { key: "cacheCreation", label: t("usage.cache_creation_short"), color: TOKEN_COLORS.cacheCreation },
    { key: "cost", label: t("usage.total_cost"), color: TOKEN_COLORS.cost },
  ];

  const tickFormatter = (value: string) => (isHourly ? value.slice(-6) : value);
  const dot = isHourly ? false : { r: 2, strokeWidth: 0 };

  return (
    <div>
      <div className="mb-2 flex flex-wrap items-center gap-x-4 gap-y-1">
        {legendItems.map((item) => (
          <span key={item.key} className="flex items-center gap-1.5 text-xs text-fg-muted">
            <span className="h-2 w-2 rounded-full" style={{ background: item.color }} />
            {item.label}
          </span>
        ))}
      </div>
      <div className="h-64 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="rgb(var(--border) / var(--border-alpha))" />
            <XAxis
              dataKey="date"
              tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
              stroke="rgb(var(--border) / var(--border-alpha))"
              tickFormatter={tickFormatter}
            />
            <YAxis
              yAxisId="left"
              tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
              stroke="rgb(var(--border) / var(--border-alpha))"
              tickFormatter={(v: number) => (v / 1000).toFixed(0) + "k"}
            />
            <YAxis
              yAxisId="right"
              orientation="right"
              tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
              stroke="rgb(var(--border) / var(--border-alpha))"
              tickFormatter={(v: number) => "$" + v.toFixed(2)}
            />
            <Tooltip
              formatter={(value) => Number(value).toLocaleString()}
              contentStyle={{
                background: "rgb(var(--bg-elev))",
                border: "1px solid rgb(var(--border) / var(--border-alpha))",
                borderRadius: 8,
                color: "rgb(var(--fg))",
                fontSize: 12,
              }}
              labelStyle={{ color: "rgb(var(--fg-muted))" }}
              labelFormatter={(label) => (isHourly && typeof label === "string" ? label.slice(-6) : label)}
            />
            <Line yAxisId="left" type="monotone" dataKey="input" stroke={TOKEN_COLORS.input} name={t("usage.input_tokens_short")} dot={dot} />
            <Line yAxisId="left" type="monotone" dataKey="output" stroke={TOKEN_COLORS.output} name={t("usage.output_tokens_short")} dot={dot} />
            <Line yAxisId="left" type="monotone" dataKey="cacheRead" stroke={TOKEN_COLORS.cacheRead} name={t("usage.cache_read_short")} dot={dot} />
            <Line yAxisId="left" type="monotone" dataKey="cacheCreation" stroke={TOKEN_COLORS.cacheCreation} name={t("usage.cache_creation_short")} dot={dot} />
            <Line yAxisId="right" type="monotone" dataKey="cost" stroke={TOKEN_COLORS.cost} name={t("usage.total_cost")} dot={dot} />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
