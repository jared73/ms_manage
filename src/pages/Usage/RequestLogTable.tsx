import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportRequestLogsCsv, getRequestLogs, writeCsvFile, type PaginatedLogs, type RequestLogRow } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { toast } from "sonner";
import { EmptyHint } from "@/components/Section";

function fmtLatency(ms: number): string {
  if (ms <= 0) return "-";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** 渲染层使用的行：后端 row + 预解析的时间字符串（避免每行 render 都 new Date+toLocaleString） */
interface DisplayRow extends RequestLogRow {
  displayTime: string;
}

/** 单行组件 memo：仅当 r 引用或 costFmt 变时才重渲染 —— 自动刷新下避免整页 N×M 次 Date 计算 */
const Row = memo(function Row({
  r,
  costFormatted,
}: {
  r: DisplayRow;
  costFormatted: string;
}) {
  const failed = r.statusCode !== 200 || !!r.errorMessage;
  return (
    <tr
      className={`border-b border-border ${failed ? "bg-red-500/5" : ""}`}
      title={r.errorMessage ?? undefined}
    >
      <td className="py-1.5 pr-3 text-xs text-fg-muted">{r.displayTime}</td>
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
      <td className="py-1.5 text-right text-xs">¥{costFormatted}</td>
    </tr>
  );
});

export function RequestLogTable({
  startDate,
  appFilter,
  modelFilter,
}: {
  startDate?: number;
  appFilter?: string;
  modelFilter?: string;
}) {
  const { t, i18n } = useTranslation();
  const costFmt = useMemo(
    () =>
      new Intl.NumberFormat(i18n.language, {
        minimumFractionDigits: 4,
        maximumFractionDigits: 4,
      }),
    [i18n.language],
  );
  const [data, setData] = useState<PaginatedLogs | null>(null);
  const [page, setPage] = useState(1);
  const [exporting, setExporting] = useState(false);
  const pageSize = 20;

  // 预解析 createdAt → displayTime（避免每行 render 都 new Date+toLocaleString）
  const visibleRows = useMemo<DisplayRow[]>(() => {
    const rows = data?.rows ?? [];
    return rows.map((r) => ({
      ...r,
      displayTime: new Date(r.createdAt * 1000).toLocaleString(),
    }));
  }, [data]);

  const requestSeqRef = useRef(0);

  const load = useCallback(async () => {
    const reqId = ++requestSeqRef.current;
    try {
      const result = await getRequestLogs({
        startDate,
        // 空串 → undefined,避免后端 SQL 拼 "app_type = ''" 永远不匹配 (#17 回归 bug)
        appType: appFilter || undefined,
        model: modelFilter || undefined,
        page,
        pageSize,
      });
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
  }, [startDate, appFilter, modelFilter, page]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    setPage(1);
  }, [startDate, appFilter, modelFilter]);

  const doExport = async () => {
    setExporting(true);
    try {
      // 1. 弹原生 save dialog 让用户选保存位置
      const today = new Date().toISOString().slice(0, 10);
      const target = await save({
        title: t("usage.export_csv"),
        defaultPath: `usage-request-logs-${today}.csv`,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (!target) return; // 用户取消
      // 2. 拉 CSV 内容
      const csv = await exportRequestLogsCsv(
        startDate,
        undefined,
        appFilter || undefined,
        modelFilter || undefined,
      );
      // 3. 让 Rust 端写文件(避免前端 tauri-plugin-fs 的 scope 限制)
      await writeCsvFile(target, "\uFEFF" + csv);
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
      <div className="mb-2 flex items-center justify-end">
        <button
          onClick={doExport}
          disabled={exporting || !data || data.total === 0}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-overlay-soft disabled:opacity-40"
        >
          <Download size={13} />
          {t("usage.export_csv")}
        </button>
      </div>
      {!data || visibleRows.length === 0 ? (
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
            {visibleRows.map((r) => (
              <Row key={r.requestId} r={r} costFormatted={costFmt.format(Number(r.totalCostUsd))} />
            ))}
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
