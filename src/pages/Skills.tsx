import { useMemo, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Plus, Trash2, Pencil, Eye, EyeOff, FolderOpen, Link2, Plus as PlusIcon, Upload, FileSearch, File, Search, LayoutGrid, List, Loader2, Copy, Sparkles, RefreshCw } from "lucide-react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { Section, EmptyHint } from "@/components/Section";
import { Modal } from "@/components/Modal";
import { ConfirmModal } from "@/components/ConfirmModal";
import { Toggle } from "@/components/Toggle";
import { toast } from "sonner";
import {
  listSkills, upsertSkill, deleteSkill, toggleSkill, openDataDir, openPath,
  listSkillTargets, toggleSkillTarget, addSkillTarget, removeSkillTarget, updateSkillTarget,
  importSkillFromPath, parseSkillFrontmatter,
  scanExternalSkills, importExternalSkills,
  analyzeSkill, analyzeAllSkills, listSkillAnalyses, getModelConfig,
} from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import type { Skill, SkillTarget, ExternalSkillDto, SkillAnalysis } from "@/lib/ipc";

export function Skills() {
  const { t } = useTranslation();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [editing, setEditing] = useState<Skill | null>(null);
  const [creating, setCreating] = useState(false);
  const [targets, setTargets] = useState<SkillTarget[]>(([]));
  const [addTargetOpen, setAddTargetOpen] = useState(false);
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [scanModalOpen, setScanModalOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  // 自由文本搜索:过滤 name/description,大小写不敏感。
  // Skills 数量一般 < 50,在前端过滤就够了;不需要走 IPC。
  const [search, setSearch] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [viewMode, setViewMode] = useState<"card" | "list">("card");
  // LLM 解析缓存:skill_id → SkillAnalysis。组件挂载时一次性加载。
  const [analyses, setAnalyses] = useState<Record<string, SkillAnalysis>>({});
  // 单 skill 解析中的 busy 集合,用于按钮 spinner
  const [analyzingIds, setAnalyzingIds] = useState<Set<string>>(new Set());
  // 全局更新进度
  const [analyzeProgress, setAnalyzeProgress] = useState<{ done: number; total: number; current: string } | null>(null);
  const [globalBusy, setGlobalBusy] = useState(false);
  const visibleSkills = useMemo(() => {
    if (!search.trim()) return skills;
    const q = search.toLowerCase();
    return skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        (s.description ?? "").toLowerCase().includes(q),
    );
  }, [skills, search]);
  // 每行单独追踪 in-flight 操作,防止双击/重复请求导致竞态
  const [busyIds, setBusyIds] = useState<Record<string, "toggle" | "delete">>({});
  const markBusy = (id: string, kind: "toggle" | "delete") => setBusyIds((m) => ({ ...m, [id]: kind }));
  const clearBusy = (id: string) => setBusyIds((m) => { const { [id]: _, ...rest } = m; return rest; });

  const load = async () => {
    try {
      const [ss, tgs, ans] = await Promise.all([listSkills(), listSkillTargets(), listSkillAnalyses()]);
      setSkills(ss);
      setTargets(tgs);
      setAnalyses(ans);
    } catch (e) {
      toast.error(t("skills.load_failed", { msg: describeError(e) }) as string);
    }
  };
  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-once 加载
  }, []);
  const [confirmDelete, setConfirmDelete] = useState<Skill | null>(null);
  const [confirmTargetDelete, setConfirmTargetDelete] = useState<SkillTarget | null>(null);
  const [editingTarget, setEditingTarget] = useState<SkillTarget | null>(null);

  // Tauri 拖拽支持：直接拿 OS 路径（HTML5 drag API 在 webview 里拿不到真路径）
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let aborted = false;
    (async () => {
      try {
        const webview = getCurrentWebview();
        const handle = await webview.onDragDropEvent((event) => {
          if (event.payload.type === "over") {
            setDragOver(true);
          } else if (event.payload.type === "leave") {
            setDragOver(false);
          } else if (event.payload.type === "drop") {
            setDragOver(false);
            const paths = event.payload.paths;
            void doImport(paths);
          }
        });
        // onDragDropEvent 是异步的 —— 组件可能在 await 期间卸载。
        // 如果已经 unmount,立即 detach 防止 listener 泄漏,避免下次挂载时重复注册。
        if (aborted) {
          handle();
          return;
        }
        unlisten = handle;
      } catch (e) {
        // 非 Tauri 环境（vite dev 时）忽略
        console.warn("drag-drop setup failed:", e);
      }
    })();
    return () => { aborted = true; unlisten?.(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-once 注册 Tauri 拖拽 listener
  }, []);

  const doImport = async (paths: string[]) => {
    let totalImported = 0;
    for (const p of paths) {
      try {
        const r = await importSkillFromPath(p);
        totalImported += r.imported.length;
        if (r.imported.length > 0) {
          toast.success(t("skills.import_success", { count: r.imported.length, list: r.imported.join(", ") }) as string);
        }
        if (r.errors.length > 0) {
          toast.error(t("skills.import_errors", { name: p, errors: r.errors.join("; ") }) as string);
        }
        if (r.skipped.length > 0) {
          toast(t("skills.import_skipped_msg", { path: p, details: r.skipped.join("; ") }) as string);
        }
      } catch (e) {
        toast.error(t("skills.import_errors", { name: p, errors: describeError(e) }) as string);
      }
    }
    if (totalImported > 0) await load();
  };

  const handleDelete = async (s: Skill) => {
    if (busyIds[s.id]) return;
    markBusy(s.id, "delete");
    try {
      await deleteSkill(s.id);
      await load();
      toast.success(t("skills.deleted", { name: s.name }) as string);
    } catch (e) {
      toast.error(t("skills.delete_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(s.id);
    }
  };

  const handleToggle = async (s: Skill) => {
    if (busyIds[s.id]) return;
    markBusy(s.id, "toggle");
    try {
      await toggleSkill(s.id, !s.enabled);
      await load();
    } catch (e) {
      toast.error(t("skills.toggle_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(s.id);
    }
  };

  const handleTargetDelete = async (tg: SkillTarget) => {
    if (busyIds[`target:${tg.id}`]) return;
    markBusy(`target:${tg.id}`, "delete");
    try {
      await removeSkillTarget(tg.id);
      await load();
      toast.success(t("skills.target_deleted", { name: tg.name }) as string);
    } catch (e) {
      toast.error(t("skills.target_delete_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(`target:${tg.id}`);
    }
  };

  // ---- Skill 快捷操作:打开目录 / 复制路径 ----
  // 替代内置代码编辑器 —— 大部分 skill 是网上下的,用户用 VSCode 等
  // 自己熟悉的工具改更高效。应用只负责"告诉你 path + 帮你一键到达"。
  // (2026-08-29:删除了 "在终端打开" 按钮 —— 用户觉得没用,且 Windows 下
  // cmd /c start 的引号解析在不同环境下行为不稳定)
  const handleOpenDir = async (s: Skill) => {
    if (!s.dir_path) {
      toast.error(t("skills.path_missing") as string);
      return;
    }
    try {
      await openPath(s.dir_path);
    } catch (e) {
      toast.error(describeError(e));
    }
  };
  const handleCopyPath = async (s: Skill) => {
    if (!s.dir_path) {
      toast.error(t("skills.path_missing") as string);
      return;
    }
    try {
      await navigator.clipboard.writeText(s.dir_path);
      toast.success(t("skills.skill_path_copied", { path: s.dir_path }) as string);
    } catch (e) {
      toast.error(describeError(e));
    }
  };

  // ---- LLM 解析 ----
  const handleAnalyzeOne = async (s: Skill) => {
    if (analyzingIds.has(s.id) || globalBusy) return;
    setAnalyzingIds((m) => new Set([...m, s.id]));
    try {
      const cfg = await getModelConfig();
      if (!cfg) {
        toast.error(t("skill_analyze.not_configured") as string);
        return;
      }
      const a = await analyzeSkill(s.id);
      setAnalyses((prev) => ({ ...prev, [s.id]: a }));
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setAnalyzingIds((m) => { const n = new Set(m); n.delete(s.id); return n; });
    }
  };
  const handleAnalyzeAll = async () => {
    if (globalBusy) return;
    setGlobalBusy(true);
    try {
      const cfg = await getModelConfig();
      if (!cfg) {
        toast.error(t("skill_analyze.not_configured") as string);
        return;
      }
      const count = await analyzeAllSkills();
      // 重新加载缓存拿到最新结果
      await load();
      toast.success(t("skill_analyze.global_done", { count }) as string);
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setGlobalBusy(false);
      setAnalyzeProgress(null);
    }
  };

  // 监听全局更新进度事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let aborted = false;
    (async () => {
      try {
        const u = await tauriListen<{ done: number; total: number; current: string }>(
          "skill_analysis_progress",
          (e) => setAnalyzeProgress(e.payload),
        );
        if (aborted) { u(); return; }
        unlisten = u;
      } catch { /* 非 Tauri 环境忽略 */ }
    })();
    return () => { aborted = true; unlisten?.(); };
  }, []);

  return (
    <div className="space-y-4 p-6 relative">
      {/* 拖拽时的全屏提示 */}
      {dragOver && (
        <div className="pointer-events-none fixed inset-0 z-40 grid place-items-center bg-accent/20 backdrop-blur-sm">
          <div className="rounded-lg border-2 border-dashed border-accent bg-bg/80 px-8 py-6 text-center">
            <Upload size={48} className="mx-auto text-accent" />
            <div className="mt-3 text-lg font-semibold">{t("skills.drag_drop_hint_title")}</div>
            <div className="mt-1 text-xs text-fg-muted">{t("skills.drag_drop_hint_subtitle")}</div>
          </div>
        </div>
      )}

      <Section
        title={t("skills.title")}
        action={
          <div className="flex gap-2">
            <div className="flex max-w-[220px]">
              <input
                type="search"
                className="input flex-1 rounded-r-none"
                placeholder={t("skills.search_placeholder") as string}
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") setSearch(searchInput); }}
                aria-label={t("skills.search_label") as string}
              />
              <button className="btn-primary rounded-l-none" onClick={() => setSearch(searchInput)} title={t("common.search")}>
                <Search size={14} />
              </button>
            </div>
            <div className="flex rounded-md border border-border">
              <button className={`px-2 py-1.5 rounded-l-md ${viewMode === "card" ? "bg-accent/15 text-accent" : "text-fg-muted hover:text-fg"}`} onClick={() => setViewMode("card")}>
                <LayoutGrid size={14} />
              </button>
              <button className={`px-2 py-1.5 rounded-r-md border-l border-border ${viewMode === "list" ? "bg-accent/15 text-accent" : "text-fg-muted hover:text-fg"}`} onClick={() => setViewMode("list")}>
                <List size={14} />
              </button>
            </div>
            <button className="btn-ghost" onClick={() => setImportModalOpen(true)} title={t("skills.import_tooltip") as string}>
              <Upload className="icon-sm"/>{t("skills.import_btn")}
            </button>
            <button className="btn-ghost" onClick={() => setScanModalOpen(true)} title={t("skills.scan_external_tooltip") as string}>
              <FileSearch className="icon-sm"/>{t("skills.scan_external_btn")}
            </button>
            <button
              className="btn-ghost"
              onClick={handleAnalyzeAll}
              disabled={globalBusy}
              title={t("skill_analyze.global_btn_tooltip") as string}
            >
              {globalBusy ? <Loader2 className="icon-sm animate-spin" /> : <Sparkles className="icon-sm" />}
              {globalBusy && analyzeProgress
                ? `${analyzeProgress.done}/${analyzeProgress.total}`
                : t("skill_analyze.global_btn")}
            </button>
            <button className="btn-ghost" onClick={() => openDataDir()} title={t("skills.open_data_dir_tooltip") as string}>
              <FolderOpen className="icon-sm"/>{t("skills.data_dir_btn")}
            </button>
            <button className="btn-primary" onClick={() => setCreating(true)}><Plus className="icon-sm"/>{t("common.add")}</button>
          </div>
        }
      >
        {skills.length === 0 ? (
          <EmptyHint>{t("common.empty")}</EmptyHint>
        ) : visibleSkills.length === 0 ? (
          <div className="rounded-md border border-border bg-bg-subtle p-6 text-center text-sm text-fg-muted">
            <Search className="icon-lg mx-auto mb-2 opacity-50" />
            {t("skills.no_match", { q: search })}
          </div>
        ) : viewMode === "list" ? (
          <div className="overflow-hidden rounded-lg border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-bg-subtle text-left text-xs uppercase text-fg-muted">
                  <th className="px-4 py-2 font-medium">{t("common.name")}</th>
                  <th className="px-4 py-2 font-medium">{t("common.status")}</th>
                  <th className="px-4 py-2 font-medium">{t("skills.files_label_short")}</th>
                  <th className="px-4 py-2 font-medium">{t("skills.updated_at")}</th>
                  <th className="px-4 py-2 text-right font-medium">{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {visibleSkills.map((s) => (
                  <tr key={s.id} className={`hover:bg-overlay-soft ${!s.enabled ? "opacity-50" : ""}`}>
                    <td className="px-4 py-2"><div className="truncate font-medium">{s.name}</div></td>
                    <td className="px-4 py-2"><span className={`inline-flex items-center gap-1 text-xs ${s.enabled ? "text-ok" : "text-fg-muted"}`}><span className={`inline-block h-1.5 w-1.5 rounded-full ${s.enabled ? "bg-ok" : "bg-fg-muted"}`} />{s.enabled ? t("common.enabled") : t("common.disabled")}</span></td>
                    <td className="px-4 py-2 text-xs text-fg-muted">{s.files?.length ?? "—"}</td>
                    <td className="px-4 py-2 text-xs text-fg-muted whitespace-nowrap">{new Date(s.updated_at).toLocaleDateString()}</td>
                    <td className="px-4 py-2"><div className="flex justify-end gap-1">
                      <button className="btn-ghost p-1.5 text-ok disabled:opacity-50" onClick={() => handleToggle(s)} disabled={!!busyIds[s.id]} title={s.enabled ? t("common.disable") : t("common.enable")}>{busyIds[s.id] === "toggle" ? <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-ok border-t-transparent" /> : s.enabled ? <EyeOff className="icon-sm"/> : <Eye className="icon-sm"/>}</button>
                      <button className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-50" onClick={() => setEditing(s)} disabled={!!busyIds[s.id]} title={t("common.edit")}><Pencil className="icon-sm"/></button>
                      <button className="btn-ghost p-1.5 text-err hover:text-err/80 disabled:opacity-50" onClick={() => setConfirmDelete(s)} disabled={!!busyIds[s.id]} title={t("common.delete")}>{busyIds[s.id] === "delete" ? <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-err border-t-transparent" /> : <Trash2 className="icon-sm"/>}</button>
                    </div></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {visibleSkills.map((s) => (
              <div key={s.id} className="card group">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-semibold">{s.name}</div>
                    {s.description && <div className="line-clamp-2 text-xs text-fg-muted">{s.description}</div>}
                    {analyses[s.id] && (
                      <div className="mt-1.5 rounded-md border border-accent/20 bg-accent/5 px-2 py-1.5 text-2xs">
                        <div className="mb-0.5 flex items-center gap-1 font-medium text-accent">
                          <Sparkles className="icon-xs" />
                          {t("skill_analyze.card_analyzed_label")}
                        </div>
                        <div className="text-fg">{analyses[s.id].summaryZh}</div>
                        <div className="mt-0.5 text-3xs text-fg-muted/70">
                          {t("skill_analyze.card_analyzed_by", {
                            model: analyses[s.id].model,
                            when: new Date(analyses[s.id].analyzedAt).toLocaleDateString(),
                          })}
                        </div>
                      </div>
                    )}
                    {s.files && s.files.length > 0 && (
                      <div className="mt-2 rounded-md bg-bg-subtle px-2 py-1.5 text-2xs text-fg-muted">
                        <div className="mb-1 flex items-center gap-1 font-medium text-fg">
                          <File className="icon-xs"/> {t("skills.files_label", { count: s.files.length })}
                        </div>
                        <ul className="max-h-24 space-y-0.5 overflow-auto pl-1">
                          {s.files.map((f) => (
                            <li key={f} className="truncate font-mono text-3xs">{f}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {/* 目录快捷操作:打开 / 复制路径 —— 替代内置编辑器,
                        大部分 skill 是网上下的,用户用 VSCode 等熟悉工具改更高效 */}
                    {s.dir_path && (
                      <div
                        className="mt-2 flex items-center gap-1 text-2xs"
                        title={t("skills.skill_path_tooltip", { path: s.dir_path }) as string}
                      >
                        <button
                          className="btn-ghost p-0.5 text-fg-muted hover:text-fg"
                          onClick={(e) => { e.preventDefault(); void handleOpenDir(s); }}
                          title={t("skills.skill_open_dir") as string}
                          aria-label={t("skills.skill_open_dir") as string}
                        >
                          <FolderOpen className="icon-xs" />
                        </button>
                        <code className="min-w-0 flex-1 truncate font-mono text-fg-muted">{s.dir_path}</code>
                        <button
                          className="btn-ghost p-0.5 text-fg-muted hover:text-fg"
                          onClick={(e) => { e.preventDefault(); void handleCopyPath(s); }}
                          title={t("skills.skill_copy_path") as string}
                          aria-label={t("skills.skill_copy_path") as string}
                        >
                          <Copy className="icon-xs" />
                        </button>
                      </div>
                    )}
                  </div>
                </div>
                {(() => {
                  const symNames = targets.filter(t => t.enabled).map(t => t.name);
                  const symDisplay = symNames.length > 2
                    ? symNames.slice(0, 2).join(" / ") + ` ... +${symNames.length - 2}`
                    : symNames.join(" / ") || null;
                  return symDisplay ? (
                    <div className="mt-2 flex items-center gap-1 text-2xs text-ok">
                      <Link2 className="icon-xs"/>{t("skills.symlinked_to", { list: symDisplay })}
                    </div>
                  ) : null;
                })()}
                <div className="mt-3 flex items-center justify-between">
                  <div className="text-2xs text-fg-muted">{new Date(s.updated_at).toLocaleString()}</div>
                  <div className="flex gap-1">
                    {/* 解析：紫色(用 LLM 给个中文摘要) */}
                    <button
                      className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => handleAnalyzeOne(s)}
                      disabled={analyzingIds.has(s.id) || globalBusy}
                      title={analyses[s.id] ? t("skill_analyze.refresh_one_tooltip") as string : t("skill_analyze.card_analyze_tooltip") as string}
                      aria-label={t("skill_analyze.card_analyze_btn") as string}
                    >
                      {analyzingIds.has(s.id) ? (
                        <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-accent border-t-transparent" />
                      ) : analyses[s.id] ? <RefreshCw className="icon-sm"/> : <Sparkles className="icon-sm"/>}
                    </button>
                    {/* 启停：绿色 */}
                    <button
                      className="btn-ghost p-1.5 text-ok disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => handleToggle(s)}
                      disabled={!!busyIds[s.id]}
                      title={s.enabled ? t("common.disable") : t("common.enable")}
                      aria-label={t("skills.card_toggle") as string}
                    >
                      {busyIds[s.id] === "toggle" ? (
                        <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-ok border-t-transparent" />
                      ) : s.enabled ? <EyeOff className="icon-sm"/> : <Eye className="icon-sm"/>}
                    </button>
                    {/* 编辑：蓝色 */}
                    <button
                      className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => setEditing(s)}
                      disabled={!!busyIds[s.id]}
                      title={t("common.edit")}
                      aria-label={t("skills.card_edit") as string}
                    >
                      <Pencil className="icon-sm"/>
                    </button>
                    {/* 删除：红色 */}
                    <button
                      className="btn-ghost p-1.5 text-err hover:text-err/80 disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => setConfirmDelete(s)}
                      disabled={!!busyIds[s.id]}
                      title={t("common.delete")}
                      aria-label={t("skills.card_delete") as string}
                    >
                      {busyIds[s.id] === "delete" ? (
                        <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-err border-t-transparent" />
                      ) : <Trash2 className="icon-sm"/>}
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </Section>

      {(editing || creating) && (
        <SkillFormModal
          initial={editing ?? blankSkill()}
          onClose={() => { setEditing(null); setCreating(false); }}
          onSaved={async () => { setEditing(null); setCreating(false); await load(); }}
        />
      )}

      {/* 集成目标管理 —— mcp-router 风格 */}
      <Section
        title={t("skills.targets_title")}
        action={
          <button className="btn-ghost" onClick={() => setAddTargetOpen(true)}>
            <PlusIcon className="icon-sm"/>{t("skills.targets_add_btn")}
          </button>
        }
      >
        <div className="text-xs text-fg-muted">
          {t("skills.targets_desc")}
        </div>
        {targets.length === 0 ? (
          <EmptyHint>{t("skills.targets_empty")}</EmptyHint>
        ) : (
          <div className="space-y-2">
            {targets.map((tg) => (
              <TargetRow
                key={tg.id}
                target={tg}
                onReload={load}
                onAskDelete={(t) => setConfirmTargetDelete(t)}
                onAskEdit={(t) => setEditingTarget(t)}
              />
            ))}
          </div>
        )}
      </Section>

      {addTargetOpen && (
        <AddTargetModal
          onClose={() => setAddTargetOpen(false)}
          onAdded={async () => { setAddTargetOpen(false); await load(); }}
        />
      )}

      {editingTarget && (
        <EditTargetPathModal
          target={editingTarget}
          onClose={() => setEditingTarget(null)}
          onSaved={async () => { setEditingTarget(null); await load(); }}
        />
      )}

      {importModalOpen && (
        <ImportModal
          onClose={() => setImportModalOpen(false)}
          onImported={async () => { setImportModalOpen(false); await load(); }}
        />
      )}
      {scanModalOpen && (
        <ScanExternalModal
          onClose={() => setScanModalOpen(false)}
          onImported={async () => { setScanModalOpen(false); await load(); }}
        />
      )}

      {/* 删除确认 Modal —— 用 ConfirmModal 抽取的共享组件 */}
      <ConfirmModal
        open={!!confirmDelete}
        title={t("common.confirm")}
        confirmLabel={t("common.delete")}
        busy={!!confirmDelete && !!busyIds[confirmDelete.id]}
        onClose={() => setConfirmDelete(null)}
        onConfirm={async () => {
          if (!confirmDelete) return;
          const target = confirmDelete;
          setConfirmDelete(null);
          await handleDelete(target);
        }}
      >
        <p className="text-sm">{t("skills.delete_confirm")}</p>
        {confirmDelete && (
          <p className="mt-2 break-all font-mono text-xs text-fg-muted">{confirmDelete.name}</p>
        )}
      </ConfirmModal>

      <ConfirmModal
        open={!!confirmTargetDelete}
        title={t("common.confirm")}
        confirmLabel={t("common.delete")}
        busy={!!confirmTargetDelete && !!busyIds[`target:${confirmTargetDelete.id}`]}
        onClose={() => setConfirmTargetDelete(null)}
        onConfirm={async () => {
          if (!confirmTargetDelete) return;
          const target = confirmTargetDelete;
          setConfirmTargetDelete(null);
          await handleTargetDelete(target);
        }}
      >
        <p className="text-sm">{t("skills.target_delete_confirm")}</p>
        {confirmTargetDelete && (
          <p className="mt-2 break-all font-mono text-xs text-fg-muted">
            {confirmTargetDelete.name} <br />
            <code>{confirmTargetDelete.path}</code>
          </p>
        )}
      </ConfirmModal>
    </div>
  );
}

function ImportModal({ onClose, onImported }: { onClose: () => void; onImported: () => Promise<void> }) {
  const { t } = useTranslation();
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  // 后端通过 `skill_import_progress` 事件实时推送当前阶段,
  // 让用户看到「解压中 → 扫描中 → 安装中 → 完成」而不是只有一个无意义的 spinner。
  const [stageMsg, setStageMsg] = useState<string>("");

  useEffect(() => {
    const unlisten = listenSkillImportProgress((p) => {
      // 在最后一次 `submit` 触发的导入完成前一直累积显示最新阶段信息
      if (p.stage === "done") setStageMsg(p.message);
      else setStageMsg(p.message);
    });
    return () => { void unlisten.then((fn) => fn()); };
  }, []);

  // 弹出原生文件选择对话框（zip / tar.gz / 目录 都能选）
  const pickFile = async () => {
    try {
      const selected = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          { name: t("skills.import_modal_filter_archives") as string, extensions: ["zip", "tar.gz", "tgz"] },
          { name: t("skills.import_modal_filter_all") as string, extensions: ["*"] },
        ],
        title: t("skills.import_modal_dialog_title") as string,
      });
      if (typeof selected === "string" && selected) setPath(selected);
    } catch (_e) {
      // 目录选择某些系统不支持 filters，先试目录
      try {
        const dir = await openDialog({ multiple: false, directory: true, title: t("skills.import_modal_dialog_title_dir") as string });
        if (typeof dir === "string" && dir) setPath(dir);
      } catch {}
    }
  };

  const submit = async () => {
    if (!path.trim()) return;
    setBusy(true);
    setStageMsg(t("skills.import_stage_start") as string);
    try {
      const r = await importSkillFromPath(path.trim());
      if (r.imported.length > 0) {
        toast.success(t("skills.import_success", { count: r.imported.length, list: r.imported.join(", ") }) as string);
        await onImported();
      } else if (r.errors.length > 0) {
        toast.error(r.errors.join("; "));
      } else if (r.skipped.length > 0) {
        toast(`${path}: ${r.skipped.join("; ")}`);
      }
    } catch (e) {
      toast.error(t("skills.import_fail_prefix") as string + describeError(e));
    } finally {
      setBusy(false);
      setStageMsg("");
    }
  };
  // Enter 直接提交 —— textarea(Skill 编辑器那种)不需要,但普通 input 接 Enter 能省一次鼠标移动。
  const onInputKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  };
  return (
    <Modal
      open
      title={t("skills.import_modal_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>{t("common.cancel")}</button>
          <button className="btn-primary" onClick={submit} disabled={!path.trim() || busy}>
            {busy ? t("skills.import_modal_busy") : t("skills.import_modal_btn")}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="rounded-md border border-border bg-bg-subtle p-3 text-xs text-fg-muted"
          dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(t("skills.import_modal_hint") as string) }}
        />
        <div>
          <label className="mb-1 block text-sm text-fg-muted">{t("skills.target_add_placeholder_path")}</label>
          <div className="relative">
            <input
              className="input w-full pr-24 font-mono text-xs"
              placeholder={t("skills.import_modal_placeholder") as string}
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={onInputKeyDown}
              spellCheck={false}
            />
            <button
              className="absolute right-1 top-1/2 -translate-y-1/2 inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-fg-muted hover:bg-overlay-soft hover:text-fg transition-colors"
              onClick={pickFile}
              title={t("skills.import_modal_browse_title") as string}
              type="button"
            >
              <FileSearch className="icon-sm"/>{t("skills.import_modal_browse")}
            </button>
          </div>
          <p className="mt-1 text-2xs text-fg-muted">
            {t("skills.import_modal_caption")}
          </p>
          {busy && stageMsg && (
            <div
              role="status"
              aria-live="polite"
              className="mt-2 inline-flex items-center gap-2 rounded bg-accent/10 px-2 py-1 text-2xs text-accent"
            >
              <FileSearch className="icon-xs animate-pulse" />
              {stageMsg}
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}

function TargetRow({ target, onReload, onAskDelete, onAskEdit }: { target: SkillTarget; onReload: () => Promise<void>; onAskDelete: (t: SkillTarget) => void; onAskEdit: (t: SkillTarget) => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState<"toggle" | null>(null);
  const handleToggle = async () => {
    if (busy) return;
    setBusy("toggle");
    try { await toggleSkillTarget(target.id, !target.enabled); await onReload(); }
    catch (e) { toast.error(t("skills.toggle_failed", { msg: describeError(e) }) as string); }
    finally { setBusy(null); }
  };
  return (
    <div className="flex items-center gap-2 rounded-md bg-bg-subtle px-3 py-2">
      {/* toggle */}
      <Toggle
        checked={target.enabled}
        onChange={() => handleToggle()}
        label={`${target.name} ${target.enabled ? t("common.enabled") : t("common.disabled")}`}
        busy={!!busy}
        disabled={!!busy}
      />
      <div className="w-32 shrink-0 text-sm font-medium">{target.name}</div>
      <code className="flex-1 truncate text-xs text-fg-muted">{target.path}</code>
      {target.builtin && <span className="badge-mute text-3xs">{t("skills.targets_builtin_badge")}</span>}
      <button className="btn-ghost p-1.5 disabled:opacity-50 disabled:cursor-not-allowed" onClick={() => onAskEdit(target)} disabled={!!busy} title={t("skills.targets_edit_path_title")}><Pencil className="icon-xs"/></button>
      {!target.builtin && <button className="btn-ghost p-1.5 text-err disabled:opacity-50 disabled:cursor-not-allowed" onClick={() => onAskDelete(target)} disabled={!!busy} title={t("skills.targets_delete_title")}><Trash2 className="icon-xs"/></button>}
    </div>
  );
}

function AddTargetModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => Promise<void> }) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    // 空字段给个 toast,不再静默吞 —— 用户点击却没反应最容易让人怀疑应用坏了
    if (!name.trim() || !path.trim()) {
      toast.error(t("skills.target_add_required") as string);
      return;
    }
    setBusy(true);
    try {
      await addSkillTarget(name.trim(), path.trim());
      toast.success(t("skills.target_added", { name }) as string);
      await onAdded();
    } catch (e) {
      toast.error(t("skills.target_add_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusy(false);
    }
  };
  // Enter 直接提交 —— 省一次鼠标移到「添加」按钮
  const onInputKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  };

  return (
    <Modal
      open
      title={t("skills.add_target_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>{t("common.cancel")}</button>
          <button className="btn-primary" onClick={submit} disabled={!name.trim() || !path.trim() || busy}>
            {t("skills.add_target_btn")}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div>
          <label className="mb-1 block text-sm text-fg-muted">{t("skills.target_add_placeholder_name")}</label>
          <input className="input w-full" placeholder={t("skills.add_target_name_placeholder") as string} value={name} onChange={(e) => setName(e.target.value)} onKeyDown={onInputKeyDown} />
        </div>
        <div>
          <label className="mb-1 block text-sm text-fg-muted">{t("skills.add_target_path_label")}</label>
          <input className="input w-full font-mono text-xs" placeholder={t("skills.add_target_path_placeholder") as string} value={path} onChange={(e) => setPath(e.target.value)} onKeyDown={onInputKeyDown} />
          <p className="mt-1 text-xs text-fg-muted">
            {t("skills.add_target_path_hint")}
          </p>
        </div>
      </div>
    </Modal>
  );
}

function EditTargetPathModal({ target, onClose, onSaved }: {
  target: SkillTarget;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [path, setPath] = useState(target.path);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const trimmed = path.trim();
    if (!trimmed || trimmed === target.path) {
      onClose();
      return;
    }
    setBusy(true);
    try {
      await updateSkillTarget(target.id, undefined, trimmed);
      toast.success(t("skills.target_updated") as string);
      await onSaved();
    } catch (e) {
      toast.error(t("skills.target_update_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusy(false);
    }
  };
  // Enter 直接提交 —— 单输入框,无多行场景,无需 shift 判断
  const onInputKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  };

  return (
    <Modal
      open
      size="sm"
      title={t("skills.targets_edit_path_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>{t("common.cancel")}</button>
          <button className="btn-primary" onClick={submit} disabled={!path.trim() || busy}>
            {t("common.save")}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div>
          <div className="mb-1 text-sm text-fg-muted">{target.name}</div>
          <label className="mb-1 block text-sm text-fg-muted">{t("skills.target_add_placeholder_path")}</label>
          <input
            className="input w-full font-mono text-xs"
            placeholder={t("skills.add_target_path_placeholder") as string}
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={onInputKeyDown}
            spellCheck={false}
          />
          <p className="mt-1 text-xs text-fg-muted">
            {t("skills.add_target_path_hint")}
          </p>
        </div>
      </div>
    </Modal>
  );
}

function blankSkill(): Skill {
  const now = new Date().toISOString();
  const name = "new-skill";
  return {
    id: crypto.randomUUID(),
    name,
    description: "",
    content: `---\nname: ${name}\ndescription: 在此写 Skill 的简要描述\n---\n\n# ${name}\n\n在此编写 Skill 的 Markdown 内容。\n`,
    frontmatter: { name, description: "" },
    dir_path: "",
    enabled: true,
    created_at: now,
    updated_at: now,
  };
}

function SkillFormModal({ initial, onClose, onSaved }: {
  initial: Skill;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [content, setContent] = useState(initial.content);
  const [tab, setTab] = useState<"edit" | "preview">("edit");

  const submit = async () => {
    try {
      // 调用后端 serde_yaml 解析 frontmatter —— 之前用 JS regex 解析
      // "Use this: for code review" 这种含 `:` 的 description 会被截断到 "Use this"。
      const parsed = await parseSkillFrontmatter(content);
      const front = (parsed.front ?? {}) as Record<string, unknown>;
      const name = parsed.name ?? initial.name;
      const description = parsed.description ?? "";
      await upsertSkill({
        ...initial,
        name,
        description,
        content,
        frontmatter: front,
        updated_at: new Date().toISOString(),
      });
      await onSaved();
    } catch (e) {
      toast.error(t("skills.save_failed", { msg: describeError(e) }) as string);
    }
  };

  return (
    <Modal
      open
      size="xl"
      title={initial.content ? t("skills.edit_title") : t("skills.add_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose}>{t("common.cancel")}</button>
          <button className="btn-primary" onClick={submit}>{t("common.save")}</button>
        </>
      }
    >
      <div className="mb-2 flex gap-1">
        <button className={tab === "edit" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("edit")}>{t("skills.raw")}</button>
        <button className={tab === "preview" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("preview")}>{t("skills.preview")}</button>
      </div>
      {tab === "edit" ? (
        <textarea
          className="input min-h-[60vh] font-mono text-sm"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={t("skills.content_placeholder") as string}
        />
      ) : (
        <div
          className="prose prose-invert max-h-[60vh] max-w-none overflow-auto rounded-md border border-border bg-bg-subtle p-4 [&_*]:text-fg [&_code]:text-accent [&_a]:text-accent"
          dangerouslySetInnerHTML={{
            // skill 内容可从 ZIP/GitHub 第三方导入，必须消毒：
            // 防 markdown 里的 <img onerror> / <a href="javascript:"> 在 webview 执行。
            __html: DOMPurify.sanitize(
              marked.parse(content.replace(/^---[\s\S]*?---\n/, "")) as string,
            ),
          }}
        />
      )}
      <div className="mt-2 text-xs text-fg-muted">{t("skills.frontmatter_hint")}</div>
    </Modal>
  );
}

/** 后端 import_skill_from_path 通过 skill_import_progress 事件推送阶段信息。
 * 返回 unlisten 的 Promise —— 调用方负责 await + cleanup。 */
function listenSkillImportProgress(
  onMsg: (p: { path: string; stage: string; message: string }) => void,
): Promise<UnlistenFn> {
  return tauriListen<{ path: string; stage: string; message: string }>(
    "skill_import_progress",
    (e) => onMsg(e.payload),
  );
}

/**
 * 把 ISO 时间格式化成"3 天前 / 2 小时前"形式。
 * 用 Intl.RelativeTimeFormat 自动跟随 i18n 语言（zh → "3 天前",en → "3 days ago"），
 * 不用再为时间单位加 i18n key。
 */
function formatRelativeTime(iso: string, lang: string): string {
  const then = new Date(iso).getTime();
  const diffSec = Math.round((then - Date.now()) / 1000);
  const abs = Math.abs(diffSec);
  const rtf = new Intl.RelativeTimeFormat(lang || undefined, { numeric: "auto" });
  if (abs < 60) return rtf.format(diffSec, "second");
  if (abs < 3600) return rtf.format(Math.round(diffSec / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diffSec / 3600), "hour");
  if (abs < 86400 * 30) return rtf.format(Math.round(diffSec / 86400), "day");
  if (abs < 86400 * 365) return rtf.format(Math.round(diffSec / (86400 * 30)), "month");
  return rtf.format(Math.round(diffSec / (86400 * 365)), "year");
}

/** 扫描外部 Skill 弹窗：列出 Claude/opencode/Cursor 等目录里已有的 skill，勾选导入 */
function ScanExternalModal({ onClose, onImported }: { onClose: () => void; onImported: () => Promise<void> }) {
  const { t, i18n } = useTranslation();
  const [results, setResults] = useState<ExternalSkillDto[] | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const r = await scanExternalSkills();
        if (cancelled) return;
        // 排序：未导入过的（新的）排在前面,已存在的沉底 —— 让用户一眼看到
        // "哪些是我没拿过的",而不是被一堆已导入的刷屏。Array.sort 是 stable
        // （ES2019+）,同名/同 exists 的保留相对顺序,不会打乱同名不同源的展示。
        const sorted = [...r].sort((a, b) => Number(a.exists) - Number(b.exists));
        setResults(sorted);
        // 默认勾选所有"新发现"（不勾已存在）
        setChecked(new Set(sorted.filter((x) => !x.exists).map((x) => x.path)));
      } catch (e) {
        toast.error(describeError(e));
        if (!cancelled) onClose();
      }
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggle = (path: string) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const doImport = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const selections = (results ?? [])
        .filter((r) => checked.has(r.path))
        .map((r) => ({ path: r.path, name: r.name, overwrite: r.exists }));
      if (selections.length === 0) { setBusy(false); return; }
      const r = await importExternalSkills(selections);
      if (r.errors.length > 0) {
        toast.error(`${t("skills.scan_import_partial")}: ${r.errors.join("; ")}`);
      } else {
        toast.success(t("skills.scan_imported", { n: r.imported.length }));
      }
      await onImported();
    } catch (e) {
      toast.error(describeError(e));
      setBusy(false);
    }
  };

  return (
    <Modal open title={t("skills.scan_external_title")} onClose={onClose}>
      <div className="min-w-[480px] max-w-[640px]">
        {results === null ? (
          <div className="flex items-center gap-2 py-8 text-sm text-fg-muted">
            <Loader2 className="icon-sm animate-spin" />
            {t("skills.scan_scanning")}
          </div>
        ) : results.length === 0 ? (
          <div className="py-8 text-center text-sm text-fg-muted">
            <FileSearch className="icon-lg mx-auto mb-2 opacity-50" />
            {t("skills.scan_empty")}
          </div>
        ) : (
          <>
            <div className="max-h-[50vh] space-y-1 overflow-auto">
              {results.map((r) => (
                <label
                  key={r.path}
                  className="flex cursor-pointer items-start gap-2.5 rounded-md border border-border bg-bg-subtle px-3 py-2 hover:bg-overlay-soft"
                >
                  <input
                    type="checkbox"
                    checked={checked.has(r.path)}
                    onChange={() => toggle(r.path)}
                    className="mt-0.5 accent-accent"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 text-sm">
                      <span className="truncate font-medium">{r.name}</span>
                      {r.exists && (
                        <span className="shrink-0 rounded bg-warn/15 px-1.5 py-0.5 text-2xs text-warn">
                          {t("skills.scan_exists")}
                        </span>
                      )}
                    </div>
                    {r.description && (
                      <div className="mt-0.5 truncate text-xs text-fg-muted">{r.description}</div>
                    )}
                    <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-2xs text-fg-muted/70">
                      <span className="font-mono">{r.source}</span>
                      {r.fileCount > 0 && (
                        <span title={t("skills.scan_files_count", { count: r.fileCount }) as string}>
                          {t("skills.scan_files_count", { count: r.fileCount })}
                        </span>
                      )}
                      {r.lastModified ? (
                        <span title={new Date(r.lastModified).toLocaleString()}>
                          {t("skills.scan_modified", { when: formatRelativeTime(r.lastModified, i18n.language) })}
                        </span>
                      ) : (
                        <span>{t("skills.scan_modified_unknown")}</span>
                      )}
                      {r.pathTail && (
                        <span
                          className="truncate font-mono opacity-70"
                          title={t("skills.scan_path_tooltip", { path: r.path }) as string}
                        >
                          {r.pathTail}
                        </span>
                      )}
                    </div>
                  </div>
                </label>
              ))}
            </div>
            <div className="mt-3 flex items-center justify-end gap-2">
              <span className="mr-auto text-xs text-fg-muted">
                {t("skills.scan_selected", { n: checked.size })}
              </span>
              <button className="btn-ghost" onClick={onClose}>{t("common.cancel")}</button>
              <button className="btn-primary" onClick={doImport} disabled={busy || checked.size === 0}>
                {busy ? <Loader2 className="icon-sm animate-spin" /> : <Plus className="icon-sm" />}
                {t("skills.scan_import_btn", { n: checked.size })}
              </button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
