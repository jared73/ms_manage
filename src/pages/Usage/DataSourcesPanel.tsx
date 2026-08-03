import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { getUsageDataSources, syncSessionUsage, type DataSourceStatus, type SessionSyncResult } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function DataSourcesPanel({ onSynced }: { onSynced?: () => void }) {
  const { t } = useTranslation();
  const [sources, setSources] = useState<DataSourceStatus[]>([]);
  const [syncing, setSyncing] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setSources(await getUsageDataSources());
    } catch (e) {
      toast.error(describeError(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const doSync = async () => {
    setSyncing(true);
    try {
      const r: SessionSyncResult = await syncSessionUsage();
      toast.success(t("usage.sync_done", { imported: r.imported, skipped: r.skipped }));
      await refresh();
      onSynced?.();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setSyncing(false);
    }
  };

  return (
    <div className="rounded-xl border border-border bg-bg-subtle p-4">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold">{t("usage.data_sources")}</h3>
        <button
          onClick={doSync}
          disabled={syncing}
          className="flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
        >
          <RefreshCw size={13} className={syncing ? "animate-spin" : ""} />
          {t("usage.sync_now")}
        </button>
      </div>
      <ul className="space-y-2">
        {sources.map((s) => (
          <li key={s.appType} className="flex items-center gap-2 text-xs">
            <span className={`h-2 w-2 rounded-full ${s.exists ? "bg-emerald-500" : "bg-gray-400"}`} />
            <span className="w-24 shrink-0 font-medium">{s.label}</span>
            <code className="truncate font-mono text-fg-muted">{s.path}</code>
          </li>
        ))}
      </ul>
    </div>
  );
}
