import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Plus, Trash2, Pencil, Eye, EyeOff, FolderOpen, Link2, Plus as PlusIcon, Upload, FileSearch, File } from "lucide-react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { Section, EmptyHint } from "@/components/Section";
import { Modal } from "@/components/Modal";
import { ConfirmModal } from "@/components/ConfirmModal";
import { toast } from "sonner";
import {
  listSkills, upsertSkill, deleteSkill, toggleSkill, openDataDir,
  listSkillTargets, toggleSkillTarget, addSkillTarget, removeSkillTarget, updateSkillTarget,
  importSkillFromPath,
} from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import type { Skill, SkillTarget } from "@/lib/ipc";

export function Skills() {
  const { t } = useTranslation();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [editing, setEditing] = useState<Skill | null>(null);
  const [creating, setCreating] = useState(false);
  const [targets, setTargets] = useState<SkillTarget[]>(([]));
  const [addTargetOpen, setAddTargetOpen] = useState(false);
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  // 每行单独追踪 in-flight 操作,防止双击/重复请求导致竞态
  const [busyIds, setBusyIds] = useState<Record<string, "toggle" | "delete">>({});
  const markBusy = (id: string, kind: "toggle" | "delete") => setBusyIds((m) => ({ ...m, [id]: kind }));
  const clearBusy = (id: string) => setBusyIds((m) => { const { [id]: _, ...rest } = m; return rest; });

  const load = async () => {
    try {
      setSkills(await listSkills());
      setTargets(await listSkillTargets());
    } catch (e) {
      toast.error(t("skills.load_failed", { msg: describeError(e) }) as string);
    }
  };
  useEffect(() => { void load(); }, []);
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
      toast.success(t("skills.deleted", { name: s.name }) as string, { duration: 2000 });
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
      toast.success(t("skills.target_deleted", { name: tg.name }) as string, { duration: 2000 });
    } catch (e) {
      toast.error(t("skills.target_delete_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(`target:${tg.id}`);
    }
  };

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
            <button className="btn-ghost" onClick={() => setImportModalOpen(true)} title={t("skills.install_from_zip_tooltip") as string}>
              <Upload size={14}/>{t("skills.install_from_zip")}
            </button>
            <button className="btn-ghost" onClick={() => openDataDir()} title={t("skills.open_data_dir_tooltip") as string}>
              <FolderOpen size={14}/>{t("skills.data_dir_btn")}
            </button>
            <button className="btn-primary" onClick={() => setCreating(true)}><Plus size={14}/>{t("common.add")}</button>
          </div>
        }
      >
        {skills.length === 0 ? (
          <EmptyHint>{t("common.empty")}</EmptyHint>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {skills.map((s) => (
              <div key={s.id} className="card group">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-semibold">{s.name}</div>
                    {s.description && <div className="line-clamp-2 text-xs text-fg-muted">{s.description}</div>}
                    {s.files && s.files.length > 0 && (
                      <div className="mt-2 rounded-md bg-bg-subtle px-2 py-1.5 text-[11px] text-fg-muted">
                        <div className="mb-1 flex items-center gap-1 font-medium text-fg">
                          <File size={11}/> {t("skills.files_label", { count: s.files.length })}
                        </div>
                        <ul className="max-h-24 space-y-0.5 overflow-auto pl-1">
                          {s.files.map((f) => (
                            <li key={f} className="truncate font-mono text-[10px]">{f}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                  </div>
                  <span className={s.enabled ? "badge-ok" : "badge-mute"}>
                    {s.enabled ? t("common.enabled") : t("common.disabled")}
                  </span>
                </div>
                {s.enabled && (
                  <div className="mt-2 flex items-center gap-1 text-[11px] text-ok">
                    <Link2 size={11}/>{t("skills.symlinked_to", { list: targets.filter(t => t.enabled).map(t => t.name).join(" / ") || t("skills.no_target") })}
                  </div>
                )}
                <div className="mt-3 flex items-center justify-between">
                  <div className="text-[11px] text-fg-muted">{new Date(s.updated_at).toLocaleString()}</div>
                  <div className="flex gap-1">
                    {/* 启停：绿色 */}
                    <button
                      className="btn-ghost p-1.5 text-ok disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => handleToggle(s)}
                      disabled={!!busyIds[s.id]}
                      title={s.enabled ? t("common.disable") : t("common.enable")}
                    >
                      {busyIds[s.id] === "toggle" ? (
                        <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-ok border-t-transparent" />
                      ) : s.enabled ? <EyeOff size={14}/> : <Eye size={14}/>}
                    </button>
                    {/* 编辑：蓝色 */}
                    <button
                      className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => setEditing(s)}
                      disabled={!!busyIds[s.id]}
                    >
                      <Pencil size={14}/>
                    </button>
                    {/* 删除：红色 */}
                    <button
                      className="btn-ghost p-1.5 text-err hover:text-err/80 disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => setConfirmDelete(s)}
                      disabled={!!busyIds[s.id]}
                      title={t("common.delete")}
                    >
                      {busyIds[s.id] === "delete" ? (
                        <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-err border-t-transparent" />
                      ) : <Trash2 size={14}/>}
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
            <PlusIcon size={14}/>{t("skills.targets_add_btn")}
          </button>
        }
      >
        <div className="text-xs text-fg-muted">
          {t("skills.targets_desc")}
        </div>
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
              spellCheck={false}
            />
            <button
              className="absolute right-1 top-1/2 -translate-y-1/2 inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-fg-muted hover:bg-overlay-soft hover:text-fg transition-colors"
              onClick={pickFile}
              title={t("skills.import_modal_browse_title") as string}
              type="button"
            >
              <FileSearch size={13}/>{t("skills.import_modal_browse")}
            </button>
          </div>
          <p className="mt-1 text-[11px] text-fg-muted">
            {t("skills.import_modal_caption")}
          </p>
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
      <button
        onClick={handleToggle}
        role="switch"
        aria-checked={target.enabled}
        aria-label={`${target.name} ${target.enabled ? t("common.enabled") : t("common.disabled")}`}
        disabled={!!busy}
        aria-busy={!!busy}
        className={`relative h-5 w-9 shrink-0 rounded-full transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-60 disabled:cursor-not-allowed ${target.enabled ? "bg-accent" : "bg-overlay-soft"}`}
        title={target.enabled ? t("common.enabled") : t("common.disabled")}
      >
        <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-bg-subtle shadow-sm transition-all ${target.enabled ? "left-4" : "left-0.5"}`} />
      </button>
      <div className="w-32 shrink-0 text-sm font-medium">{target.name}</div>
      <code className="flex-1 truncate text-xs text-fg-muted">{target.path}</code>
      {target.builtin && <span className="badge-mute text-[10px]">{t("skills.targets_builtin_badge")}</span>}
      <button className="btn-ghost p-1.5 disabled:opacity-50 disabled:cursor-not-allowed" onClick={() => onAskEdit(target)} disabled={!!busy} title={t("skills.targets_edit_path_title")}><Pencil size={12}/></button>
      {!target.builtin && <button className="btn-ghost p-1.5 text-err disabled:opacity-50 disabled:cursor-not-allowed" onClick={() => onAskDelete(target)} disabled={!!busy} title={t("skills.targets_delete_title")}><Trash2 size={12}/></button>}
    </div>
  );
}

function AddTargetModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => Promise<void> }) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim() || !path.trim()) return;
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
          <input className="input w-full" placeholder={t("skills.add_target_name_placeholder") as string} value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div>
          <label className="mb-1 block text-sm text-fg-muted">{t("skills.add_target_path_label")}</label>
          <input className="input w-full font-mono text-xs" placeholder={t("skills.add_target_path_placeholder") as string} value={path} onChange={(e) => setPath(e.target.value)} />
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
      // 从 frontmatter 提取 name + description
      const m = content.match(/^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/);
      const front: Record<string, unknown> = {};
      if (m) {
        m[1].split(/\n/).forEach((line) => {
          const kv = line.match(/^([\w-]+)\s*:\s*(.*)$/);
          if (kv) {
            const v = kv[2].trim();
            front[kv[1]] = v.startsWith('"') && v.endsWith('"') ? v.slice(1, -1) : v;
          }
        });
      }
      const name = String(front.name ?? initial.name);
      const description = String(front.description ?? "");
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
