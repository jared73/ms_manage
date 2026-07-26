/**
 * Logs 主页面:筛选 + 自动刷新 + 导出 + 清空 + 表格。
 *
 * 设计:
 * - 数据/副作用用 ./useLogs.ts,纯渲染表格用 ./LogTable.tsx
 * - 页面只承担"装订"职责
 *
 * 错误/加载状态:
 * - 首屏拉取未完成 → 显示骨架 + 标题旁的"Loading"标记
 * - 拉取失败 → 在表格区显示红条 + 重试按钮(loadError)
 * - 空数据 → 表格显示 "暂无日志" 提示(由 LogTable 渲染)
 */
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Trash2, Download, AlertTriangle, Loader2, Search } from "lucide-react";
import { Section } from "@/components/Section";
import { ConfirmModal } from "@/components/ConfirmModal";
import { useLogs } from "./useLogs";
import { LogTable } from "./LogTable";

export function Logs() {
  const { t } = useTranslation();
  const lg = useLogs();

  // 自由文本搜索:过滤 message + source,大小写不敏感。
  // 与 level/source filter 是 AND 关系(先 severity 过滤再文本过滤)。
  const visibleLogs = useMemo(() => {
    if (!lg.search.trim()) return lg.logs;
    const q = lg.search.toLowerCase();
    return lg.logs.filter(
      (l) => l.message.toLowerCase().includes(q) || l.source.toLowerCase().includes(q),
    );
  }, [lg.logs, lg.search]);

  const exportJson = () => {
    const blob = new Blob([JSON.stringify(lg.logs, null, 2)], { type: "application/json" });
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
          <div className="flex flex-wrap items-center gap-2">
            {lg.loading && (
              <span className="inline-flex items-center gap-1 text-xs text-fg-muted">
                <Loader2 size={12} className="animate-spin" />
                {t("logs.loading")}
              </span>
            )}
            <select
              className="input w-auto"
              value={lg.level}
              onChange={(e) => lg.setLevel(e.target.value)}
              title={t("logs.filter_level_tooltip") as string}
            >
              <option value="">{t("logs.level_all")}</option>
              <option value="debug">{t("logs.level_debug")}</option>
              <option value="info">{t("logs.level_info")}</option>
              <option value="warn">{t("logs.level_warn")}</option>
              <option value="error">{t("logs.level_error")}</option>
            </select>
            <input
              type="search"
              className="input flex-1 min-w-[140px] max-w-[260px]"
              placeholder={t("logs.search_placeholder") as string}
              value={lg.search}
              onChange={(e) => lg.setSearch(e.target.value)}
              aria-label={t("logs.search_label") as string}
            />
            <select
              className="input w-auto"
              value={lg.sourceFilter}
              onChange={(e) => lg.setSourceFilter(e.target.value)}
              title={t("logs.filter_source_tooltip") as string}
            >
              <option value="">{t("logs.source_all")}</option>
              {lg.sources.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
            <label
              className="flex shrink-0 cursor-pointer items-center gap-1.5 text-xs text-fg-muted"
              title={t("logs.auto_refresh_tooltip") as string}
            >
              <input
                type="checkbox"
                checked={lg.autoRefresh}
                onChange={(e) => lg.setAutoRefresh(e.target.checked)}
                className="accent-accent"
                aria-label={t("logs.auto_refresh") as string}
              />
              {t("logs.auto_refresh")}
            </label>
            <button className="btn-ghost shrink-0" onClick={exportJson} disabled={lg.logs.length === 0}>
              <Download size={14} />
              {t("logs.export_json")}
            </button>
            <button className="btn-ghost shrink-0" onClick={() => lg.setConfirmClear(true)}>
              <Trash2 size={14} />
              {t("logs.clear")}
            </button>
          </div>
        }
      >
        {/* 拉取失败:显示错误条 + 重试按钮 */}
        {lg.loadError && (
          <div
            role="alert"
            className="mb-3 flex items-start gap-2 rounded-md border border-err/40 bg-err/5 p-3 text-xs"
          >
            <AlertTriangle size={14} className="mt-0.5 shrink-0 text-err" />
            <div className="flex-1">
              <div className="font-medium text-err">{t("logs.load_error_title")}</div>
              <div className="mt-0.5 text-fg-muted break-all">{lg.loadError}</div>
            </div>
            <button className="btn-primary text-xs" onClick={() => void lg.reload()}>
              {t("logs.retry")}
            </button>
          </div>
        )}

        {/* 首屏 loading:显示骨架,避免空表 / 闪烁 */}
        {lg.loading && !lg.loadError && lg.logs.length === 0 ? (
          <div className="space-y-1.5">
            {Array.from({ length: 6 }).map((_, i) => (
              <div
                key={i}
                className="h-7 animate-pulse rounded bg-bg-subtle"
                style={{ width: `${60 + (i * 7) % 35}%` }}
              />
            ))}
          </div>
        ) : visibleLogs.length === 0 && lg.search.trim() ? (
          <div className="rounded-md border border-border bg-bg-subtle p-6 text-center text-sm text-fg-muted">
            <Search size={18} className="mx-auto mb-2 opacity-50" />
            {t("logs.no_match", { q: lg.search })}
          </div>
        ) : (
          <LogTable logs={visibleLogs} />
        )}
      </Section>

      {/* 清空二次确认(防止误点导致所有日志瞬间消失) */}
      <ConfirmModal
        open={lg.confirmClear}
        title={t("common.confirm")}
        confirmLabel={t("logs.clear")}
        onClose={() => lg.setConfirmClear(false)}
        onConfirm={async () => { await lg.clear(); }}
      >
        <p className="text-sm">{t("logs.clear_confirm")}</p>
      </ConfirmModal>
    </div>
  );
}
