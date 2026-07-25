/**
 * Logs 主页面:筛选 + 自动刷新 + 导出 + 清空 + 表格。
 *
 * 设计:
 * - 数据/副作用用 ./useLogs.ts,纯渲染表格用 ./LogTable.tsx
 * - 页面只承担"装订"职责
 */
import { useTranslation } from "react-i18next";
import { Trash2, Download } from "lucide-react";
import { Section } from "@/components/Section";
import { useLogs } from "./useLogs";
import { LogTable } from "./LogTable";

export function Logs() {
  const { t } = useTranslation();
  const lg = useLogs();

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
          <div className="flex items-center gap-2">
            <select
              className="input max-w-[120px]"
              value={lg.level}
              onChange={(e) => lg.setLevel(e.target.value)}
              title={t("logs.filter_level_tooltip") as string}
            >
              <option value="">{t("logs.level_all")}</option>
              <option value="debug">debug</option>
              <option value="info">info</option>
              <option value="warn">warn</option>
              <option value="error">error</option>
            </select>
            <select
              className="input max-w-[160px]"
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
              className="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted"
              title={t("logs.auto_refresh_tooltip") as string}
            >
              <input
                type="checkbox"
                checked={lg.autoRefresh}
                onChange={(e) => lg.setAutoRefresh(e.target.checked)}
                className="accent-accent"
              />
              {t("logs.auto_refresh")}
            </label>
            <button className="btn-ghost" onClick={exportJson} disabled={lg.logs.length === 0}>
              <Download size={14} />
              {t("logs.export_json")}
            </button>
            <button className="btn-ghost" onClick={() => void lg.clear()}>
              <Trash2 size={14} />
              {t("logs.clear")}
            </button>
          </div>
        }
      >
        <LogTable logs={lg.logs} />
      </Section>
    </div>
  );
}
