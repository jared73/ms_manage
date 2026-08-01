/**
 * 安装流程 hook:管理 install_one / uninstall_one / 批量预装 / install_progress 事件订阅。
 *
 * 与原 Servers.tsx 内联实现的差异:
 * - 三个 Set + progress + batch + confirmOpen 全部收口在一个 hook 里
 * - listen() 用 cancelled 标记 + unlistenFn 替代 then,避开卸载竞态(P1 #4)
 * - 等待 done/error 时有 10 分钟超时,事件丢失不会永久挂起
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { installServer, uninstallServer } from "@/lib/ipc";
import type { McpServer } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

type Progress = { stage: string; message: string };

export interface BatchState {
  pending: string[];
  current: string | null;
  done: number;
  total: number;
  succeeded: string[];
  failed: { name: string; msg: string }[];
}

export interface UseInstallFlow {
  /** 安装中 server id(单装 & 批量合并) */
  installingIds: Set<string>;
  /** 卸载中 server id */
  uninstallingIds: Set<string>;
  /** 每个 server 当前的进度文本 */
  progress: Record<string, Progress>;
  /** 批量安装状态,null = 不在跑 */
  batch: BatchState | null;
  /** 批量确认对话框是否打开 */
  confirmOpen: boolean;

  installOne: (s: McpServer) => Promise<void>;
  uninstallOne: (s: McpServer) => Promise<void>;
  openConfirm: (pending: McpServer[]) => void;
  closeConfirm: () => void;
  runBatch: (currentPending: McpServer[], reload: () => Promise<void>) => Promise<void>;
  /** 用户中途放弃 —— 把 batchCancelledRef 置位,runBatch 下一轮迭代不再发起新安装。
   *  当前进行中那条 install_server / waitForInstallDone 不会突然中断(避免子进程僵死),
   *  但批会在它返回后结束,reload 一次再总结。 */
  cancelBatch: () => void;
  dismiss: () => void;
}

const INSTALL_TIMEOUT_MS = 10 * 60 * 1000;

export function useInstallFlow(onSettled?: () => void): UseInstallFlow {
  const { t } = useTranslation();
  const [installingIds, setInstallingIds] = useState<Set<string>>(new Set());
  const [uninstallingIds, setUninstallingIds] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<Record<string, Progress>>({});
  const [batch, setBatch] = useState<BatchState | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  // 单装/卸载完成后刷新列表的钩子。用 ref 固定，避免闭包持有过期值；
  // 批量模式跳过（batchRunningRef 为 true 时由 runBatch 自行 reload，避免 N 次全量重取）。
  const onSettledRef = useRef(onSettled);
  onSettledRef.current = onSettled;

  // 批量安装进行中跳过 install_progress 的 load()(避免 N 次全量 3-IPC 重取)
  const batchRunningRef = useRef(false);
  // 卸载/重跑时通知 runBatch 立即停止,避免在 listener 已被卸之后还注册新 resolver
  const batchCancelledRef = useRef(false);
  // runBatch 结束的自动清理定时器(防止卸载后还 setBatch)
  const batchDismissTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // 等待 install_server 走到 done/error 的回调
  const progressResolversRef = useRef<
    Map<string, (r: { ok: boolean; message: string }) => void>
  >(new Map());

  // 监听 install_progress 事件 —— P1 #4 修复:用 cancelled + unlistenFn 替代 then(fn)
  useEffect(() => {
    let cancelled = false;
    let unlistenFn: UnlistenFn | null = null;
    // 把 resolver map 引用固定到本 effect 局部变量:react-hooks 要求 cleanup
    // 用 ref 的"创建时"值,而不是 ref.current 的"运行时"值,避免闭包旧值
    const resolvers = progressResolversRef.current;

    listen<{
      id: string;
      stage: "checking" | "installing" | "resolving" | "done" | "error";
      message: string;
    }>("install_progress", (e) => {
      const { id, stage, message } = e.payload;
      setProgress((prev) => ({ ...prev, [id]: { stage, message } }));
      if (stage === "done" || stage === "error") {
        setInstallingIds((prev) => {
          const n = new Set(prev);
          n.delete(id);
          return n;
        });
        setUninstallingIds((prev) => {
          const n = new Set(prev);
          n.delete(id);
          return n;
        });
        if (stage === "done") {
          toast.success(message, { id: `install-${id}` });
        } else {
          toast.error(message, { id: `install-${id}` });
        }
        const resolver = resolvers.get(id);
        if (resolver) {
          resolver({ ok: stage === "done", message });
          resolvers.delete(id);
        }
        // 单装/卸载走到终态后通知页面刷新列表（批量模式由 runBatch 统一 reload）。
        if (!batchRunningRef.current) {
          onSettledRef.current?.();
        }
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlistenFn = fn;
    });

    return () => {
      cancelled = true;
      unlistenFn?.();
      batchCancelledRef.current = true;
      if (batchDismissTimerRef.current !== null) {
        clearTimeout(batchDismissTimerRef.current);
        batchDismissTimerRef.current = null;
      }
      // 卸载时清掉任何未触发的 progress resolver
      resolvers.forEach((r) =>
        r({ ok: false, message: t("servers.install_wait_timeout") as string }),
      );
      resolvers.clear();
    };
    // t 是 useTranslation 返回的稳定函数,不需要列进 deps
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** 等待某个 server 的 install_event 走到终态,带超时兜底。 */
  const waitForInstallDone = useCallback(
    (id: string): Promise<{ ok: boolean; message: string }> => {
      return new Promise((resolve) => {
        const timer = setTimeout(() => {
          progressResolversRef.current.delete(id);
          resolve({ ok: false, message: t("servers.install_wait_timeout") as string });
        }, INSTALL_TIMEOUT_MS);
        progressResolversRef.current.set(id, (r) => {
          clearTimeout(timer);
          resolve(r);
        });
      });
    },
    // t 来自 useTranslation,稳定;progressResolversRef 是 ref,无需列 deps
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  /** 单装一个 server。 */
  const installOne = useCallback(
    async (s: McpServer) => {
      setInstallingIds((prev) => new Set(prev).add(s.id));
      toast.loading(t("servers.toast_install_one", { name: s.name }) as string, { id: `install-${s.id}` });
      try {
        await installServer(s.id);
        // 结果通过 install_progress 事件回调
      } catch (e) {
        setInstallingIds((prev) => {
          const n = new Set(prev);
          n.delete(s.id);
          return n;
        });
        toast.error(t("servers.toast_install_fail", { msg: describeError(e) }) as string, { id: `install-${s.id}` });
      }
    },
    [t],
  );

  /** 卸载。 */
  const uninstallOne = useCallback(async (s: McpServer) => {
    setUninstallingIds((prev) => new Set(prev).add(s.id));
    toast.loading(t("servers.toast_uninstall_one", { name: s.name }) as string, { id: `install-${s.id}` });
    try {
      await uninstallServer(s.id);
    } catch (e) {
      setUninstallingIds((prev) => {
        const n = new Set(prev);
        n.delete(s.id);
        return n;
      });
      toast.error(t("servers.toast_uninstall_fail", { msg: describeError(e) }) as string, { id: `install-${s.id}` });
    }
  }, [t]);

  const openConfirm = useCallback((pending: McpServer[]) => {
    if (pending.length === 0) {
      toast.info(t("servers.batch_install_no_pending") as string);
      return;
    }
    setConfirmOpen(true);
  }, [t]);

  const closeConfirm = useCallback(() => setConfirmOpen(false), []);

  const dismiss = useCallback(() => {
    if (batchDismissTimerRef.current !== null) {
      clearTimeout(batchDismissTimerRef.current);
      batchDismissTimerRef.current = null;
    }
    setBatch((b) => (b && b.done === b.total ? null : b));
  }, []);

  /** 用户中途取消批量安装。
   * - 设置 ref,runBatch 下一轮迭代会 break
   * - 当前进行中那条 installServer / waitForInstallDone 仍会跑完子进程
   *   (避免子进程变成孤儿)—— 只是结束批
   * - 不会 close 弹窗/dismiss 批状态,留给调用方控制(用户在 UI 上手动 dismiss) */
  const cancelBatch = useCallback(() => {
    batchCancelledRef.current = true;
  }, []);

  /** 串行跑批量安装(同一时刻一个,避免 npm/uv 全局锁冲突)。 */
  const runBatch = useCallback(
    async (
      currentPending: McpServer[],
      reload: () => Promise<void>,
    ) => {
      // 立即清掉上一批的自动 dismiss 定时器 —— 避免上一次 8s 计时
      // 和这一次 reload 同时跑(可能造成 setBatch 被老 timer 抢先清掉)
      if (batchDismissTimerRef.current !== null) {
        clearTimeout(batchDismissTimerRef.current);
        batchDismissTimerRef.current = null;
      }
      batchCancelledRef.current = false;

      setConfirmOpen(false);
      const target = currentPending.filter(
        (s) => s.needs_install === true && !s.installed && s.enabled,
      );
      if (target.length === 0) return;

      const state: BatchState = {
        pending: target.map((s) => s.id),
        current: null,
        done: 0,
        total: target.length,
        succeeded: [],
        failed: [],
      };
      setBatch({ ...state });
      batchRunningRef.current = true;

      for (const s of target) {
        // 卸载后立即停止,不再发起新一轮 install / 不再注册 resolver
        if (batchCancelledRef.current) break;
        state.current = s.id;
        setBatch({ ...state });
        try {
          const waitPromise = waitForInstallDone(s.id);
          await installServer(s.id);
          const r = await waitPromise;
          if (r.ok) state.succeeded.push(s.name);
          else state.failed.push({ name: s.name, msg: r.message });
        } catch (e) {
          state.failed.push({ name: s.name, msg: describeError(e) });
        }
        state.done += 1;
        state.current = null;
        setBatch({ ...state });
      }

      // 已卸载就不动 UI(全部 setter 都会警告)
      if (batchCancelledRef.current) return;

      batchRunningRef.current = false;
      await reload();
      if (state.failed.length === 0) {
        toast.success(t("servers.batch_install_run_done_ok", { count: state.succeeded.length }) as string);
      } else {
        toast.warning(
          t("servers.batch_install_run_done_warn", { ok: state.succeeded.length, fail: state.failed.length }) as string,
        );
      }
      // 保留 batch 状态显示总结,几秒后自动清掉
      batchDismissTimerRef.current = setTimeout(() => {
        batchDismissTimerRef.current = null;
        setBatch((b) => (b && b.done === b.total ? null : b));
      }, 8000);
    },
    [waitForInstallDone, t],
  );

  return {
    installingIds,
    uninstallingIds,
    progress,
    batch,
    confirmOpen,
    installOne,
    uninstallOne,
    openConfirm,
    closeConfirm,
    runBatch,
    cancelBatch,
    dismiss,
  };
}
