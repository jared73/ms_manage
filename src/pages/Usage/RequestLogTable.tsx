import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import { exportRequestLogsCsv, getRequestLogs, type PaginatedLogs } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { toast } from "sonner";
import { EmptyHint } from "@/components/Section";

function fmtLatency(ms: number): string {
  if (ms <= 0) return "-";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export function RequestLogTable({ startDate }: { startDate?: number }) {
  const { t } = useTranslation();
  const [data, setData] = useState<PaginatedLogs | null>(null);
  const [page, setPage] = useState(1);
  const [exporting, setExporting] = useState(false);
  const pageSize = 20;

  const requestSeqRef = useRef(0);

  const load = useCallback(async () => {
    const reqId = ++requestSeqRef.current;
    try {
      const result = await getRequestLogs({ startDate, page, pageSize });
      if (reqId !== requestSeqRef.current) return;
      setData(result);
      // 新数据返回后 clamp 页码，避免超出实际页数
      const totalPages = Math.max(1, Math.ceil(result.total / pageSize));
      if (page > totalPages) {
        setPage(totalPages);
      }
    } catch (e) {
      if (reqId !== requestSeqRef.current) return;
      toast.error(describeError(e));
    }
  }, [startDate, page]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    setPage(1);
  }, [startDate]);

  const doExport = async () => {
    setExporting(true);
    try {
      const csv = await exportRequestLogsCsv(startDate);
      const blob = new Blob(["\uFEFF" + csv], { type: "text/csv;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `usage-request-logs-${new Date().toISOString().slice(0, 10)}.csv`;
      a.click();
      URL.revokeObjectURL(url);
      toast.success(t("usage.csv_exported"));
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setExporting(false);
    }
  };

  const totalPages = data ? Math.max(1, Math.ceil(data.total / pageSize)) : 1;

  return (
    <div>
      <div className="mb-2 flex justify-end">
        <button
          onClick={doExport}
          disabled={exporting || !data || data.total === 0}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-overlay-soft disabled:opacity-40"
        >
          <Download size={13} />
          {t("usage.export_csv")}
        </button>
      </div>
      {!data || data.rows.length === 0 ? (
        <EmptyHint>{t("usage.no_logs")}</EmptyHint>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left text-2xs text-fg-muted">
              <th className="py-2 pr-3">{t("usage.time")}</th>
              <th className="py-2 pr-3">{t("usage.app")}</th>
              <th className="py-2 pr-3">{t("usage.model")}</th>
              <th className="py-2 pr-3 text-right">{t("usage.input_tokens")}</th>
              <th className="py-2 pr-3 text-right">{t("usage.output_tokens")}</th>
              <th className="py-2 pr-3 text-right">{t("usage.latency")}</th>
              <th className="py-2 pr-3 text-right">{t("usage.status")}</th>
              <th className="py-2 text-right">{t("usage.total_cost")}</th>
            </tr>
          </thead>
          <tbody>
            {data?.rows.map((r) => {
              const failed = r.statusCode !== 200 || !!r.errorMessage;
              return (
                <tr
                  key={r.requestId}
                  className={`border-b border-border ${failed ? "bg-red-500/5" : ""}`}
                  title={r.errorMessage ?? undefined}
                >
                  <td className="py-1.5 pr-3 text-xs text-fg-muted">
                    {new Date(r.createdAt * 1000).toLocaleString()}
                  </td>
                  <td className="py-1.5 pr-3 text-xs">{r.appType}</td>
                  <td className="py-1.5 pr-3 font-mono text-xs">{r.model}</td>
                  <td className="py-1.5 pr-3 text-right text-xs">{r.inputTokens.toLocaleString()}</td>
                  <td className="py-1.5 pr-3 text-right text-xs">{r.outputTokens.toLocaleString()}</td>
                  <td className="py-1.5 pr-3 text-right text-xs">{fmtLatency(r.latencyMs)}</td>
                  <td className="py-1.5 pr-3 text-right text-xs">
                    <span className={failed ? "font-medium text-red-500" : "text-fg-muted"}>
                      {r.statusCode !== 200 ? `HTTP ${r.statusCode}` : r.errorMessage ? "ERR" : "OK"}
                    </span>
                  </td>
                  <td className="py-1.5 text-right text-xs">¥{Number(r.totalCostUsd).toFixed(4)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      {data && data.total > 0 && (
        <div className="mt-3 flex items-center justify-end gap-2 text-xs">
          <button
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
            className="rounded-lg border border-border px-2.5 py-1 disabled:opacity-40"
          >
            {t("usage.prev")}
          </button>
          <span className="text-fg-muted">
            {page} / {totalPages}
          </span>
          <button
            disabled={page >= totalPages}
            onClick={() => setPage((p) => p + 1)}
            className="rounded-lg border border-border px-2.5 py-1 disabled:opacity-40"
          >
            {t("usage.next")}
          </button>
        </div>
      )}
    </div>
  );
}
