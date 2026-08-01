import { useMemo, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Pencil, Eye, EyeOff, FolderOpen, Search, Upload, Download, FileText, RefreshCw, Check } from "lucide-react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { Section, EmptyHint } from "@/components/Section";
import { Modal } from "@/components/Modal";
import { ConfirmModal } from "@/components/ConfirmModal";
import { toast } from "sonner";
import {
  listMemories, upsertMemory, deleteMemory, toggleMemory,
  syncMemoryToTarget, pullMemoryFromTarget, openMemoryTargetDir,
} from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import type { Memory } from "@/lib/ipc";

function blankMemory(name = ""): Memory {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    name,
    description: "",
    target_path: "",
    enabled: false,
    builtin: false,
    content: `# ${name || "memory"}\n\n在此编写该工具的全局记忆与规则。\n`,
    created_at: now,
    updated_at: now,
  };
}

export function Memories() {
  const { t } = useTranslation();
  const [memories, setMemories] = useState<Memory[]>([]);
  const [editing, setEditing] = useState<Memory | null>(null);
  const [creating, setCreating] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<Memory | null>(null);
  const [busyIds, setBusyIds] = useState<Record<string, "toggle" | "delete" | "sync" | "pull">>({});
  const [search, setSearch] = useState("");
  const [searchInput, setSearchInput] = useState("");

  const markBusy = (id: string, kind: "toggle" | "delete" | "sync" | "pull") => setBusyIds((m) => ({ ...m, [id]: kind }));
  const clearBusy = (id: string) => setBusyIds((m) => { const { [id]: _, ...rest } = m; return rest; });

  const load = async () => {
    try {
      setMemories(await listMemories());
    } catch (e) {
      toast.error(t("memories.load_failed", { msg: describeError(e) }) as string);
    }
  };
  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- 只在 mount 时加载,t 已通过 i18n 缓存稳定
  }, []);

  const visible = useMemo(() => {
    if (!search.trim()) return memories;
    const q = search.toLowerCase();
    return memories.filter(
      (m) =>
        m.name.toLowerCase().includes(q) ||
        (m.description ?? "").toLowerCase().includes(q) ||
        m.target_path.toLowerCase().includes(q),
    );
  }, [memories, search]);

  const handleToggle = async (m: Memory) => {
    if (busyIds[m.id]) return;
    markBusy(m.id, "toggle");
    try {
      await toggleMemory(m.id, !m.enabled);
      await load();
      toast.success(t(m.enabled ? "memories.disabled" : "memories.enabled", { name: m.name }) as string);
    } catch (e) {
      toast.error(t("memories.toggle_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(m.id);
    }
  };

  const handleSync = async (m: Memory) => {
    if (busyIds[m.id]) return;
    markBusy(m.id, "sync");
    try {
      await syncMemoryToTarget(m.id);
      await load();
      toast.success(t("memories.synced", { name: m.name }) as string);
    } catch (e) {
      toast.error(t("memories.sync_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(m.id);
    }
  };

  const handlePull = async (m: Memory) => {
    if (busyIds[m.id]) return;
    markBusy(m.id, "pull");
    try {
      const updated = await pullMemoryFromTarget(m.id);
      setMemories((prev) => prev.map((x) => (x.id === updated.id ? updated : x)));
      toast.success(t("memories.pulled", { name: m.name }) as string);
    } catch (e) {
      toast.error(t("memories.pull_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(m.id);
    }
  };

  const handleDelete = async (m: Memory) => {
    if (busyIds[m.id]) return;
    markBusy(m.id, "delete");
    try {
      await deleteMemory(m.id);
      await load();
      toast.success(t("memories.deleted", { name: m.name }) as string);
    } catch (e) {
      toast.error(t("memories.delete_failed", { msg: describeError(e) }) as string);
    } finally {
      clearBusy(m.id);
    }
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("memories.title")}
        action={
          <div className="flex gap-2">
            <div className="flex max-w-[220px]">
              <input
                type="search"
                className="input flex-1 rounded-r-none"
                placeholder={t("memories.search_placeholder") as string}
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") setSearch(searchInput); }}
                aria-label={t("memories.search_label") as string}
              />
              <button className="btn-primary rounded-l-none" onClick={() => setSearch(searchInput)} title={t("common.search")}>
                <Search size={14} />
              </button>
            </div>
            <button className="btn-primary" onClick={() => setCreating(true)}><Plus className="icon-sm"/>{t("common.add")}</button>
          </div>
        }
      >
        <div className="mb-2 text-xs text-fg-muted">{t("memories.desc")}</div>
        {memories.length === 0 ? (
          <EmptyHint>{t("memories.empty")}</EmptyHint>
        ) : visible.length === 0 ? (
          <div className="rounded-md border border-border bg-bg-subtle p-6 text-center text-sm text-fg-muted">
            <Search className="icon-lg mx-auto mb-2 opacity-50" />
            {t("memories.no_match", { q: search })}
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {visible.map((m) => (
              <MemoryCard
                key={m.id}
                memory={m}
                busy={busyIds[m.id]}
                onToggle={() => handleToggle(m)}
                onEdit={() => setEditing(m)}
                onAskDelete={() => setConfirmDelete(m)}
                onSync={() => handleSync(m)}
                onPull={() => handlePull(m)}
              />
            ))}
          </div>
        )}
      </Section>

      {(editing || creating) && (
        <MemoryFormModal
          creating={creating}
          initial={editing ?? blankMemory()}
          onClose={() => { setEditing(null); setCreating(false); }}
          onSaved={async () => { setEditing(null); setCreating(false); await load(); }}
        />
      )}

      <ConfirmModal
        open={!!confirmDelete}
        title={t("common.confirm")}
        confirmLabel={t("common.delete")}
        busy={!!confirmDelete && busyIds[confirmDelete.id] === "delete"}
        onClose={() => setConfirmDelete(null)}
        onConfirm={async () => {
          if (!confirmDelete) return;
          const target = confirmDelete;
          setConfirmDelete(null);
          await handleDelete(target);
        }}
      >
        <p className="text-sm">{t("memories.delete_confirm")}</p>
        {confirmDelete && (
          <p className="mt-2 break-all font-mono text-xs text-fg-muted">{confirmDelete.name}</p>
        )}
      </ConfirmModal>
    </div>
  );
}

function MemoryCard({ memory: m, busy, onToggle, onEdit, onAskDelete, onSync, onPull }: {
  memory: Memory;
  busy?: string;
  onToggle: () => void;
  onEdit: () => void;
  onAskDelete: () => void;
  onSync: () => void;
  onPull: () => void;
}) {
  const { t } = useTranslation();
  const drift = m.enabled && m.target_exists && !m.in_sync;
  const status = !m.enabled
    ? { label: t("memories.st_disabled"), cls: "text-fg-muted", dot: "bg-fg-muted" }
    : !m.target_exists
      ? { label: t("memories.st_no_target"), cls: "text-warn", dot: "bg-warn" }
      : m.in_sync
        ? { label: t("memories.st_in_sync"), cls: "text-ok", dot: "bg-ok" }
        : { label: t("memories.st_drift"), cls: "text-warn", dot: "bg-warn" };

  return (
    <div className="card group">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="truncate font-semibold">{m.name}</span>
            {m.builtin && <span className="badge-mute text-3xs">{t("memories.builtin_badge")}</span>}
          </div>
          {m.description && <div className="line-clamp-1 text-xs text-fg-muted">{m.description}</div>}
        </div>
        <span className={`inline-flex shrink-0 items-center gap-1 text-xs ${status.cls}`}>
          <span className={`inline-block h-1.5 w-1.5 rounded-full ${status.dot}`} />
          {status.label}
        </span>
      </div>

      <div className="mt-2 flex items-center gap-1.5 rounded-md bg-bg-subtle px-2 py-1.5">
        <FileText className="icon-xs shrink-0 text-fg-muted" />
        <code className="truncate font-mono text-3xs text-fg-muted">{m.target_path}</code>
      </div>

      {drift && (
        <div className="mt-2 flex items-center gap-1 rounded-md bg-warn/10 px-2 py-1.5 text-2xs text-warn">
          <RefreshCw className="icon-xs" />
          <span className="min-w-0 truncate">{t("memories.drift_hint")}</span>
          <span className="ml-auto flex shrink-0 gap-1">
            <button
              className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-fg hover:bg-overlay-soft disabled:opacity-50"
              onClick={onPull}
              disabled={!!busy}
              title={t("memories.pull_title") as string}
            >
              {busy === "pull" ? <Spinner cls="text-fg" /> : <Download className="icon-xs" />}
              {t("memories.pull_btn")}
            </button>
            <button
              className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-fg hover:bg-overlay-soft disabled:opacity-50"
              onClick={onSync}
              disabled={!!busy}
              title={t("memories.sync_title") as string}
            >
              {busy === "sync" ? <Spinner cls="text-fg" /> : <Upload className="icon-xs" />}
              {t("memories.sync_btn")}
            </button>
          </span>
        </div>
      )}

      <div className="mt-3 flex items-center justify-between">
        <div className="text-2xs text-fg-muted">{new Date(m.updated_at).toLocaleString()}</div>
        <div className="flex gap-1">
          <button className="btn-ghost p-1.5" onClick={() => { void openMemoryTargetDir(m.id).catch((e) => toast.error(describeError(e))); }} title={t("memories.open_dir_title")} aria-label={t("memories.open_dir_title") as string}>
            <FolderOpen className="icon-sm text-fg-muted" />
          </button>
          <button className="btn-ghost p-1.5 text-ok disabled:opacity-50 disabled:cursor-not-allowed" onClick={onToggle} disabled={!!busy} title={m.enabled ? t("common.disable") : t("common.enable")} aria-label={t("memories.card_toggle") as string}>
            {busy === "toggle" ? <Spinner cls="text-ok" /> : m.enabled ? <EyeOff className="icon-sm"/> : <Eye className="icon-sm"/>}
          </button>
          <button className="btn-ghost p-1.5 text-accent hover:opacity-80 disabled:opacity-50 disabled:cursor-not-allowed" onClick={onEdit} disabled={!!busy} title={t("common.edit")} aria-label={t("memories.card_edit") as string}>
            <Pencil className="icon-sm"/>
          </button>
          <button className="btn-ghost p-1.5 text-err hover:text-err/80 disabled:opacity-50 disabled:cursor-not-allowed" onClick={onAskDelete} disabled={!!busy} title={t("common.delete")} aria-label={t("memories.card_delete") as string}>
            {busy === "delete" ? <Spinner cls="text-err" /> : <Trash2 className="icon-sm"/>}
          </button>
        </div>
      </div>
    </div>
  );
}

function Spinner({ cls }: { cls: string }) {
  return <span className={`inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-t-transparent ${cls}`} />;
}

function MemoryFormModal({ creating, initial, onClose, onSaved }: {
  creating: boolean;
  initial: Memory;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial.name);
  const [description, setDescription] = useState(initial.description ?? "");
  const [targetPath, setTargetPath] = useState(initial.target_path);
  const [content, setContent] = useState(initial.content);
  const [tab, setTab] = useState<"edit" | "preview">("edit");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!name.trim()) { toast.error(t("memories.name_required") as string); return; }
    if (!targetPath.trim()) { toast.error(t("memories.path_required") as string); return; }
    setBusy(true);
    try {
      await upsertMemory({
        ...initial,
        name: name.trim(),
        description: description.trim() || undefined,
        target_path: targetPath.trim(),
        content,
        updated_at: new Date().toISOString(),
      });
      toast.success(t("memories.saved") as string);
      await onSaved();
    } catch (e) {
      toast.error(t("memories.save_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusy(false);
    }
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); void submit(); }
  };

  return (
    <Modal
      open
      size="xl"
      title={creating ? t("memories.add_title") : initial.builtin ? t("memories.edit_preset_title") : t("memories.edit_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>{t("common.cancel")}</button>
          <button className="btn-primary" onClick={submit} disabled={busy}>
            {busy ? <span className="inline-flex items-center gap-1"><Spinner cls="border-onAccent" />{t("common.saving")}</span> : t("common.save")}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="mb-1 block text-sm text-fg-muted">{t("common.name")}</label>
            <input className="input w-full" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={onKeyDown} placeholder={t("memories.name_placeholder") as string} />
          </div>
          <div>
            <label className="mb-1 block text-sm text-fg-muted">{t("common.description")}</label>
            <input className="input w-full" value={description} onChange={(e) => setDescription(e.target.value)} onKeyDown={onKeyDown} placeholder={t("memories.desc_placeholder") as string} />
          </div>
        </div>
        <div>
          <label className="mb-1 block text-sm text-fg-muted">{t("memories.target_path_label")}</label>
          <input className="input w-full font-mono text-xs" value={targetPath} onChange={(e) => setTargetPath(e.target.value)} onKeyDown={onKeyDown} spellCheck={false} placeholder={t("memories.target_path_placeholder") as string} />
          <p className="mt-1 text-xs text-fg-muted">{t("memories.target_path_hint")}</p>
        </div>
        <div>
          <div className="mb-1 flex items-center justify-between">
            <span className="text-sm text-fg-muted">{t("memories.content_label")}</span>
            <div className="flex gap-1">
              <button className={tab === "edit" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("edit")}>{t("skills.raw")}</button>
              <button className={tab === "preview" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("preview")}>{t("skills.preview")}</button>
            </div>
          </div>
          {tab === "edit" ? (
            <textarea
              className="input min-h-[40vh] font-mono text-sm"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder={t("memories.content_placeholder") as string}
            />
          ) : (
            <div
              className="prose prose-invert max-h-[40vh] max-w-none overflow-auto rounded-md border border-border bg-bg-subtle p-4 [&_*]:text-fg [&_code]:text-accent [&_a]:text-accent"
              dangerouslySetInnerHTML={{
                __html: DOMPurify.sanitize(marked.parse(content) as string),
              }}
            />
          )}
        </div>
        <div className="flex items-center gap-2 text-xs text-fg-muted">
          <Check className="icon-xs text-ok" />
          {t("memories.enabled_hint")}
        </div>
      </div>
    </Modal>
  );
}
