/**
 * LogTable:日志表格行 + 表头 + 空态。子组件纯渲染,不持有逻辑。
 *
 * 用 React.memo 包装行,3s 自动刷新时只有新增/变化的行重渲。
 */
import React from "react";
import { useTranslation } from "react-i18next";
import type { LogEntry } from "@/lib/ipc";

const LEVEL_BADGE: Record<string, string> = {
  error: "badge-err",
  warn: "badge-warn",
  info: "badge-ok",
  debug: "badge-mute",
};

export const LogTable = React.memo(function LogTable({ logs }: { logs: LogEntry[] }) {
  const { t } = useTranslation();
  return (
    <div className="overflow-hidden rounded-lg border border-border">
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
  );
});

const LogRow = React.memo(function LogRow({ log: l }: { log: LogEntry }) {
  return (
    <tr className="border-t border-border hover:bg-overlay-soft">
      <td className="px-3 py-1.5 font-mono text-[11px] text-fg-muted whitespace-nowrap">
        {l.timestamp.slice(11, 19)}
      </td>
      <td className="px-3 py-1.5">
        <span className={LEVEL_BADGE[l.level] ?? "badge-mute"}>{l.level}</span>
      </td>
      <td className="px-3 py-1.5 font-mono text-xs">{l.source}</td>
      <td className="px-3 py-1.5">
        <div>{l.message}</div>
        {l.detail && (
          <pre className="mt-1 text-[11px] text-fg-muted whitespace-pre-wrap">{l.detail}</pre>
        )}
      </td>
    </tr>
  );
});
