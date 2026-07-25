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

/** 项目管理弹窗:列表 + 新建/编辑/删除。 */
export function ProjectsModal({
  onClose,
  onChanged,
}: {
  onClose: () => void;
  onChanged: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [list, setList] = useState<Project[]>([]);
  const [editing, setEditing] = useState<Project | null>(null);
  const [creating, setCreating] = useState(false);

  const load = async () => {
    try {
      setList(await listProjects());
    } catch (e) {
      toast.error(t("projects.load_failed", { msg: describeError(e) }) as string);
    }
  };
  useEffect(() => { void load(); }, []);

  const handleSave = async (p: Project) => {
    try {
      if (p.id && p.id !== "__unassigned__") {
        await upsertProject(p);
      } else {
        await upsertProject({ ...p, id: "" });
      }
      setEditing(null);
      setCreating(false);
      await load();
      await onChanged();
      toast.success(t("projects.saved", { defaultValue: "项目已保存" }) as string, { duration: 1500 });
    } catch (e) {
      toast.error(t("projects.save_failed", { msg: describeError(e) }) as string);
    }
  };

  const handleDelete = async (p: Project) => {
    try {
      await deleteProject(p.id);
      await load();
      await onChanged();
      toast.success(t("projects.deleted", { name: p.name }) as string, { duration: 2000 });
    } catch (e) {
      toast.error(t("projects.delete_failed", { msg: describeError(e) }) as string);
    }
  };

  const blank = (): Project => {
    const now = new Date().toISOString();
    return { id: "", name: "", description: "", created_at: now, updated_at: now };
  };

  return (
    <Modal
      open
      size="lg"
      title={t("projects.title")}
      onClose={onClose}
      footer={
        <button className="btn-primary" onClick={() => setCreating(true)}>
          <Plus size={14} />
          {t("common.add")}
        </button>
      }
    >
      <div className="space-y-2">
        {list.length === 0 ? (
          <EmptyHint>{t("common.empty")}</EmptyHint>
        ) : (
          list.map((p) => (
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
                  <button className="btn-ghost p-1.5" onClick={() => setEditing(p)}>
                    <Pencil size={14} />
                  </button>
                  <button
                    className="btn-ghost p-1.5 text-err"
                    onClick={() => handleDelete(p)}
                  >
                    <Trash2 size={14} />
                  </button>
                </>
              )}
            </div>
          ))
        )}
      </div>

      {(editing || creating) && (
        <Modal
          open
          size="md"
          title={editing?.name ? t("common.edit") : t("common.add")}
          onClose={() => {
            setEditing(null);
            setCreating(false);
          }}
          footer={
            <>
              <button
                className="btn-ghost"
                onClick={() => {
                  setEditing(null);
                  setCreating(false);
                }}
              >
                {t("common.cancel")}
              </button>
              <button
                className="btn-primary"
                onClick={() => handleSave(editing ?? blank())}
              >
                {t("common.save")}
              </button>
            </>
          }
        >
          <ProjectForm initial={editing ?? blank()} onChange={setEditing as (p: Project) => void} />
        </Modal>
      )}
    </Modal>
  );
}

function ProjectForm({
  initial,
  onChange,
}: {
  initial: Project;
  onChange: (p: Project) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-3">
      <Field label={t("projects.name")}>
        <input
          className="input"
          value={initial.name}
          onChange={(e) => onChange({ ...initial, name: e.target.value })}
        />
      </Field>
      <Field label={t("projects.description")}>
        <input
          className="input"
          value={initial.description ?? ""}
          onChange={(e) => onChange({ ...initial, description: e.target.value })}
        />
      </Field>
    </div>
  );
}
