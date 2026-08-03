import { useTranslation } from "react-i18next";
import type { ModelStats } from "@/lib/ipc";

export function ModelStatsTable({ data }: { data: ModelStats[] }) {
  const { t } = useTranslation();
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-border text-left text-2xs text-fg-muted">
          <th className="py-2 pr-3">{t("usage.model")}</th>
          <th className="py-2 pr-3">{t("usage.total_requests")}</th>
          <th className="py-2 pr-3">{t("usage.total_tokens")}</th>
          <th className="py-2 text-right">{t("usage.total_cost")}</th>
        </tr>
      </thead>
      <tbody>
        {data.map((m) => (
          <tr key={m.model} className="border-b border-border">
            <td className="py-2 pr-3 font-mono text-xs">{m.model}</td>
            <td className="py-2 pr-3">{m.requestCount.toLocaleString()}</td>
            <td className="py-2 pr-3">{m.totalTokens.toLocaleString()}</td>
            <td className="py-2 text-right">${Number(m.totalCost).toFixed(4)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
