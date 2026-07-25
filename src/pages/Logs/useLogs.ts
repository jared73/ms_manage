/**
 * 日志页 hook:拉日志、级别过滤、自动刷新、内存中的 source 集合(给下拉用)。
 *
 * 设计:
 * - 用 useRef + setInterval 实现 3s 自动刷新,只在新依赖变化时重建定时器
 * - 暴露源 / 级别 / 数据 / 清除 / 导出 JSON,主组件只负责渲染
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listLogs, clearLogs } from "@/lib/ipc";
import type { LogEntry } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { toast } from "sonner";

const LEVEL_ORDER = ["debug", "info", "warn", "error"] as const;
type Level = (typeof LEVEL_ORDER)[number];

function toLevel(s: string): Level {
  // 未识别值兜底为 "debug"(最低级别,显示所有日志)
  return (LEVEL_ORDER as readonly string[]).includes(s) ? (s as Level) : "debug";
}

export interface UseLogs {
  logs: LogEntry[];
  level: string;
  sourceFilter: string;
  autoRefresh: boolean;
  /** 当前一次拉取失败的原因(null = OK)。UI 展示"重试"。 */
  loadError: string | null;
  /** 首屏拉取还没拿到过一次响应 — 区分 loading 与 failure。 */
  loading: boolean;
  sources: string[];
  setLevel: (v: string) => void;
  setSourceFilter: (v: string) => void;
  setAutoRefresh: (v: boolean) => void;
  /** 手动触发完整 reload,会清除错误状态。常用于 "重试" 按钮。 */
  reload: () => Promise<void>;
  /** 静默轮询 reload(失败只更新错误状态,不抛 toast) */
  silentReload: () => Promise<void>;
  clear: () => Promise<void>;
  /** 弹出"确认清空"二次确认弹窗需要的 confirm 状态 */
  confirmClear: boolean;
  setConfirmClear: (v: boolean) => void;
}

export function useLogs(): UseLogs {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [level, setLevel] = useState<string>("info");
  const [sourceFilter, setSourceFilter] = useState<string>("");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);

  // load() 引用保持最新 —— 防止 effect 闭包旧值引发 stale load
  // inFlightRef:避免上一个 3s 轮询还在 IPC 时又触发下一次,落后响应覆盖最新数据
  const inFlightRef = useRef(false);
  /**
   * 静默 reload:用于定时轮询。
   * - 失败不弹 toast(每 3s 弹一次会很烦),仅记录 loadError 让 UI 显示重试按钮。
   * - 第一次拿到空数据(可能是日志真的为空)不算 error,继续后续轮询。
   */
  const silentReload = useCallback(async () => {
    if (inFlightRef.current) return; // 上一次还没回,丢掉本次
    inFlightRef.current = true;
    try {
      const r = await listLogs({ limit: 1000, source: sourceFilter || undefined });
      const min = LEVEL_ORDER.indexOf(toLevel(level));
      setLogs(r.filter((l) => LEVEL_ORDER.indexOf(toLevel(l.level)) >= min));
      setLoadError(null);
    } catch (e) {
      setLoadError(describeError(e));
    } finally {
      inFlightRef.current = false;
      setLoading(false);
    }
  }, [level, sourceFilter]);

  /** 用户主动 reload:失败弹 toast,清错误状态,触发 loading 重置。 */
  const reload = useCallback(async () => {
    setLoadError(null);
    setLoading(true);
    await silentReload();
  }, [silentReload]);

  const loadRef = useRef<() => Promise<void>>(async () => undefined);
  loadRef.current = silentReload;

  useEffect(() => {
    void silentReload();
  }, [silentReload]);

  useEffect(() => {
    if (!autoRefresh) return;
    const handle = setInterval(() => {
      void loadRef.current();
    }, 3000);
    return () => clearInterval(handle);
  }, [autoRefresh]);

  // 给下拉用:当前已加载的日志里出现过的 source
  const sources = Array.from(new Set(logs.map((l) => l.source))).sort();

  const clear = useCallback(async () => {
    setConfirmClear(false);
    try {
      await clearLogs();
      await silentReload();
      toast.success(t("logs.cleared"), { duration: 1500 });
    } catch (e) {
      toast.error(t("logs.clear_failed", { msg: describeError(e) }) as string);
    }
  }, [silentReload]);

  return {
    logs,
    level,
    sourceFilter,
    autoRefresh,
    loadError,
    loading,
    sources,
    setLevel,
    setSourceFilter,
    setAutoRefresh,
    reload,
    silentReload,
    clear,
    confirmClear,
    setConfirmClear,
  };
}
