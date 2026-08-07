import { useTranslation } from "react-i18next";
import type { ProviderStats } from "@/lib/ipc";

const APP_LABELS: Record<string, string> = {
  opencode: "OpenCode",
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini",
  grokbuild: "Grok Build",
};

export function ProviderStatsTable({ data }: { data: ProviderStats[] }) {
  const { t } = useTranslation();
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-border text-left text-2xs text-fg-muted">
          <th className="py-2 pr-3">{t("usage.app")}</th>
          <th className="py-2 pr-3">{t("usage.total_requests")}</th>
          <th className="py-2 pr-3">{t("usage.total_tokens")}</th>
          <th className="py-2 pr-3">{t("usage.input_tokens_short")}</th>
          <th className="py-2 pr-3">{t("usage.output_tokens_short")}</th>
          <th className="py-2 pr-3">{t("usage.cache_read_short")}</th>
          <th className="py-2 pr-3">{t("usage.cache_creation_short")}</th>
          <th className="py-2 text-right">{t("usage.total_cost")}</th>
        </tr>
      </thead>
      <tbody>
        {data.map((p) => (
          <tr key={p.appType} className="border-b border-border">
            <td className="py-2 pr-3 font-mono text-xs">
              {APP_LABELS[p.appType] ?? p.appType}
            </td>
            <td className="py-2 pr-3">{p.requestCount.toLocaleString()}</td>
            <td className="py-2 pr-3">{p.totalTokens.toLocaleString()}</td>
            <td className="py-2 pr-3">{p.totalInputTokens.toLocaleString()}</td>
            <td className="py-2 pr-3">{p.totalOutputTokens.toLocaleString()}</td>
            <td className="py-2 pr-3">{p.totalCacheReadTokens.toLocaleString()}</td>
            <td className="py-2 pr-3">{p.totalCacheCreationTokens.toLocaleString()}</td>
            <td className="py-2 text-right">¥{Number(p.totalCost).toFixed(4)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
