import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getRequestLogs, type PaginatedLogs } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { toast } from "sonner";

export function RequestLogTable({ startDate }: { startDate?: number }) {
  const { t } = useTranslation();
  const [data, setData] = useState<PaginatedLogs | null>(null);
  const [page, setPage] = useState(1);
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

  const totalPages = data ? Math.max(1, Math.ceil(data.total / pageSize)) : 1;

  return (
    <div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-2xs text-fg-muted">
            <th className="py-2 pr-3">{t("usage.time")}</th>
            <th className="py-2 pr-3">{t("usage.app")}</th>
            <th className="py-2 pr-3">{t("usage.model")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.input_tokens")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.output_tokens")}</th>
            <th className="py-2 text-right">{t("usage.total_cost")}</th>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((r) => (
            <tr key={r.requestId} className="border-b border-border">
              <td className="py-1.5 pr-3 text-xs text-fg-muted">
                {new Date(r.createdAt * 1000).toLocaleString()}
              </td>
              <td className="py-1.5 pr-3 text-xs">{r.appType}</td>
              <td className="py-1.5 pr-3 font-mono text-xs">{r.model}</td>
              <td className="py-1.5 pr-3 text-right text-xs">{r.inputTokens.toLocaleString()}</td>
              <td className="py-1.5 pr-3 text-right text-xs">{r.outputTokens.toLocaleString()}</td>
              <td className="py-1.5 text-right text-xs">${Number(r.totalCostUsd).toFixed(4)}</td>
            </tr>
          ))}
        </tbody>
      </table>
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
