/**
 * Servers 主页面。
 *
 * 设计:
 * - 数据 + 副作用收到 ./useServers.ts(后续 P0 #2)和 ./useInstallFlow.ts
 * - 4 个 Modal/Drawer 分别放在各自文件
 * - 表格行是 <ServerRow/> 用 React.memo(P1 #5)
 *
 * 子组件统一通过相对路径 ./xxx 引用,IPC 与类型仍走 @/lib/ipc。
 */
import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Plus,
  Trash2,
  Pencil,
  Play,
  Square,
  TestTube2,
  FolderOpen,
  Loader2,
  FileInput,
  FileDown,
  X,
  Download,
  PackageCheck,
  Unplug,
  Zap,
  RefreshCw,
  Check,
  XCircle,
  Copy,
  CopyPlus,
  Search,
} from "lucide-react";
import { toast } from "sonner";
import { Section, EmptyHint } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Modal } from "@/components/Modal";
import { ConfirmModal } from "@/components/ConfirmModal";
import {
  listServers,
  listServerStates,
  listProjects,
  startServer,
  stopServer,
  testServer,
  refreshInstallStatus,
  deleteServer,
  upsertServer,
} from "@/lib/ipc";
import type { McpServer, ServerState, Project } from "@/lib/ipc";
import { ProjectsModal } from "./ProjectsModal";
import { ServerFormModal } from "./ServerFormModal";
import { blankServer } from "./blankServer";
import { ImportJsonModal } from "./ImportJsonModal";
import { ServerDetailDrawer } from "./ServerDetailDrawer";
import { useInstallFlow } from "./useInstallFlow";
import { describeError } from "@/lib/errors";

export function Servers() {
  const { t } = useTranslation();
  // ---- 数据 ----
  const [servers, setServers] = useState<McpServer[]>([]);
  const [states, setStates] = useState<Record<string, ServerState>>({});
  const [projects, setProjects] = useState<Project[]>([]);
  const [search, setSearch] = useState("");
  const [filterProject, setFilterProject] = useState<string>("");
  // 筛选确认:输入框的值 ≠ 实际过滤值,Enter/按钮才提交
  const [searchInput, setSearchInput] = useState("");
  // 已清除的错误(server.name → true),前端隐藏,不删日志
  const [dismissedErrors, setDismissedErrors] = useState<Set<string>>(new Set());

  // ---- Modal / Drawer ----
  const [editing, setEditing] = useState<McpServer | null>(null);
  const [creating, setCreating] = useState(false);
  const [managingProjects, setManagingProjects] = useState(false);
  const [viewing, setViewing] = useState<McpServer | null>(null);
  const [importModalOpen, setImportModalOpen] = useState(false);
  // 删除确认 Modal —— 避免误删
  const [confirmDelete, setConfirmDelete] = useState<McpServer | null>(null);

  // ---- 操作中的 id 集合 ----
  const [startingIds, setStartingIds] = useState<Set<string>>(new Set());
  const [stoppingIds, setStoppingIds] = useState<Set<string>>(new Set());
  // 测试/探测/删除等其他异步操作的 in-flight 集合
  const [probingIds, setProbingIds] = useState<Set<string>>(new Set());
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());

  // ---- 数据加载 ----
  const load = async () => {
    try {
      const [ss, sts, ps] = await Promise.all([listServers(), listServerStates(), listProjects()]);
      setServers(ss);
      setStates(Object.fromEntries(sts.map((s) => [s.server.id, s])));
      setProjects(ps);
    } catch (e) {
      toast.error(t("servers.toast_load_failed", { msg: describeError(e) }) as string);
    }
  };

  // 安装进度 + 批量流程(独立 hook)。load 在 useInstallFlow 之前定义,
  // 单装/卸载完成后由 hook 回调触发列表刷新。
  const install = useInstallFlow(load);

  useEffect(() => {
    void load();
    // 只在挂载时拉一次
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const filtered = useMemo(
    () =>
      servers.filter(
        (s) =>
          (!search ||
            s.name.toLowerCase().includes(search.toLowerCase()) ||
            (s.description ?? "").toLowerCase().includes(search.toLowerCase())) &&
          (!filterProject ||
            (filterProject === "__unassigned__" && !s.project_id) ||
            s.project_id === filterProject),
      ),
    [servers, search, filterProject],
  );

  // 项目 id → 名称 的查找表。
  // 原先在 ServerRow 渲染时 `projects.find(...)`,每行 O(P),M 行做 M×P 次 work;
  // 提到父组件用 Map 缓存,降到 O(M+P) —— 真正发挥 React.memo 的浅比较意义。
  const projectNameById = useMemo(() => {
    const m = new Map<string, string>();
    for (const p of projects) m.set(p.id, p.name);
    return m;
  }, [projects]);

  const needsInstall = (s: McpServer): boolean => {
    // 优先用后端字段(列表 refresh 后会带回)。
    if (s.needs_install === true) return true;
    if (s.needs_install === false) return false;
    // 新建 server 后到下次 listServers 之前,后端字段是 undefined。
    // 本地用命令白名单兜底 —— 避免 UX 信号丢失。
    const cmd = (s.command ?? "").trim();
    if (!cmd) return false;
    const stem = cmd.toLowerCase();
    return ["npx", "uvx", "pnpx", "bunx"].includes(stem);
  };

  const handleDelete = async (s: McpServer) => {
    if (deletingIds.has(s.id)) return;
    setDeletingIds((prev) => new Set(prev).add(s.id));
    try {
      await deleteServer(s.id);
      await load();
      toast.success(t("servers.toast_deleted", { name: s.name }) as string);
    } catch (e) {
      toast.error(t("servers.toast_delete_failed", { msg: describeError(e) }) as string);
    } finally {
      setDeletingIds((prev) => {
        const n = new Set(prev);
        n.delete(s.id);
        return n;
      });
    }
  };

  const handleTest = async (s: McpServer) => {
    if (probingIds.has(s.id)) return;
    setProbingIds((prev) => new Set(prev).add(s.id));
    const tid = `test-${s.id}`;
    toast.loading(t("servers.toast_testing", { name: s.name }) as string, { id: tid });
    try {
      const r = await testServer(s.id);
      if (r.ok) {
        toast.success(
          t("servers.toast_test_ok", { name: s.name, latency: r.latency_ms ?? "?", count: (r.tools ?? []).length }) as string,
          { id: tid },
        );
      } else {
        toast.error(t("servers.toast_test_fail", { name: s.name, error: r.error }) as string, { id: tid });
      }
    } catch (e) {
      toast.error(t("servers.toast_test_error", { msg: describeError(e) }) as string, { id: tid });
    } finally {
      setProbingIds((prev) => {
        const n = new Set(prev);
        n.delete(s.id);
        return n;
      });
    }
  };

  const handleStart = async (s: McpServer) => {
    if (needsInstall(s) && !s.installed) {
      toast.error(t("servers.toast_install_first", { name: s.name }) as string);
      return;
    }
    setStartingIds((prev) => new Set(prev).add(s.id));
    toast.loading(t("servers.toast_starting", { name: s.name }) as string, { id: `start-${s.id}` });
    try {
      // 启动 + 同步 auto_start 一次完成,避免再发 upsert_server 触发全量 load
      await startServer(s.id, !s.auto_start ? true : undefined);
      toast.success(t("servers.toast_started", { name: s.name }) as string, { id: `start-${s.id}` });
      await load();
    } catch (e) {
      toast.error(t("servers.toast_start_failed", { name: s.name, msg: describeError(e) }) as string, { id: `start-${s.id}` });
    } finally {
      setStartingIds((prev) => {
        const n = new Set(prev);
        n.delete(s.id);
        return n;
      });
    }
  };

  const handleStop = async (s: McpServer) => {
    setStoppingIds((prev) => new Set(prev).add(s.id));
    toast.loading(t("servers.toast_stopping", { name: s.name }) as string, { id: `stop-${s.id}` });
    try {
      // 停止 + 同步 auto_start 一次完成
      await stopServer(s.id, s.auto_start ? false : undefined);
      toast.success(t("servers.toast_stopped", { name: s.name }) as string, { id: `stop-${s.id}` });
      await load();
    } catch (e) {
      toast.error(t("servers.toast_stop_failed", { name: s.name, msg: describeError(e) }) as string, { id: `stop-${s.id}` });
    } finally {
      setStoppingIds((prev) => {
        const n = new Set(prev);
        n.delete(s.id);
        return n;
      });
    }
  };

  const handleRefreshInstall = async (s: McpServer) => {
    toast.loading(t("servers.toast_probing", { name: s.name }) as string, { id: `install-${s.id}` });
    try {
      await refreshInstallStatus(s.id);
      toast.success(t("servers.toast_probed", { name: s.name }) as string, { id: `install-${s.id}` });
      await load();
    } catch (e) {
      toast.error(t("servers.toast_refresh_failed", { msg: describeError(e) }) as string, { id: `install-${s.id}` });
    }
  };

  /**
   * 导入完成后的刷新:
   * 1. 自动探测一遍「需要装但还没装」的服务 —— 否则刚导入的服务(upsert 时
   *    installed 默认为 false)即使本机已装过该包,也只会显示「安装」按钮。
   * 2. 对本次导入、已装且未运行的服务自动启动(不修改 auto_start)。
   */
  const handleImported = useCallback(async (names?: string[]) => {
    try {
      const ss = await listServers();
      const targets = ss.filter((s) => needsInstall(s) && !s.installed);
      await Promise.allSettled(targets.map((s) => refreshInstallStatus(s.id)));
    } catch {
      // 探测失败不影响导入结果,列表照常刷新
    }
    await load();
    if (names && names.length > 0) {
      try {
        const [ss, sts] = await Promise.all([listServers(), listServerStates()]);
        const running = new Set(sts.filter((s) => s.status === "running").map((s) => s.server.id));
        const toStart = ss.filter(
          (s) => names.includes(s.name) && s.enabled && s.installed && !running.has(s.id),
        );
        if (toStart.length > 0) {
          await Promise.allSettled(toStart.map((s) => startServer(s.id)));
          await load();
        }
      } catch {
        // 自动启动失败不打扰用户,列表已刷新
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- load 每次 render 重建,useCallback 只需首次捕获的 setter
  }, []);

  const handleDuplicate = async (s: McpServer) => {
    const newName = `${s.name} (Copy)`;
    const clone: McpServer = {
      ...s,
      id: crypto.randomUUID(),
      name: newName,
      enabled: false,
      auto_start: false,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    try {
      await upsertServer(clone);
      await load();
      toast.success(t("servers.toast_duplicated", { name: s.name, newName }) as string);
    } catch (e) {
      toast.error(t("servers.toast_save_failed", { msg: describeError(e) }) as string);
    }
  };

  const handleCopyConfig = async (s: McpServer) => {
    try {
      const json = buildMcpServersJson([s]);
      await navigator.clipboard.writeText(json);
      toast.success(t("servers.toast_config_copied", { name: s.name }) as string);
    } catch {
      // fallback: trigger download
      const blob = new Blob([buildMcpServersJson([s])], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${s.name}_mcp.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast.info(t("servers.toast_clipboard_failed", { label: s.name }) as string);
    }
  };

  /**
   * 行级 action 派发器:稳定的 useCallback 一次创建,所有 row 共用。
   * 各 row 通过 onAction(action, server) 调用,避免 11 个 inline 箭头 prop
   * 让 React.memo 失效的问题(P1 #5 的真正生效形态)。
   *
   * 参数用 ServerRowAction 联合类型而非 string,这样未来加新 action 时
   * 编译器会要求这里穷举处理,避免 silently swallowed。
   */
  const dispatchRow = useCallback(
    async (action: ServerRowAction, s: McpServer) => {
      switch (action) {
        case "start": return handleStart(s);
        case "stop": return handleStop(s);
        case "test": return handleTest(s);
        case "delete": setConfirmDelete(s); return;
        case "edit": setEditing(s); return;
        case "view": setViewing(s); return;
        case "install": return install.installOne(s);
        case "uninstall": return install.uninstallOne(s);
        case "refresh": return handleRefreshInstall(s);
        case "duplicate": return handleDuplicate(s);
        case "copyConfig": return handleCopyConfig(s);
      }
      // 编译期保证:每个 action 都被处理,这里不需要 default
      const _exhaustive: never = action;
      void _exhaustive;
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  const pendingInstall = useMemo(
    () => servers.filter((s) => needsInstall(s) && !s.installed && s.enabled),
    [servers],
  );

  /**
   * 把一批 server 打成 mcpServers JSON。
   * - 单一来源:exportSingle 和 exportAll 都调它,避免 11 字段拼装逻辑双份维护
   *   导致以后改 transport 字段(比如 http 加 auth)漂移。
   * - http/sse → `type` + `url` + `headers`
   * - stdio → `command` + `args` + `env` + `cwd`
   */
  const buildMcpServersJson = useCallback((items: McpServer[]): string => {
    const mcpServers: Record<string, unknown> = {};
    for (const s of items) {
      if (s.transport === "http") {
        mcpServers[s.name] = { type: "http", url: s.url, headers: s.headers };
      } else if (s.transport === "sse") {
        mcpServers[s.name] = { type: "sse", url: s.url, headers: s.headers };
      } else {
        mcpServers[s.name] = { command: s.command, args: s.args, env: s.env, cwd: s.cwd };
      }
    }
    return JSON.stringify({ mcpServers }, null, 2);
  }, []);

  /**
   * 复制并下载一段 JSON。
   * - clipboard.writeText 失败时(部分 Webview 沙箱)弹 toast 提示用户去下载目录找;
   *   之前 .catch(() => {}) 静默吞掉,用户以为复制成功实际剪贴板是空的。
   * - 下载动作不阻塞,失败也至少有一个降级路径(用户能拿到文件)。
   * - i18n 的 success key 必须带 {{label}},与各家 toaster 兼容。
   */
  const copyAndDownload = useCallback(async (
    json: string,
    filename: string,
    label: string,
    successKey: string,
  ) => {
    let clipboardOk = true;
    try {
      await navigator.clipboard.writeText(json);
    } catch {
      clipboardOk = false;
    }
    if (!clipboardOk) {
      toast.warning(t("servers.toast_clipboard_failed", { label }) as string);
    } else {
      toast.success(t(successKey, { label }) as string);
    }
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  }, [t]);

  /** 导出全部 enabled server。 */
  const exportToJson = async () => {
    const enabled = servers.filter((s) => s.enabled);
    await copyAndDownload(
      buildMcpServersJson(enabled),
      `mcp_servers_${new Date().toISOString().slice(0, 10)}.json`,
      t("servers.export_all_btn_label"),
      "servers.toast_exported_all",
    );
    // export-all 显式提示一下 count,toast 已经成功时不再重复。
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("servers.title")}
        action={
          <div className="flex items-center gap-2">
            {pendingInstall.length > 0 && (
              <button
                className="inline-flex items-center gap-1.5 rounded-md border border-accent/40 bg-accent/10 px-3 py-1.5 text-xs font-medium text-accent hover:bg-accent/20 disabled:opacity-50"
                onClick={() => install.openConfirm(pendingInstall)}
                disabled={install.batch !== null}
                title={t("servers.install_btn_tooltip")}
              >
                {install.batch ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Download size={14} />
                )}
                {t("servers.install_btn_text", { count: pendingInstall.length })}
              </button>
            )}
            <button className="btn-ghost" onClick={load} title={t("servers.refresh_list") as string}>
              <RefreshCw size={14} />
            </button>
            <button className="btn-ghost" onClick={() => setManagingProjects(true)}>
              <FolderOpen size={14} />
              {t("projects.title")}
            </button>
            <button
              className="btn-ghost"
              onClick={() => setImportModalOpen(true)}
              title={t("servers.import_btn_tooltip") as string}
            >
              <FileInput size={14} />
              {t("servers.import_btn")}
            </button>
            <button
              className="btn-ghost"
              onClick={exportToJson}
              title={t("servers.export_btn_tooltip") as string}
            >
              <FileDown size={14} />
              {t("servers.export_btn")}
            </button>
            <button className="btn-primary" onClick={() => setCreating(true)}>
              <Plus size={14} />
              {t("common.add")}
            </button>
          </div>
        }
      >
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative flex max-w-sm flex-1">
            <input
              className="input rounded-r-none"
              placeholder={t("common.search")}
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") setSearch(searchInput); }}
              aria-label={t("common.search")}
            />
            <button
              className="btn-primary rounded-l-none"
              onClick={() => setSearch(searchInput)}
              title={t("common.search")}
            >
              <Search size={14} />
            </button>
          </div>
          <select
            className="input max-w-[180px]"
            value={filterProject}
            onChange={(e) => setFilterProject(e.target.value)}
            aria-label={t("servers.projects_filter_label") as string}
          >
            <option value="">{t("servers.projects_filter_all")}</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <span className="text-xs text-fg-muted tabular-nums">
            {filtered.length} / {servers.length}
          </span>
        </div>

        {install.batch && (
          <div className="surface-accent-soft rounded-md px-3 py-2 text-xs">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <Loader2 size={12} className="animate-spin text-accent" />
                <span>
                  {t("servers.batch_dialog_progress", { done: install.batch.done, total: install.batch.total })}
                  {install.batch.current &&
                    (() => {
                      const cur = servers.find((s) => s.id === install.batch!.current);
                      return cur ? (
                        <span className="ml-2 text-fg-muted">
                          {t("servers.batch_dialog_installing", { name: "" })}
                          <span className="text-fg">{cur.name}</span>
                        </span>
                      ) : null;
                    })()}
                </span>
              </div>
              {/* 跑完前显示 ✕ 取消按钮 —— 当前这条会跑完(避免子进程僵死),
                  后面的 server 一律不再启动 */}
              {install.batch.done < install.batch.total && (
                <button
                  className="btn-ghost text-2xs text-err hover:text-err/80"
                  onClick={() => {
                    install.cancelBatch();
                    toast.info(t("servers.batch_install_cancelled") as string);
                  }}
                  title={t("servers.batch_install_cancel_tooltip") as string}
                >
                  <X size={12} />
                  {t("servers.batch_install_cancel_btn")}
                </button>
              )}
              {install.batch.done === install.batch.total && (
                <div className="flex items-center gap-3">
                  <span className="text-ok"><Check size={12} aria-hidden="true" /> {install.batch.succeeded.length}</span>
                  {install.batch.failed.length > 0 && (
                    <span className="text-err"><XCircle size={12} aria-hidden="true" /> {install.batch.failed.length}</span>
                  )}
                  <button
                    className="text-fg-muted hover:text-fg"
                    onClick={install.dismiss}
                    title={t("servers.close_btn_text") as string}
                  >
                    <X size={12} />
                  </button>
                </div>
              )}
            </div>
            <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-bg-subtle">
              <div
                className="h-full bg-accent transition-[width]"
                style={{ width: `${(install.batch.done / install.batch.total) * 100}%` }}
              />
            </div>
            {install.batch.failed.length > 0 && install.batch.done === install.batch.total && (
              <ul className="mt-1.5 space-y-0.5 text-err/90">
                {install.batch.failed.map((f, i) => (
                  <li key={i} className="truncate">
                    · {f.name}: {f.msg}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        {filtered.length === 0 ? (
          <EmptyHint>{t("common.empty")}</EmptyHint>
        ) : (
          // 横向滚动容器:表格列宽总和 > 容器宽度时 (例如 6 列的工具列会顶起来),
          // 不允许整页横向滚动只允许表格区域横向滚动 —— 避免侧栏也被推跑。
          <div className="overflow-hidden rounded-lg border border-border shadow-sm">
            <div className="overflow-x-auto">
            <table className="w-full min-w-[800px] text-sm">
              <thead>
                <tr className="border-b border-border bg-bg-subtle">
                  <th scope="col" className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("common.name")}</th>
                  <th scope="col" className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("common.transport")}</th>
                  <th scope="col" className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("projects.title")}</th>
                  <th scope="col" className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("common.status")}</th>
                  <th scope="col" className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("servers.col_pkg")}</th>
                  <th scope="col" className="px-4 py-2.5 text-left text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("common.last_error")}</th>
                  <th scope="col" className="px-4 py-2.5 text-right text-xs font-semibold uppercase tracking-wider text-fg-muted">{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {filtered.map((s) => (
                  <ServerRow
                    key={s.id}
                    server={s}
                    state={states[s.id]}
                    projectName={projectNameById.get(s.project_id ?? "") ?? "—"}
                    isStarting={startingIds.has(s.id)}
                    isStopping={stoppingIds.has(s.id)}
                    isProbing={probingIds.has(s.id)}
                    isDeleting={deletingIds.has(s.id)}
                    isInstalling={
                      install.installingIds.has(s.id) || install.batch !== null
                    }
                    isUninstalling={install.uninstallingIds.has(s.id)}
                    installProgress={install.progress[s.id]}
                    errorDismissed={dismissedErrors.has(s.name)}
                    onDismissError={() => setDismissedErrors((p) => new Set(p).add(s.name))}
                    onAction={dispatchRow}
                  />
                ))}
              </tbody>
            </table>
            </div>
          </div>
        )}
      </Section>

      {(editing || creating) && (
        <ServerFormModal
          initial={editing ?? blankServer()}
          projects={projects.filter((p) => p.id !== "__unassigned__")}
          onClose={() => {
            setEditing(null);
            setCreating(false);
          }}
          onSaved={async () => {
            setEditing(null);
            setCreating(false);
            await load();
          }}
        />
      )}

      {managingProjects && (
        <ProjectsModal
          onClose={() => setManagingProjects(false)}
          onChanged={async () => {
            await load();
          }}
        />
      )}

      {importModalOpen && (
        <ImportJsonModal
          onClose={() => setImportModalOpen(false)}
          onImported={handleImported}
        />
      )}

      {/* 删除确认 Modal —— 防误删(用 ConfirmModal 抽取的共享确认组件) */}
      <ConfirmModal
        open={!!confirmDelete}
        title={t("common.confirm")}
        confirmLabel={t("common.delete")}
        busy={!!confirmDelete && deletingIds.has(confirmDelete.id)}
        onClose={() => setConfirmDelete(null)}
        onConfirm={async () => {
          if (!confirmDelete) return;
          const target = confirmDelete;
          setConfirmDelete(null);
          await handleDelete(target);
        }}
      >
        <p className="text-sm">{t("servers.delete_confirm")}</p>
        {confirmDelete && (
          <p className="mt-2 break-all font-mono text-xs text-fg-muted">
            {confirmDelete.name}
            {confirmDelete.description ? ` — ${confirmDelete.description}` : ""}
          </p>
        )}
      </ConfirmModal>

      {install.confirmOpen && (
        <Modal
          open
          title={t("servers.batch_dialog_text")}
          onClose={install.closeConfirm}
          footer={
            <>
              <button className="btn-ghost" onClick={install.closeConfirm}>
                {t("common.cancel")}
              </button>
              <button
                className="btn-primary"
                onClick={() => install.runBatch(pendingInstall, load)}
              >
                <Download size={14} />
                {t("servers.batch_install_start_btn", { count: pendingInstall.length })}
              </button>
            </>
          }
        >
          <div className="space-y-3 text-sm">
            <p>
              {t("servers.batch_dialog_subtitle", { count: pendingInstall.length }) as string}
            </p>
            <p className="text-xs text-fg-muted">
              {t("servers.batch_dialog_serial_hint") as string}
            </p>
            <div className="max-h-60 overflow-auto rounded border border-border bg-bg-subtle p-2">
              <ul className="space-y-1">
                {pendingInstall.map((s) => (
                  <li key={s.id} className="flex items-center gap-2">
                    <span className="badge-mute text-3xs">{s.command}</span>
                    <span className="font-medium">{s.name}</span>
                    <code className="flex-1 truncate text-xs text-fg-muted">
                      {(s.args ?? []).join(" ")}
                    </code>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </Modal>
      )}

      {viewing && (
        <ServerDetailDrawer
          server={viewing}
          onClose={() => setViewing(null)}
          onChanged={load}
        />
      )}
    </div>
  );
}

// =================================================================
// ServerRow:表格行子组件,memo 避免无操作时整张表重渲(P1 #5)
// =================================================================

/** ServerRow 支持的 action 名称。父级 dispatch 走 (action, server) 派发,
 *  这样 React.memo 才能真正生效(避免 11 个 inline 箭头 prop 让浅比较失效)。 */
export type ServerRowAction =
  | "start" | "stop" | "test" | "delete" | "edit" | "view"
  | "install" | "uninstall" | "refresh" | "duplicate" | "copyConfig";

interface ServerRowProps {
  server: McpServer;
  state?: ServerState;
  projectName: string;
  isStarting: boolean;
  isStopping: boolean;
  isInstalling: boolean;
  isUninstalling: boolean;
  isProbing: boolean;
  isDeleting: boolean;
  installProgress?: { stage: string; message: string };
  errorDismissed: boolean;
  onDismissError: () => void;
  /**
   * 稳定的 useCallback 派发器。父组件不应传新箭头,否则 memo 会失效。
   * memo 比较 server / state / projectName / 7 个 bool / progress(text) 是否变化,
   *  不变就跳过整行重渲。
   */
  onAction: (action: ServerRowAction, server: McpServer) => void;
}

const ServerRow = React.memo(function ServerRow({
  server: s,
  state: st,
  projectName,
  isStarting,
  isStopping,
  isInstalling,
  isUninstalling,
  isProbing,
  isDeleting,
  installProgress,
  errorDismissed,
  onDismissError,
  onAction,
}: ServerRowProps) {
  const { t } = useTranslation();
  const showInstall = s.needs_install === true;
  const startBlocked = showInstall && !s.installed;
  const anyBusy = isStarting || isStopping || isProbing || isDeleting;
  const isDisabled = !s.enabled;

  const transportBadge = (t: string) => {
    switch (t) {
      case "stdio": return "badge-accent";
      case "sse": return "badge-ok";
      case "http": return "badge-warn";
      default: return "badge-mute";
    }
  };

  return (
    <tr className={`group transition-colors hover:bg-overlay-soft ${isDisabled ? "opacity-50" : ""}`}>
      {/* Name + description */}
      <td className="px-4 py-2.5 cursor-pointer" onClick={() => onAction("view", s)}>
        <div className="flex items-center gap-2">
          {/* 启/禁用指示点 */}
          <span
            className={`inline-block h-2 w-2 shrink-0 rounded-full ${s.enabled ? "bg-ok" : "bg-fg-muted"}`}
            title={s.enabled ? t("servers.detail_status_enabled") : t("servers.detail_status_disabled")}
          />
          <div className="min-w-0">
            <div className="truncate font-medium">{s.name}</div>
            {s.description && (
              <div className="truncate text-xs text-fg-muted">{s.description}</div>
            )}
          </div>
        </div>
      </td>
      {/* Transport */}
      <td className="px-4 py-2.5">
        <span className={`badge uppercase text-3xs ${transportBadge(s.transport)}`}>{s.transport}</span>
      </td>
      {/* Project */}
      <td className="px-4 py-2.5 text-xs text-fg-muted">{projectName}</td>
      {/* Status */}
      <td className="px-4 py-2.5">
        {st ? <StatusBadge status={st.status} /> : <span className="text-xs text-fg-muted">—</span>}
      </td>
      {/* Package */}
      <td className="px-4 py-2.5 text-xs">
        {!showInstall ? (
          <span className="text-fg-muted">—</span>
        ) : isInstalling || isUninstalling ? (
          <div className="flex items-center gap-1.5">
            <Loader2 size={12} className="animate-spin text-accent" />
            <span className="truncate text-fg-muted" title={installProgress?.message}>
              {installProgress?.message ?? t("servers.install_processing")}
            </span>
          </div>
        ) : (
          <div className="flex flex-col gap-1">
            <button
              className="inline-flex w-fit items-center gap-1 rounded px-1 py-0.5 hover:bg-overlay-soft"
              onClick={() => onAction("refresh", s)}
              title={t("servers.refresh_install_tooltip_btn")}
            >
              {s.installed ? (
                <>
                  <PackageCheck size={12} className="text-ok" />
                  <span className="text-ok">{t("servers.pkg_label_installed", { version: s.installed_version ?? "?" })}</span>
                </>
              ) : s.install_error ? (
                <span className="text-err">{t("servers.pkg_label_failed")}</span>
              ) : (
                <>
                  <Zap size={12} className="text-fg-muted" />
                  <span className="text-fg-muted">{t("servers.pack_on_demand")}</span>
                </>
              )}
            </button>
            {s.install_error && (
              <span className="truncate text-3xs text-fg-muted max-w-[180px]" title={s.install_error}>
                {s.install_error}
              </span>
            )}
            <div className="flex items-center gap-1">
              {s.installed ? (
                <>
                  <button
                    className="btn-ghost p-1 text-warn hover:text-warn/80 disabled:opacity-40"
                    onClick={() => onAction("install", s)}
                    disabled={isInstalling}
                    title={t("servers.tooltip_upgrade")}
                    aria-label={t("servers.tooltip_upgrade")}
                  >
                    <RefreshCw size={12} />
                  </button>
                  <button
                    className="btn-ghost p-1 text-fg-muted hover:text-err disabled:opacity-40"
                    onClick={() => onAction("uninstall", s)}
                    disabled={isUninstalling}
                    title={t("servers.tooltip_uninstall")}
                    aria-label={t("servers.tooltip_uninstall")}
                  >
                    <Unplug size={12} />
                  </button>
                </>
              ) : (
                <button
                  className="btn-ghost inline-flex items-center gap-1 px-1.5 py-0.5 text-accent hover:opacity-80 disabled:opacity-40"
                  onClick={() => onAction("install", s)}
                  disabled={isInstalling}
                  title={t("servers.tooltip_install")}
                  aria-label={t("servers.tooltip_install")}
                >
                  <Download size={12} />
                  <span>{t("servers.install_btn_text_short")}</span>
                </button>
              )}
            </div>
          </div>
        )}
      </td>
      {/* Last error */}
      <td className="px-4 py-2.5">
        {st?.last_error && !errorDismissed ? (
          <div className="flex items-center gap-1">
            <span className="block max-w-[130px] truncate text-xs text-err/80" title={st.last_error}>
              {st.last_error}
            </span>
            <button
              className="shrink-0 rounded px-1 py-0.5 text-2xs text-err/60 hover:bg-err/10 hover:text-err transition-colors"
              onClick={onDismissError}
              title={t("common.close")}
            >
              <X size={12} />
            </button>
          </div>
        ) : (
          <span className="text-xs text-fg-muted">—</span>
        )}
      </td>
      {/* Actions */}
      <td className="px-4 py-2.5">
        <div className="flex items-center justify-end gap-0.5">
          {/* 启动/停止 */}
          {st?.status === "running" ? (
            <button
              className="btn-ghost p-1.5 text-err hover:opacity-80 disabled:opacity-40"
              onClick={() => onAction("stop", s)}
              disabled={isStopping || isProbing || isDeleting}
              title={t("common.stop")}
              aria-label={t("servers.row_stop")}
            >
              {isStopping ? <Loader2 size={14} className="animate-spin" /> : <Square size={14} />}
            </button>
          ) : (
            <button
              className="btn-ghost p-1.5 text-ok disabled:opacity-40"
              onClick={() => onAction("start", s)}
              disabled={isStarting || startBlocked || isProbing || isDeleting}
              title={startBlocked ? t("servers.preinst_required_for_start") : t("common.start")}
              aria-label={t("servers.row_start")}
            >
              {isStarting ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
            </button>
          )}
          {/* 分隔 */}
          <span className="mx-0.5 h-4 w-px bg-border" />
          {/* 测试 */}
          <button
            className="btn-ghost p-1.5 text-warn hover:text-warn/80 disabled:opacity-40"
            onClick={() => onAction("test", s)}
            disabled={anyBusy}
            title={t("common.test")}
            aria-label={t("common.test")}
          >
            {isProbing ? <Loader2 size={14} className="animate-spin text-warn" /> : <TestTube2 size={14} />}
          </button>
          {/* 编辑 */}
          <button
            className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-40"
            onClick={() => onAction("edit", s)}
            disabled={anyBusy}
            title={t("common.edit")}
            aria-label={t("servers.row_edit")}
          >
            <Pencil size={14} />
          </button>
          {/* 更多操作：分组在下拉或直接露出 */}
          <span className="mx-0.5 h-4 w-px bg-border" />
          {/* 复制配置 */}
          <button
            className="btn-ghost p-1.5 text-fg-muted hover:text-accent disabled:opacity-40"
            onClick={() => onAction("copyConfig", s)}
            disabled={anyBusy}
            title={t("servers.copy_server_config")}
            aria-label={t("servers.copy_server_config")}
          >
            <Copy size={14} />
          </button>
          {/* 复制服务器 */}
          <button
            className="btn-ghost p-1.5 text-fg-muted hover:text-accent disabled:opacity-40"
            onClick={() => onAction("duplicate", s)}
            disabled={anyBusy}
            title={t("servers.duplicate_server")}
            aria-label={t("servers.row_duplicate")}
          >
            <CopyPlus size={14} />
          </button>
          {/* 删除 */}
          <button
            className="btn-ghost p-1.5 text-err/60 hover:text-err disabled:opacity-40"
            onClick={() => onAction("delete", s)}
            disabled={anyBusy}
            title={t("common.delete")}
            aria-label={t("servers.row_delete")}
          >
            {isDeleting ? <Loader2 size={14} className="animate-spin text-err" /> : <Trash2 size={14} />}
          </button>
        </div>
      </td>
    </tr>
  );
});
