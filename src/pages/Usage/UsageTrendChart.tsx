import { useTranslation } from "react-i18next";
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { DailyStats } from "@/lib/ipc";

export function UsageTrendChart({ data }: { data: DailyStats[] }) {
  const { t } = useTranslation();
  const chartData = data.map((d) => ({
    date: d.date,
    tokens: d.totalTokens,
    cost: Number(d.totalCost),
  }));

  return (
    <div className="h-64 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={chartData}>
          <CartesianGrid strokeDasharray="3 3" stroke="rgb(var(--border) / var(--border-alpha))" />
          <XAxis
            dataKey="date"
            tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
            stroke="rgb(var(--border) / var(--border-alpha))"
          />
          <YAxis
            yAxisId="left"
            tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
            stroke="rgb(var(--border) / var(--border-alpha))"
            tickFormatter={(v) => (v / 1000).toFixed(0) + "k"}
          />
          <YAxis
            yAxisId="right"
            orientation="right"
            tick={{ fontSize: 12, fill: "rgb(var(--fg-muted))" }}
            stroke="rgb(var(--border) / var(--border-alpha))"
            tickFormatter={(v) => "$" + v.toFixed(2)}
          />
          <Tooltip
            contentStyle={{
              background: "rgb(var(--bg-elev))",
              border: "1px solid rgb(var(--border) / var(--border-alpha))",
              borderRadius: 8,
              color: "rgb(var(--fg))",
              fontSize: 12,
            }}
            labelStyle={{ color: "rgb(var(--fg-muted))" }}
          />
          <Line yAxisId="left" type="monotone" dataKey="tokens" stroke="rgb(var(--accent))" name={t("usage.total_tokens")} dot={false} />
          <Line yAxisId="right" type="monotone" dataKey="cost" stroke="rgb(var(--warn))" name={t("usage.total_cost")} dot={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
