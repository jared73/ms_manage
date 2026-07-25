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
  sources: string[];
  setLevel: (v: string) => void;
  setSourceFilter: (v: string) => void;
  setAutoRefresh: (v: boolean) => void;
  reload: () => Promise<void>;
  clear: () => Promise<void>;
}

export function useLogs(): UseLogs {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [level, setLevel] = useState<string>("info");
  const [sourceFilter, setSourceFilter] = useState<string>("");
  const [autoRefresh, setAutoRefresh] = useState(true);

  // load() 引用保持最新 —— 防止 effect 闭包旧值引发 stale load
  // inFlightRef:避免上一个 3s 轮询还在 IPC 时又触发下一次,落后响应覆盖最新数据
  const inFlightRef = useRef(false);
  const reload = useCallback(async () => {
    if (inFlightRef.current) return; // 上一次还没回,丢掉本次
    inFlightRef.current = true;
    try {
      const r = await listLogs({ limit: 1000, source: sourceFilter || undefined });
      const min = LEVEL_ORDER.indexOf(toLevel(level));
      setLogs(r.filter((l) => LEVEL_ORDER.indexOf(toLevel(l.level)) >= min));
    } finally {
      inFlightRef.current = false;
    }
  }, [level, sourceFilter]);

  const loadRef = useRef<() => Promise<void>>(async () => undefined);
  loadRef.current = reload;

  useEffect(() => {
    void reload();
  }, [reload]);

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
    try {
      await clearLogs();
      await reload();
      toast.success(t("logs.cleared"), { duration: 1500 });
    } catch (e) {
      toast.error(t("logs.clear_failed", { msg: describeError(e) }) as string);
    }
  }, [reload]);

  return {
    logs,
    level,
    sourceFilter,
    autoRefresh,
    sources,
    setLevel,
    setSourceFilter,
    setAutoRefresh,
    reload,
    clear,
  };
}
