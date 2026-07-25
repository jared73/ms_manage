import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Pencil, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { EmptyHint } from "@/components/Section";
import { listProjects, upsertProject, deleteProject } from "@/lib/ipc";
import type { Project } from "@/lib/ipc";
import { Field } from "./types";
import { describeError } from "@/lib/errors";

/** 项目管理弹窗:列表 + 内嵌新建/编辑表单 + 删除确认。
 *
 * 设计:为了避免「ProjectsModal → Modal(编辑) → 提示框」三层 modal 嵌套,
 * 新建/编辑一律在主弹窗内的 ProjectForm 内联渲染(只允许一个二级 modal:
 * 删除确认)。这样既减少了用户的视觉负担,也避免 z-index 叠加造成的层级混乱。
 */
export function ProjectsModal({
  onClose,
  onChanged,
}: {
  onClose: () => void;
  onChanged: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [list, setList] = useState<Project[]>([]);
  // 编辑/新建共享一套表单状态 —— 用 editing 标志"修改现有",creating 标志"新建"
  const [editing, setEditing] = useState<Project | null>(null);
  const [creating, setCreating] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<Project | null>(null);
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());

  const load = async () => {
    try {
      setList(await listProjects());
    } catch (e) {
      toast.error(t("projects.load_failed", { msg: describeError(e) }) as string);
    }
  };
  useEffect(() => { void load(); }, []);

  const handleSave = async (p: Project) => {
    const name = p.name.trim();
    if (!name) {
      toast.error(t("projects.validation_name_required") as string);
      return;
    }
    setBusyIds((prev) => new Set(prev).add(p.id || "__new__"));
    try {
      await upsertProject(p.id ? p : { ...p, id: "" });
      setEditing(null);
      setCreating(false);
      await load();
      await onChanged();
      toast.success(t("projects.saved", { defaultValue: "项目已保存" }) as string, { duration: 1500 });
    } catch (e) {
      toast.error(t("projects.save_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusyIds((prev) => { const n = new Set(prev); n.delete(p.id || "__new__"); return n; });
    }
  };

  const handleDelete = async (p: Project) => {
    if (busyIds.has(p.id)) return;
    setBusyIds((prev) => new Set(prev).add(p.id));
    try {
      await deleteProject(p.id);
      await load();
      await onChanged();
      toast.success(t("projects.deleted", { name: p.name }) as string, { duration: 2000 });
      setConfirmDelete(null);
    } catch (e) {
      toast.error(t("projects.delete_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusyIds((prev) => { const n = new Set(prev); n.delete(p.id); return n; });
    }
  };

  const blank = (): Project => {
    const now = new Date().toISOString();
    return { id: "", name: "", description: "", created_at: now, updated_at: now };
  };

  const isBusy = (id: string) => busyIds.has(id) || busyIds.has("__new__");

  return (
    <>
      <Modal
        open
        size="lg"
        title={t("projects.title")}
        onClose={onClose}
        footer={
          <button
            className="btn-primary"
            onClick={() => { setCreating(true); setEditing(null); }}
            disabled={!!editing || creating}
          >
            <Plus size={14} />
            {t("common.add")}
          </button>
        }
      >
        <div className="space-y-3">
          {/* 内联编辑表单 —— 避免嵌套 modal */}
          {(editing || creating) && (
            <div className="rounded-md border border-accent/40 bg-accent/5 p-3">
              <ProjectForm
                key={editing?.id ?? "__new__"}
                initial={editing ?? blank()}
                busy={isBusy(editing?.id ?? "__new__")}
                onChange={(p) => {
                  if (editing) setEditing(p);
                  else setEditing(p);
                }}
                onCancel={() => { setEditing(null); setCreating(false); }}
                onSave={(p) => handleSave(p)}
              />
            </div>
          )}

          {/* 项目列表 —— 没有表单时显示,有表单时缩起 */}
          {!editing && !creating && (
            list.length === 0 ? (
              <EmptyHint>{t("common.empty")}</EmptyHint>
            ) : (
              <div className="space-y-2">
                {list.map((p) => (
                  <div
                    key={p.id}
                    className="flex items-center gap-2 rounded-md border border-border bg-bg-subtle px-3 py-2"
                  >
                    <div className="flex-1">
                      <div className="font-medium">{p.name}</div>
                      {p.description && <div className="text-xs text-fg-muted">{p.description}</div>}
                      {p.id === "__unassigned__" && (
                        <div className="text-xs text-fg-muted">{t("servers.projects_unassigned_hint")}</div>
                      )}
                    </div>
                    {p.id !== "__unassigned__" && (
                      <>
                        <button
                          className="btn-ghost p-1.5"
                          onClick={() => { setEditing(p); setCreating(false); }}
                          disabled={isBusy(p.id)}
                          title={t("common.edit")}
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          className="btn-ghost p-1.5 text-err disabled:opacity-50"
                          onClick={() => setConfirmDelete(p)}
                          disabled={isBusy(p.id)}
                          title={t("common.delete")}
                        >
                          {isBusy(p.id) ? (
                            <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-err border-t-transparent" />
                          ) : <Trash2 size={14} />}
                        </button>
                      </>
                    )}
                  </div>
                ))}
              </div>
            )
          )}
        </div>
      </Modal>

      {/* 删除确认 —— 唯一的二级 modal,防误删 */}
      {confirmDelete && (
        <Modal
          open
          size="sm"
          title={t("common.confirm")}
          onClose={() => { if (!isBusy(confirmDelete.id)) setConfirmDelete(null); }}
          footer={
            <>
              <button
                className="btn-ghost"
                onClick={() => setConfirmDelete(null)}
                disabled={isBusy(confirmDelete.id)}
              >
                {t("common.cancel")}
              </button>
              <button
                className="btn-primary bg-err hover:bg-err/80 text-bg disabled:opacity-60"
                onClick={() => handleDelete(confirmDelete)}
                disabled={isBusy(confirmDelete.id)}
              >
                {t("common.delete")}
              </button>
            </>
          }
        >
          <p className="text-sm">{t("projects.delete_confirm")}</p>
          <p className="mt-2 break-all font-mono text-xs text-fg-muted">{confirmDelete.name}</p>
        </Modal>
      )}
    </>
  );
}

function ProjectForm({
  initial,
  busy,
  onChange,
  onCancel,
  onSave,
}: {
  initial: Project;
  busy: boolean;
  onChange: (p: Project) => void;
  onCancel: () => void;
  onSave: (p: Project) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-3">
      <Field label={t("projects.name")} error={!initial.name.trim() ? undefined : undefined}>
        <input
          className="input"
          value={initial.name}
          onChange={(e) => onChange({ ...initial, name: e.target.value })}
          autoFocus
        />
      </Field>
      <Field label={t("projects.description")}>
        <input
          className="input"
          value={initial.description ?? ""}
          onChange={(e) => onChange({ ...initial, description: e.target.value })}
        />
      </Field>
      <div className="flex justify-end gap-2 pt-1">
        <button className="btn-ghost" onClick={onCancel} disabled={busy}>{t("common.cancel")}</button>
        <button className="btn-primary" onClick={() => onSave(initial)} disabled={busy || !initial.name.trim()}>
          {t("common.save")}
        </button>
      </div>
    </div>
  );
}
