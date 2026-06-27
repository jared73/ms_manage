import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Trash2, Download } from "lucide-react";
import { Section } from "@/components/Section";
import { listLogs, clearLogs } from "@/lib/ipc";
import type { LogEntry } from "@/lib/ipc";

export function Logs() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState<string>("");

  const load = async () => {
    const r = await listLogs({ limit: 500, level: filter || undefined });
    setLogs(r);
  };
  useEffect(() => { void load(); }, [filter]);

  const exportJson = () => {
    const blob = new Blob([JSON.stringify(logs, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `ms-manage-logs-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("logs.title")}
        action={
          <div className="flex items-center gap-2">
            <select className="input max-w-[140px]" value={filter} onChange={(e) => setFilter(e.target.value)}>
              <option value="">all</option>
              <option value="debug">debug</option>
              <option value="info">info</option>
              <option value="warn">warn</option>
              <option value="error">error</option>
            </select>
            <button className="btn-ghost" onClick={exportJson} disabled={logs.length === 0}>
              <Download size={14}/>导出 JSON
            </button>
            <button className="btn-ghost" onClick={async () => { await clearLogs(); await load(); }}>
              <Trash2 size={14}/>{t("logs.clear")}
            </button>
          </div>
        }
      >
        <div className="overflow-hidden rounded-lg border border-white/5">
          <div className="max-h-[75vh] overflow-auto">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-bg-subtle text-left text-xs uppercase text-fg-muted">
                <tr>
                  <th className="w-40 px-3 py-2 font-medium">{t("logs.time")}</th>
                  <th className="w-20 px-3 py-2 font-medium">{t("logs.level")}</th>
                  <th className="w-40 px-3 py-2 font-medium">{t("logs.source")}</th>
                  <th className="px-3 py-2 font-medium">{t("logs.message")}</th>
                </tr>
              </thead>
              <tbody>
                {logs.map((l) => (
                  <tr key={l.id} className="border-t border-white/5 hover:bg-white/[0.02]">
                    <td className="px-3 py-1.5 font-mono text-[11px] text-fg-muted whitespace-nowrap">{l.timestamp.slice(11, 19)}</td>
                    <td className="px-3 py-1.5">
                      <span className={
                        l.level === "error" ? "badge-err" :
                        l.level === "warn"  ? "badge-warn" :
                        l.level === "info"  ? "badge-ok" :
                        "badge-mute"
                      }>{l.level}</span>
                    </td>
                    <td className="px-3 py-1.5 font-mono text-xs">{l.source}</td>
                    <td className="px-3 py-1.5">
                      <div>{l.message}</div>
                      {l.detail && <pre className="mt-1 text-[11px] text-fg-muted whitespace-pre-wrap">{l.detail}</pre>}
                    </td>
                  </tr>
                ))}
                {logs.length === 0 && (
                  <tr><td colSpan={4} className="px-3 py-8 text-center text-fg-muted">{t("common.empty")}</td></tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      </Section>
    </div>
  );
}
