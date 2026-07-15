import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Plus, Trash2, Pencil, Eye, EyeOff, FolderOpen, Link2, Plus as PlusIcon, Upload, FileSearch, File } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import { Modal } from "@/components/Modal";
import { toast } from "sonner";
import {
  listSkills, upsertSkill, deleteSkill, toggleSkill, openDataDir,
  listSkillTargets, toggleSkillTarget, addSkillTarget, removeSkillTarget, updateSkillTarget,
  importSkillFromPath,
} from "@/lib/ipc";
import type { Skill, SkillTarget } from "@/lib/ipc";

export function Skills() {
  const { t } = useTranslation();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [editing, setEditing] = useState<Skill | null>(null);
  const [creating, setCreating] = useState(false);
  const [targets, setTargets] = useState<SkillTarget[]>([]);
  const [addTargetOpen, setAddTargetOpen] = useState(false);
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  const load = async () => {
    setSkills(await listSkills());
    setTargets(await listSkillTargets());
  };
  useEffect(() => { void load(); }, []);

  // Tauri 拖拽支持：直接拿 OS 路径（HTML5 drag API 在 webview 里拿不到真路径）
  useEffect(() => {
    let unlisten: (() => void) | undefined;
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
        unlisten = handle;
      } catch (e) {
        // 非 Tauri 环境（vite dev 时）忽略
        console.warn("drag-drop setup failed:", e);
      }
    })();
    return () => { unlisten?.(); };
  }, []);

  const doImport = async (paths: string[]) => {
    let totalImported = 0;
    let totalErrors = 0;
    for (const p of paths) {
      try {
        const r = await importSkillFromPath(p);
        totalImported += r.imported.length;
        totalErrors += r.errors.length + r.skipped.length;
        if (r.imported.length > 0) {
          toast.success(`已导入 ${r.imported.length} 个 skill（${r.imported.join(", ")}）`);
        }
        if (r.errors.length > 0) {
          toast.error(`${p}: ${r.errors.join("; ")}`);
        }
        if (r.skipped.length > 0) {
          toast(`${p}: ${r.skipped.join("; ")}`);
        }
      } catch (e) {
        totalErrors++;
        toast.error(`${p}: ${e}`);
      }
    }
    if (totalImported > 0) await load();
  };

  const handleDelete = async (s: Skill) => {
    try {
      await deleteSkill(s.id);
      await load();
      toast.success(`已删除 "${s.name}"`, { duration: 2000 });
    } catch (e) {
      toast.error(`删除失败: ${e}`);
    }
  };

  const handleToggle = async (s: Skill) => {
    try { await toggleSkill(s.id, !s.enabled); await load(); }
    catch (e) { toast.error(`切换失败: ${e}`); }
  };

  return (
    <div className="space-y-4 p-6 relative">
      {/* 拖拽时的全屏提示 */}
      {dragOver && (
        <div className="pointer-events-none fixed inset-0 z-40 grid place-items-center bg-accent/20 backdrop-blur-sm">
          <div className="rounded-lg border-2 border-dashed border-accent bg-bg/80 px-8 py-6 text-center">
            <Upload size={48} className="mx-auto text-accent" />
            <div className="mt-3 text-lg font-semibold">松开导入 skill</div>
            <div className="mt-1 text-xs text-fg-muted">支持 .zip / .tar.gz / 文件夹（含 SKILL.md）</div>
          </div>
        </div>
      )}

      <Section
        title={t("skills.title")}
        action={
          <div className="flex gap-2">
            <button className="btn-ghost" onClick={() => setImportModalOpen(true)} title="从 ZIP / 压缩包安装完整 skill">
              <Upload size={14}/>从 ZIP 安装
            </button>
            <button className="btn-ghost" onClick={() => openDataDir()} title="打开数据目录">
              <FolderOpen size={14}/>数据目录
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
                          <File size={11}/> 文件列表 ({s.files.length})
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
                    <Link2 size={11}/>已 symlink 到 {targets.filter(t => t.enabled).map(t => t.name).join(" / ") || "（无目标）"}
                  </div>
                )}
                <div className="mt-3 flex items-center justify-between">
                  <div className="text-[11px] text-fg-muted">{new Date(s.updated_at).toLocaleString()}</div>
                  <div className="flex gap-1">
                    {/* 启停：绿色 */}
                    <button className="btn-ghost p-1.5 text-ok" onClick={() => handleToggle(s)} title={s.enabled ? t("common.disable") : t("common.enable")}>
                      {s.enabled ? <EyeOff size={14}/> : <Eye size={14}/>}
                    </button>
                    {/* 编辑：蓝色 */}
                    <button className="btn-ghost p-1.5 text-accent hover:opacity-80" onClick={() => setEditing(s)}><Pencil size={14}/></button>
                    {/* 删除：红色 */}
                    <button className="btn-ghost p-1.5 text-err hover:text-err/80" onClick={() => handleDelete(s)}><Trash2 size={14}/></button>
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
        title="集成目标"
        action={
          <button className="btn-ghost" onClick={() => setAddTargetOpen(true)}>
            <PlusIcon size={14}/>添加
          </button>
        }
      >
        <div className="text-xs text-fg-muted">
          启用的 skill 会 symlink 到下面所有开启的目标目录。点开关切换，点 ✏ 改路径，点 🗑 删自定义目标（预设不能删）。
        </div>
        <div className="space-y-2">
          {targets.map((tg) => (
            <TargetRow key={tg.id} target={tg} onReload={load} />
          ))}
        </div>
      </Section>

      {addTargetOpen && (
        <AddTargetModal
          onClose={() => setAddTargetOpen(false)}
          onAdded={async () => { setAddTargetOpen(false); await load(); }}
        />
      )}

      {importModalOpen && (
        <ImportModal
          onClose={() => setImportModalOpen(false)}
          onImported={async () => { setImportModalOpen(false); await load(); }}
        />
      )}
    </div>
  );
}

function ImportModal({ onClose, onImported }: { onClose: () => void; onImported: () => Promise<void> }) {
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  // 弹出原生文件选择对话框（zip / tar.gz / 目录 都能选）
  const pickFile = async () => {
    try {
      const selected = await openDialog({
        multiple: false,
        directory: false,
        filters: [
          { name: "Skill 压缩包 / 文件夹", extensions: ["zip", "tar.gz", "tgz"] },
          { name: "所有文件", extensions: ["*"] },
        ],
        title: "选择 skill 压缩包或文件夹",
      });
      if (typeof selected === "string" && selected) setPath(selected);
    } catch (e) {
      // 目录选择某些系统不支持 filters，先试目录
      try {
        const dir = await openDialog({ multiple: false, directory: true, title: "选择 skill 文件夹" });
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
        toast.success(`已导入 ${r.imported.length} 个 skill：${r.imported.join(", ")}`);
        await onImported();
      } else if (r.errors.length > 0) {
        toast.error(r.errors.join("; "));
      } else if (r.skipped.length > 0) {
        toast(`${path}: ${r.skipped.join("; ")}`);
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open
      title="从压缩包 / 文件夹导入 skill"
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>取消</button>
          <button className="btn-primary" onClick={submit} disabled={!path.trim() || busy}>
            {busy ? "导入中..." : "导入"}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="rounded-md border border-border bg-bg-subtle p-3 text-xs text-fg-muted">
          支持 <code className="text-fg">.zip</code> / <code className="text-fg">.tar.gz</code> / <code className="text-fg">.tgz</code> 压缩包，
          或包含 <code className="text-fg">SKILL.md</code> 的文件夹路径。压缩包/文件夹顶层可有多个 skill 子目录，逐个识别。
          或者直接拖文件到页面也可以。
        </div>
        <div>
          <label className="mb-1 block text-sm text-fg-muted">路径</label>
          <div className="relative">
            <input
              className="input w-full pr-24 font-mono text-xs"
              placeholder="拖文件进来，或点右侧浏览…"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              spellCheck={false}
            />
            <button
              className="absolute right-1 top-1/2 -translate-y-1/2 inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-fg-muted hover:bg-overlay-soft hover:text-fg transition-colors"
              onClick={pickFile}
              title="打开文件选择对话框"
              type="button"
            >
              <FileSearch size={13}/>浏览
            </button>
          </div>
          <p className="mt-1 text-[11px] text-fg-muted">
            支持 .zip / .tar.gz / .tgz / 整个文件夹；压缩包/文件夹里任意层级的 SKILL.md 都会被找到
          </p>
        </div>
      </div>
    </Modal>
  );
}

function TargetRow({ target, onReload }: { target: SkillTarget; onReload: () => Promise<void> }) {
  const handleToggle = async () => {
    try { await toggleSkillTarget(target.id, !target.enabled); await onReload(); }
    catch (e) { toast.error(`切换失败: ${e}`); }
  };
  const handleDelete = async () => {
    try {
      await removeSkillTarget(target.id);
      await onReload();
      toast.success(`已删除目标 "${target.name}"`, { duration: 2000 });
    } catch (e) {
      toast.error(`删除失败: ${e}`);
    }
  };
  const handleEditPath = async () => {
    const newPath = window.prompt(`修改 "${target.name}" 的路径：`, target.path);
    if (!newPath || newPath === target.path) return;
    try { await updateSkillTarget(target.id, undefined, newPath); await onReload(); toast.success("已更新"); }
    catch (e) { toast.error(`更新失败: ${e}`); }
  };
  return (
    <div className="flex items-center gap-2 rounded-md bg-bg-subtle px-3 py-2">
      {/* toggle */}
      <button
        onClick={handleToggle}
        className={`relative h-5 w-9 shrink-0 rounded-full transition-colors ${target.enabled ? "bg-accent" : "bg-overlay-soft"}`}
        title={target.enabled ? "已启用，点此关闭" : "已关闭，点此启用"}
      >
        <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-bg-subtle shadow-sm transition-all ${target.enabled ? "left-4" : "left-0.5"}`} />
      </button>
      <div className="w-32 shrink-0 text-sm font-medium">{target.name}</div>
      <code className="flex-1 truncate text-xs text-fg-muted">{target.path}</code>
      {target.builtin && <span className="badge-mute text-[10px]">预设</span>}
      <button className="btn-ghost p-1.5" onClick={handleEditPath} title="修改路径"><Pencil size={12}/></button>
      {!target.builtin && <button className="btn-ghost p-1.5 text-err" onClick={handleDelete} title="删除"><Trash2 size={12}/></button>}
    </div>
  );
}

function AddTargetModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => Promise<void> }) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim() || !path.trim()) return;
    setBusy(true);
    try {
      await addSkillTarget(name.trim(), path.trim());
      toast.success(`已添加 "${name}"`);
      await onAdded();
    } catch (e) {
      toast.error(`添加失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      title="添加集成目标"
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>取消</button>
          <button className="btn-primary" onClick={submit} disabled={!name.trim() || !path.trim() || busy}>
            添加
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div>
          <label className="mb-1 block text-sm text-fg-muted">名称</label>
          <input className="input w-full" placeholder="例如 Trae / Kiro / Continue" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div>
          <label className="mb-1 block text-sm text-fg-muted">Skills 目录绝对路径</label>
          <input className="input w-full font-mono text-xs" placeholder="例如 C:\Users\you\AppData\Roaming\Trae\skills" value={path} onChange={(e) => setPath(e.target.value)} />
          <p className="mt-1 text-xs text-fg-muted">
            启用的 skill 会作为 symlink / junction 放进这个目录。具体编辑器读不读得到，取决于它本身。
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
    file_path: "",
    enabled: true,
    created_at: now,
    updated_at: now,
  };
}

import { marked } from "marked";

function SkillFormModal({ initial, onClose, onSaved }: {
  initial: Skill;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [content, setContent] = useState(initial.content);
  const [tab, setTab] = useState<"edit" | "preview">("edit");

  const submit = async () => {
    // 从 frontmatter 提取 name + description
    const m = content.match(/^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/);
    let front: Record<string, unknown> = {};
    let body = content;
    if (m) {
      body = m[2];
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
            __html: marked.parse(content.replace(/^---[\s\S]*?---\n/, "")) as string,
          }}
        />
      )}
      <div className="mt-2 text-xs text-fg-muted">{t("skills.frontmatter_hint")}</div>
    </Modal>
  );
}
