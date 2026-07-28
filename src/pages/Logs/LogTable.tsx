/**
 * LogTable:日志表格行 + 表头 + 空态。子组件纯渲染,不持有逻辑。
 *
 * 用 React.memo 包装行,3s 自动刷新时只有新增/变化的行重渲。
 * - 表头点击切换时间排序(升/降)
 * - 行点击展开 detail
 */
import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronUp, ChevronDown, ChevronRight } from "lucide-react";
import type { LogEntry } from "@/lib/ipc";

const LEVEL_BADGE: Record<string, string> = {
  error: "badge-err",
  warn: "badge-warn",
  info: "badge-ok",
  debug: "badge-mute",
};

interface LogTableProps {
  logs: LogEntry[];
  sortOrder: "asc" | "desc";
  onToggleSort: () => void;
}

export const LogTable = React.memo(function LogTable({ logs, sortOrder, onToggleSort }: LogTableProps) {
  const { t } = useTranslation();
  return (
    <div className="overflow-x-auto">
      <div className="overflow-hidden rounded-lg border border-border">
      <div className="max-h-[75vh] overflow-auto">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-bg-subtle text-left text-xs uppercase text-fg-muted">
            <tr>
              <th scope="col" className="w-40 px-3 py-2 font-medium cursor-pointer select-none hover:text-fg" onClick={onToggleSort}>
                <span className="inline-flex items-center gap-1">
                  {t("logs.time")}
                  {sortOrder === "desc" ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
                </span>
              </th>
              <th scope="col" className="w-20 px-3 py-2 font-medium">{t("logs.level")}</th>
              <th scope="col" className="w-40 px-3 py-2 font-medium">{t("logs.source")}</th>
              <th scope="col" className="px-3 py-2 font-medium">{t("logs.message")}</th>
            </tr>
          </thead>
          <tbody>
            {logs.map((l) => (
              <LogRow key={l.id} log={l} />
            ))}
            {logs.length === 0 && (
              <tr>
                <td colSpan={4} className="px-3 py-8 text-center text-fg-muted">
                  {t("common.empty")}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      </div>
    </div>
  );
});

const LogRow = React.memo(function LogRow({ log: l }: { log: LogEntry }) {
  const [expanded, setExpanded] = useState(false);
  const hasDetail = !!l.detail;

  return (
    <>
      <tr
        className={`border-t border-border transition-colors ${hasDetail ? "cursor-pointer hover:bg-overlay-soft" : "hover:bg-overlay-soft"}`}
        onClick={() => hasDetail && setExpanded((v) => !v)}
        title={hasDetail ? "Click to expand" : undefined}
      >
        <td className="px-3 py-1.5 font-mono text-2xs text-fg-muted whitespace-nowrap">
          {l.timestamp.slice(11, 19)}
        </td>
        <td className="px-3 py-1.5">
          <span className={LEVEL_BADGE[l.level] ?? "badge-mute"}>{l.level}</span>
        </td>
        <td className="max-w-[10rem] truncate px-3 py-1.5 font-mono text-xs" title={l.source}>{l.source}</td>
        <td className="px-3 py-1.5">
          <div className="flex items-center gap-1">
            {hasDetail && (
              <ChevronRight size={12} className={`shrink-0 text-fg-muted transition-transform ${expanded ? "rotate-90" : ""}`} />
            )}
            <span className="truncate">{l.message}</span>
          </div>
        </td>
      </tr>
      {expanded && hasDetail && (
        <tr className="border-t border-border bg-bg-subtle">
          <td colSpan={4} className="px-6 py-2">
            <pre className="text-2xs text-fg-muted whitespace-pre-wrap break-all">{l.detail}</pre>
          </td>
        </tr>
      )}
    </>
  );
});
