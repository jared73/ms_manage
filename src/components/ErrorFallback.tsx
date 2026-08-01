/**
 * ErrorBoundary 的降级 UI 子组件。
 * 拆到这里是为了满足 react-refresh 的"单文件只 export 组件"约束,
 * 同时让 ErrorFallback 自身能作为单独单元测试 / Storybook 渲染。
 */
import { useTranslation } from "react-i18next";
import { AlertTriangle, RefreshCw } from "lucide-react";

export function ErrorFallback({
  label,
  message,
  onRetry,
  onHardReload,
}: {
  label?: string;
  message: string;
  onRetry: () => void;
  onHardReload: () => void;
}) {
  const { t } = useTranslation();
  const displayLabel = label ?? t("error_boundary.default_label");
  return (
    <div className="grid min-h-[60vh] place-items-center p-6">
      <div className="max-w-md rounded-lg border border-err/40 bg-err/5 p-6 text-sm">
        <div className="mb-3 flex items-center gap-2 text-err">
          <AlertTriangle size={18} />
          <h2 className="text-base font-semibold">{t("error_boundary.title", { label: displayLabel })}</h2>
        </div>
        <p className="mb-3 text-fg-muted">{t("error_boundary.desc")}</p>
        {/* 只显示简短的 message,完整 stack 已写入日志 */}
        <pre className="mb-4 max-h-32 overflow-auto rounded bg-bg-subtle p-3 text-xs text-fg-muted whitespace-pre-wrap break-all">
          {message}
        </pre>
        <div className="flex gap-2">
          <button className="btn-primary" onClick={onRetry}>
            <RefreshCw size={14} /> {t("error_boundary.retry")}
          </button>
          <button className="btn-ghost" onClick={onHardReload}>
            {t("error_boundary.hard_reload")}
          </button>
        </div>
      </div>
    </div>
  );
}