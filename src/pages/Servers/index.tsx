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
  FileJson,
  Zap,
  RefreshCw,
} from "lucide-react";
import { toast } from "sonner";
import { Section, EmptyHint } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Modal } from "@/components/Modal";
import {
  listServers,
  listServerStates,
  listProjects,
  startServer,
  stopServer,
  testServer,
  refreshInstallStatus,
  deleteServer,
} from "@/lib/ipc";
import type { McpServer, ServerState, Project } from "@/lib/ipc";
import { ProjectsModal } from "./ProjectsModal";
import { ServerFormModal, blankServer } from "./ServerFormModal";
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

  // 安装进度 + 批量流程(独立 hook)
  const install = useInstallFlow();

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

  const needsInstall = (s: McpServer): boolean => s.needs_install === true;

  const handleDelete = async (s: McpServer) => {
    if (deletingIds.has(s.id)) return;
    setDeletingIds((prev) => new Set(prev).add(s.id));
    try {
      await deleteServer(s.id);
      await load();
      toast.success(t("servers.toast_deleted", { name: s.name }) as string, { duration: 2000 });
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
        case "export": exportSingleToJson(s); return;
        case "install": return install.installOne(s);
        case "uninstall": return install.uninstallOne(s);
        case "refresh": return handleRefreshInstall(s);
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

  /** 把单个 server 打成 mcpServers JSON,触发复制 + 下载。 */
  const exportSingleToJson = (s: McpServer) => {
    const mcpServers: Record<string, any> = {};
    if (s.transport === "http") {
      mcpServers[s.name] = { type: "http", url: s.url, headers: s.headers };
    } else if (s.transport === "sse") {
      mcpServers[s.name] = { type: "sse", url: s.url, headers: s.headers };
    } else {
      mcpServers[s.name] = { command: s.command, args: s.args, env: s.env, cwd: s.cwd };
    }
    const json = JSON.stringify({ mcpServers }, null, 2);
    navigator.clipboard.writeText(json).catch(() => {});
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${s.name}_mcp.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success(t("servers.toast_exported_one", { name: s.name }) as string);
  };

  /** 导出全部 enabled server。 */
  const exportToJson = async () => {
    const enabled = servers.filter((s) => s.enabled);
    const mcpServers: Record<string, any> = {};
    for (const s of enabled) {
      if (s.transport === "http") {
        mcpServers[s.name] = { type: "http", url: s.url, headers: s.headers };
      } else if (s.transport === "sse") {
        mcpServers[s.name] = { type: "sse", url: s.url, headers: s.headers };
      } else {
        mcpServers[s.name] = { command: s.command, args: s.args, env: s.env, cwd: s.cwd };
      }
    }
    const json = JSON.stringify({ mcpServers }, null, 2);
    try {
      await navigator.clipboard.writeText(json);
    } catch {
      // 同上
    }
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `mcp_servers_${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success(t("servers.toast_exported_all", { count: enabled.length }) as string);
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
        <div className="flex items-center gap-2">
          <input
            className="input max-w-sm"
            placeholder={t("common.search")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <select
            className="input max-w-[180px]"
            value={filterProject}
            onChange={(e) => setFilterProject(e.target.value)}
          >
            <option value="">{t("servers.projects_filter_all")}</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <span className="text-xs text-fg-muted">
            {filtered.length} / {servers.length}
          </span>
        </div>

        {install.batch && (
          <div className="rounded-md border border-accent/30 bg-accent/5 px-3 py-2 text-xs">
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
              {install.batch.done === install.batch.total && (
                <div className="flex items-center gap-3">
                  <span className="text-ok">✓ {install.batch.succeeded.length}</span>
                  {install.batch.failed.length > 0 && (
                    <span className="text-err">✗ {install.batch.failed.length}</span>
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
                className="h-full bg-accent transition-all"
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
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead className="bg-bg-subtle text-left text-xs uppercase text-fg-muted">
                <tr>
                  <th className="px-3 py-2 font-medium">{t("common.name")}</th>
                  <th className="px-3 py-2 font-medium">{t("common.transport")}</th>
                  <th className="px-3 py-2 font-medium">{t("projects.title")}</th>
                  <th className="px-3 py-2 font-medium">{t("common.status")}</th>
                  <th className="px-3 py-2 font-medium">{t("servers.col_pkg")}</th>
                  <th className="px-3 py-2 font-medium">{t("common.last_error")}</th>
                  <th className="px-3 py-2 font-medium text-right">{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody>
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
                    onAction={dispatchRow}
                  />
                ))}
              </tbody>
            </table>
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
          onImported={load}
        />
      )}

      {/* 删除确认 Modal —— 防误删 */}
      {confirmDelete && (
        <Modal
          open
          size="sm"
          title={t("common.confirm")}
          onClose={() => { if (!deletingIds.has(confirmDelete.id)) setConfirmDelete(null); }}
          footer={
            <>
              <button className="btn-ghost" onClick={() => setConfirmDelete(null)} disabled={deletingIds.has(confirmDelete.id)}>{t("common.cancel")}</button>
              <button
                className="btn-primary bg-err hover:bg-err/80 text-bg disabled:opacity-60"
                onClick={async () => {
                  const target = confirmDelete;
                  if (deletingIds.has(target.id)) return;
                  setConfirmDelete(null);
                  await handleDelete(target);
                }}
                disabled={deletingIds.has(confirmDelete.id)}
              >
                {deletingIds.has(confirmDelete.id) ? (
                  <Loader2 size={14} className="inline animate-spin mr-1" />
                ) : null}
                {t("common.delete")}
              </button>
            </>
          }
        >
          <p className="text-sm">{t("servers.delete_confirm")}</p>
          <p className="mt-2 break-all font-mono text-xs text-fg-muted">
            {confirmDelete.name}
            {confirmDelete.description ? ` — ${confirmDelete.description}` : ""}
          </p>
        </Modal>
      )}

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
                    <span className="badge-mute text-[10px]">{s.command}</span>
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
  | "export" | "install" | "uninstall" | "refresh";

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
  onAction,
}: ServerRowProps) {
  const { t } = useTranslation();
  const showInstall = s.needs_install === true;
  const startBlocked = showInstall && !s.installed;
  // 任意动作进行中(启动/停止/测试/删除),编辑/导出/探测等都要锁定
  const anyBusy = isStarting || isStopping || isProbing || isDeleting;

  return (
    <tr className="border-t border-border hover:bg-overlay-soft">
      <td className="px-3 py-2 cursor-pointer" onClick={() => onAction("view", s)} title={t("servers.view_action_title")}>
        <div className="font-medium">{s.name}</div>
        {s.description && <div className="text-xs text-fg-muted">{s.description}</div>}
      </td>
      <td className="px-3 py-2">
        <span className="badge-mute">{s.transport}</span>
      </td>
      <td className="px-3 py-2 text-xs text-fg-muted">{projectName}</td>
      <td className="px-3 py-2">
        {st ? <StatusBadge status={st.status} /> : <span className="text-fg-muted">—</span>}
      </td>
      <td className="px-3 py-2 text-xs">
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
              <span
                className="truncate text-[10px] text-fg-muted max-w-[180px]"
                title={s.install_error}
              >
                {s.install_error}
              </span>
            )}
            <div className="flex items-center gap-1">
              {s.installed ? (
                <>
                  <button
                    className="btn-ghost p-1 text-amber-400 hover:text-amber-300 disabled:opacity-40"
                    onClick={() => onAction("install", s)}
                    disabled={isInstalling}
                    title={t("servers.tooltip_upgrade")}
                  >
                    <RefreshCw size={12} />
                  </button>
                  <button
                    className="btn-ghost p-1 text-fg-muted hover:text-err disabled:opacity-40"
                    onClick={() => onAction("uninstall", s)}
                    disabled={isUninstalling}
                    title={t("servers.tooltip_uninstall")}
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
                >
                  <Download size={12} />
                  <span>{t("servers.install_btn_text_short", { defaultValue: "安装" })}</span>
                </button>
              )}
            </div>
          </div>
        )}
      </td>
      <td className="px-3 py-2 text-xs text-err/80 max-w-xs truncate">{st?.last_error ?? "—"}</td>
      <td className="px-3 py-2">
        <div className="flex justify-end gap-1">
          {st?.status === "running" ? (
            <button
              className="btn-ghost p-1.5 text-err hover:opacity-80 disabled:opacity-40 disabled:cursor-not-allowed"
              onClick={() => onAction("stop", s)}
              disabled={isStopping || isProbing || isDeleting}
              title={t("common.stop")}
            >
              {isStopping ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <Square size={14} />
              )}
            </button>
          ) : (
            <button
              className="btn-ghost p-1.5 text-ok disabled:opacity-40 disabled:cursor-not-allowed"
              onClick={() => onAction("start", s)}
              disabled={isStarting || startBlocked || isProbing || isDeleting}
              title={startBlocked ? t("servers.preinst_required_for_start") : t("common.start")}
            >
              {isStarting ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
            </button>
          )}
          <button
            className="btn-ghost p-1.5 text-amber-400 hover:text-amber-300 disabled:opacity-40 disabled:cursor-not-allowed"
            onClick={() => onAction("test", s)}
            disabled={anyBusy}
            title={t("common.test")}
          >
            {isProbing ? (
              <Loader2 size={14} className="animate-spin text-amber-400" />
            ) : (
              <TestTube2 size={14} />
            )}
          </button>
          <button
            className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-40 disabled:cursor-not-allowed"
            onClick={() => onAction("edit", s)}
            disabled={anyBusy}
            title={t("common.edit")}
          >
            <Pencil size={14} />
          </button>
          <button
            className="btn-ghost p-1.5 text-fg-muted hover:text-fg disabled:opacity-40 disabled:cursor-not-allowed"
            onClick={() => onAction("export", s)}
            disabled={anyBusy}
            title={t("servers.export_single_btn_title") as string}
          >
            <FileJson size={14} />
          </button>
          <button
            className="btn-ghost p-1.5 text-err hover:text-err/80 disabled:opacity-40 disabled:cursor-not-allowed"
            onClick={() => onAction("delete", s)}
            disabled={anyBusy}
            title={t("common.delete")}
          >
            {isDeleting ? (
              <Loader2 size={14} className="animate-spin text-err" />
            ) : (
              <Trash2 size={14} />
            )}
          </button>
        </div>
      </td>
    </tr>
  );
});
