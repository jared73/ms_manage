import { useTranslation } from "react-i18next";
import { RefreshCw, FolderOpen, ChevronDown, AlertCircle, RotateCcw, MoreVertical } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { getUsageDataSources, openPath, resetSyncState, syncSessionUsage, type DataSourceStatus, type SessionSyncResult } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { ConfirmModal } from "@/components/ConfirmModal";

function fmtTime(ts?: number): string {
  if (!ts) return "-";
  const d = new Date(ts * 1000);
  const now = Date.now();
  const diff = now - d.getTime();
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return d.toLocaleString();
}

export function DataSourcesPanel({ onSynced }: { onSynced?: () => void }) {
  const { t } = useTranslation();
  const [sources, setSources] = useState<DataSourceStatus[]>([]);
  const [syncing, setSyncing] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [confirmResetOpen, setConfirmResetOpen] = useState(false);
  // 上次同步的结果（含 errors），用户可展开查看详情
  const [lastResult, setLastResult] = useState<SessionSyncResult | null>(null);
  const [errorsOpen, setErrorsOpen] = useState(false);
  // 三点菜单开合 + 点击外部关闭
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!menuOpen) return;
    const onDocClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [menuOpen]);

  // 数据源列表刷新:挂载时 + doSync 完成后 + onSynced 触发(父组件会 reload 整个 Usage 页并传 onSynced)
  // (定时刷新已删除 —— 与 Usage 页 toolbar 的「自动刷新」功能重叠,统一由父级控制)
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
      setLastResult(r);
      toast.success(t("usage.sync_done", { imported: r.imported, skipped: r.skipped }));
      await refresh();
      onSynced?.();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setSyncing(false);
    }
  };

  const openDir = async (path: string) => {
    try {
      await openPath(path);
    } catch (e) {
      toast.error(describeError(e));
    }
  };

  const doReset = async () => {
    setResetting(true);
    try {
      const n = await resetSyncState();
      toast.success(t("usage.sync_reset") + `: ${n}`);
      setConfirmResetOpen(false);
      await refresh();
      onSynced?.();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setResetting(false);
    }
  };

  return (
    <div className="rounded-xl border border-border bg-bg-subtle p-4">
      {/* 标题 + toolbar 分行:窗口小时 "数据源" 标题不会被控件挤掉,控件自己 flex-wrap 换行 */}
      <h3 className="mb-2 text-sm font-semibold">{t("usage.data_sources")}</h3>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <button
          onClick={doSync}
          disabled={syncing}
          className="flex shrink-0 items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
        >
          <RefreshCw size={13} className={syncing ? "animate-spin" : ""} />
          {t("usage.sync_now")}
        </button>
        {/* 三点菜单：重置同步等高级操作收纳在这里,避免 toolbar 拥挤 */}
        <div ref={menuRef} className="relative">
          <button
            onClick={() => setMenuOpen((o) => !o)}
            className="flex shrink-0 items-center justify-center rounded-lg border border-border p-1.5 text-fg-muted hover:bg-overlay-soft hover:text-fg"
            aria-label={t("usage.sync_more_actions") as string}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
          >
            <MoreVertical size={13} />
          </button>
          {menuOpen && (
            <div
              role="menu"
              className="absolute right-0 top-full z-10 mt-1 min-w-[180px] rounded-lg border border-border bg-bg-elev p-1 shadow-lg"
            >
              <button
                role="menuitem"
                onClick={() => {
                  setMenuOpen(false);
                  setConfirmResetOpen(true);
                }}
                disabled={syncing || resetting}
                className="flex w-full items-center gap-2 rounded-md px-3 py-1.5 text-left text-xs text-fg hover:bg-overlay-soft disabled:opacity-50"
              >
                <RotateCcw size={12} />
                {t("usage.sync_reset")}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* 重置同步水位线二次确认 */}
      <ConfirmModal
        open={confirmResetOpen}
        title={t("usage.sync_reset")}
        busy={resetting}
        onConfirm={doReset}
        onClose={() => !resetting && setConfirmResetOpen(false)}
      >
        <p className="text-sm">{t("usage.sync_reset_confirm")}</p>
      </ConfirmModal>

      {/* 上次同步错误详情（可折叠） */}
      {lastResult && lastResult.errors.length > 0 && (
        <div className="mb-3 rounded-lg border border-err/30 bg-err/5">
          <button
            onClick={() => setErrorsOpen((o) => !o)}
            className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-err"
          >
            <AlertCircle size={13} />
            <span>{t("usage.sync_errors", { count: lastResult.errors.length })}</span>
            <ChevronDown
              size={12}
              className={`ml-auto shrink-0 transition-transform ${errorsOpen ? "rotate-0" : "-rotate-90"}`}
            />
          </button>
          {errorsOpen && (
            <ul className="max-h-48 overflow-y-auto border-t border-err/20 px-3 py-2 text-2xs">
              {lastResult.errors.map((err, i) => (
                <li key={i} className="break-all py-0.5 font-mono text-err/90">{err}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      <ul className="space-y-2">
        {sources.map((s) => (
          <li key={s.appType} className="flex items-center gap-2 text-xs">
            <span className={`h-2 w-2 shrink-0 rounded-full ${s.exists ? "bg-emerald-500" : "bg-gray-400"}`} />
            <span className="w-24 shrink-0 font-medium">{s.label}</span>
            <span className="shrink-0 text-fg-muted">
              {s.importedCount.toLocaleString()} 条
            </span>
            <span className="shrink-0 text-fg-muted/70">{fmtTime(s.lastSyncedAt)}</span>
            <code className="truncate font-mono text-fg-muted/60">{s.path}</code>
            <button
              onClick={() => openDir(s.path)}
              title={t("usage.open_source_dir") as string}
              className="ml-auto shrink-0 rounded p-1 text-fg-muted hover:bg-overlay-soft hover:text-fg"
            >
              <FolderOpen size={12} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}