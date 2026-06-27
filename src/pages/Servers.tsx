import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, Pencil, Play, Square, RefreshCw, TestTube2, FolderOpen, Loader2, FileInput, FileDown, X } from "lucide-react";
import { toast } from "sonner";
import { Section, EmptyHint } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Modal } from "@/components/Modal";
import {
  listServers, listServerStates, upsertServer, deleteServer,
  startServer, stopServer, testServer,
  fetchServerTools, setToolEnabled,
  listProjects, upsertProject, deleteProject,
} from "@/lib/ipc";
import type { McpServer, ServerState, Transport, Project, McpTool } from "@/lib/ipc";

export function Servers() {
  const { t } = useTranslation();
  const [servers, setServers] = useState<McpServer[]>([]);
  const [states, setStates] = useState<Record<string, ServerState>>({});
  const [projects, setProjects] = useState<Project[]>([]);
  const [editing, setEditing] = useState<McpServer | null>(null);
  const [creating, setCreating] = useState(false);
  const [managingProjects, setManagingProjects] = useState(false);
  const [viewing, setViewing] = useState<McpServer | null>(null);
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [filterProject, setFilterProject] = useState<string>("");

  const load = async () => {
    const [ss, sts, ps] = await Promise.all([listServers(), listServerStates(), listProjects()]);
    setServers(ss);
    setStates(Object.fromEntries(sts.map((s) => [s.server.id, s])));
    setProjects(ps);
  };

  useEffect(() => { void load(); }, []);

  const filtered = servers.filter((s) =>
    (!search || s.name.toLowerCase().includes(search.toLowerCase()) || (s.description ?? "").toLowerCase().includes(search.toLowerCase())) &&
    (!filterProject ||
      (filterProject === "__unassigned__" && !s.project_id) ||
      s.project_id === filterProject)
  );

  const [startingIds, setStartingIds] = useState<Set<string>>(new Set());
  const [stoppingIds, setStoppingIds] = useState<Set<string>>(new Set());

  const handleDelete = async (s: McpServer) => {
    try {
      await deleteServer(s.id);
      await load();
      toast.success(`已删除 "${s.name}"`, { duration: 2000 });
    } catch (e) {
      toast.error(`删除失败: ${e}`);
    }
  };

  const handleTest = async (s: McpServer) => {
    const tid = `test-${s.id}`;
    toast.loading(`正在测速 ${s.name}...`, { id: tid });
    try {
      const r = await testServer(s.id);
      if (r.ok) {
        toast.success(`${s.name} 测速成功 (${r.latency_ms ?? "?"}ms, ${(r.tools ?? []).length} 工具)`, { id: tid });
      } else {
        toast.error(`${s.name} 测速失败: ${r.error}`, { id: tid });
      }
    } catch (e) {
      toast.error(`测速出错: ${e}`, { id: tid });
    }
  };

  const handleStart = async (s: McpServer) => {
    setStartingIds((prev) => new Set(prev).add(s.id));
    toast.loading(`正在启动 ${s.name}...`, { id: `start-${s.id}` });
    try {
      // 顺手把 auto_start 勾上，下次开 GUI 自动起（跟 mcp-router 一致）
      if (!s.auto_start) {
        await upsertServer({ ...s, auto_start: true });
      }
      await startServer(s.id);
      toast.success(`${s.name} 已启动`, { id: `start-${s.id}` });
      await load();
    } catch (e) {
      toast.error(`启动 ${s.name} 失败: ${e}`, { id: `start-${s.id}` });
    } finally {
      setStartingIds((prev) => { const n = new Set(prev); n.delete(s.id); return n; });
    }
  };

  const handleStop = async (s: McpServer) => {
    setStoppingIds((prev) => new Set(prev).add(s.id));
    toast.loading(`正在停止 ${s.name}...`, { id: `stop-${s.id}` });
    try {
      await stopServer(s.id);
      // 停止 = 用户明确不想要它常驻，把 auto_start 也关掉
      if (s.auto_start) {
        await upsertServer({ ...s, auto_start: false });
      }
      toast.success(`${s.name} 已停止`, { id: `stop-${s.id}` });
      await load();
    } catch (e) {
      toast.error(`停止 ${s.name} 失败: ${e}`, { id: `stop-${s.id}` });
    } finally {
      setStoppingIds((prev) => { const n = new Set(prev); n.delete(s.id); return n; });
    }
  };

  // 导入 JSON：用户粘贴 mcpServers 配置，解析后批量入库
  const importFromJson = () => setImportModalOpen(true);

  // 导出 JSON：把当前所有 enabled + stdio 的 server 打成 mcpServers 配置
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
    try { await navigator.clipboard.writeText(json); } catch {}
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `mcp_servers_${new Date().toISOString().slice(0,10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success(`已导出 ${enabled.length} 个 server（已复制到剪贴板，已触发下载）`);
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("servers.title")}
        action={
          <div className="flex gap-2">
            <button className="btn-ghost" onClick={() => setManagingProjects(true)}>
              <FolderOpen size={14}/>{t("projects.title")}
            </button>
            <button className="btn-ghost" onClick={importFromJson} title="粘贴 mcpServers JSON 批量导入">
              <FileInput size={14}/>导入 JSON
            </button>
            <button className="btn-ghost" onClick={exportToJson} title="把当前所有 enabled server 导出为 mcpServers JSON">
              <FileDown size={14}/>导出 JSON
            </button>
            <button className="btn-primary" onClick={() => setCreating(true)}>
              <Plus size={14}/>{t("common.add")}
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
          <select className="input max-w-[180px]" value={filterProject} onChange={(e) => setFilterProject(e.target.value)}>
            <option value="">所有项目</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
          <span className="text-xs text-fg-muted">{filtered.length} / {servers.length}</span>
        </div>

        {filtered.length === 0 ? (
          <EmptyHint>{t("common.empty")}</EmptyHint>
        ) : (
          <div className="overflow-hidden rounded-lg border border-white/5">
            <table className="w-full text-sm">
              <thead className="bg-bg-subtle text-left text-xs uppercase text-fg-muted">
                <tr>
                  <th className="px-3 py-2 font-medium">{t("common.name")}</th>
                  <th className="px-3 py-2 font-medium">{t("common.transport")}</th>
                  <th className="px-3 py-2 font-medium">{t("projects.title")}</th>
                  <th className="px-3 py-2 font-medium">{t("common.status")}</th>
                  <th className="px-3 py-2 font-medium">{t("common.last_error")}</th>
                  <th className="px-3 py-2 font-medium text-right">{t("common.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((s) => {
                  const st = states[s.id];
                  const project = projects.find((p) => p.id === s.project_id);
                  return (
                    <tr key={s.id} className="border-t border-white/5 hover:bg-white/[0.02]">
                      <td
                        className="px-3 py-2 cursor-pointer"
                        onClick={() => setViewing(s)}
                        title="点击查看详情"
                      >
                        <div className="font-medium">{s.name}</div>
                        {s.description && <div className="text-xs text-fg-muted">{s.description}</div>}
                      </td>
                      <td className="px-3 py-2"><span className="badge-mute">{s.transport}</span></td>
                      <td className="px-3 py-2 text-xs text-fg-muted">{project?.name ?? "—"}</td>
                      <td className="px-3 py-2">{st ? <StatusBadge status={st.status} /> : <span className="text-fg-muted">—</span>}</td>
                      <td className="px-3 py-2 text-xs text-err/80 max-w-xs truncate">{st?.last_error ?? "—"}</td>
                      <td className="px-3 py-2">
                        <div className="flex justify-end gap-1">
                          {st?.status === "running" ? (
                            <button className="btn-ghost p-1.5 text-ok" onClick={() => handleStop(s)} disabled={stoppingIds.has(s.id)} title={t("common.stop")}>
                              {stoppingIds.has(s.id) ? <Loader2 size={14} className="animate-spin" /> : <Square size={14}/>}
                            </button>
                          ) : (
                            <button className="btn-ghost p-1.5 text-ok" onClick={() => handleStart(s)} disabled={startingIds.has(s.id)} title={t("common.start")}>
                              {startingIds.has(s.id) ? <Loader2 size={14} className="animate-spin" /> : <Play size={14}/>}
                            </button>
                          )}
                          {/* 测速：橙色 —— "我要试一下" */}
                          <button className="btn-ghost p-1.5 text-amber-400 hover:text-amber-300" onClick={() => handleTest(s)} title={t("common.test")}>
                            <TestTube2 size={14}/>
                          </button>
                          {/* 编辑：蓝色 —— "我要改配置" */}
                          <button className="btn-ghost p-1.5 text-accent hover:opacity-80" onClick={() => setEditing(s)} title={t("common.edit")}>
                            <Pencil size={14}/>
                          </button>
                          {/* 删除：红色 —— "危险" */}
                          <button className="btn-ghost p-1.5 text-err hover:text-err/80" onClick={() => handleDelete(s)} title={t("common.delete")}>
                            <Trash2 size={14}/>
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Section>

      {(editing || creating) && (
        <ServerFormModal
          initial={editing ?? blankServer()}
          projects={projects.filter((p) => p.id !== "__unassigned__")}
          onClose={() => { setEditing(null); setCreating(false); }}
          onSaved={async () => { setEditing(null); setCreating(false); await load(); }}
        />
      )}

      {managingProjects && (
        <ProjectsModal
          onClose={() => setManagingProjects(false)}
          onChanged={async () => { await load(); }}
        />
      )}

      {importModalOpen && (
        <ImportJsonModal
          onClose={() => setImportModalOpen(false)}
          onImported={load}
        />
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

function blankServer(): McpServer {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    name: "",
    transport: "stdio",
    command: "",
    args: [],
    env: {},
    enabled: true,
    created_at: now,
    updated_at: now,
  };
}

function ProjectsModal({ onClose, onChanged }: { onClose: () => void; onChanged: () => Promise<void> }) {
  const { t } = useTranslation();
  const [list, setList] = useState<Project[]>([]);
  const [editing, setEditing] = useState<Project | null>(null);
  const [creating, setCreating] = useState(false);

  const load = async () => setList(await listProjects());
  useEffect(() => { void load(); }, []);

  const handleSave = async (p: Project) => {
    try {
      if (p.id && p.id !== "__unassigned__") {
        await upsertProject(p);
      } else {
        await upsertProject({ ...p, id: "" });
      }
      setEditing(null); setCreating(false); await load(); await onChanged();
      toast.success(`项目已保存`, { duration: 1500 });
    } catch (e) { toast.error(`保存失败: ${e}`); }
  };

  const handleDelete = async (p: Project) => {
    try {
      await deleteProject(p.id);
      await load();
      await onChanged();
      toast.success(`已删除项目 "${p.name}"`, { duration: 2000 });
    } catch (e) {
      toast.error(`删除项目失败: ${e}`);
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
      footer={<button className="btn-primary" onClick={() => setCreating(true)}><Plus size={14}/>{t("common.add")}</button>}
    >
      <div className="space-y-2">
        {list.length === 0 ? (
          <EmptyHint>{t("common.empty")}</EmptyHint>
        ) : (
          list.map((p) => (
            <div key={p.id} className="flex items-center gap-2 rounded-md border border-white/5 bg-bg-subtle px-3 py-2">
              <div className="flex-1">
                <div className="font-medium">{p.name}</div>
                {p.description && <div className="text-xs text-fg-muted">{p.description}</div>}
                {p.id === "__unassigned__" && <div className="text-xs text-fg-muted">虚拟分组 · 未指派 server 归这里</div>}
              </div>
              {p.id !== "__unassigned__" && (
                <>
                  <button className="btn-ghost p-1.5" onClick={() => setEditing(p)}><Pencil size={14}/></button>
                  <button className="btn-ghost p-1.5 text-err" onClick={() => handleDelete(p)}><Trash2 size={14}/></button>
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
          onClose={() => { setEditing(null); setCreating(false); }}
          footer={
            <>
              <button className="btn-ghost" onClick={() => { setEditing(null); setCreating(false); }}>{t("common.cancel")}</button>
              <button className="btn-primary" onClick={() => handleSave(editing ?? blank())}>{t("common.save")}</button>
            </>
          }
        >
          <ProjectForm initial={editing ?? blank()} onChange={setEditing as (p: Project) => void} />
        </Modal>
      )}
    </Modal>
  );
}

function ProjectForm({ initial, onChange }: { initial: Project; onChange: (p: Project) => void }) {
  return (
    <div className="space-y-3">
      <Field label="名称">
        <input className="input" value={initial.name} onChange={(e) => onChange({ ...initial, name: e.target.value })} />
      </Field>
      <Field label="描述">
        <input className="input" value={initial.description ?? ""} onChange={(e) => onChange({ ...initial, description: e.target.value })} />
      </Field>
    </div>
  );
}

function ServerFormModal({ initial, projects, onClose, onSaved }: {
  initial: McpServer;
  projects: Project[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [s, setS] = useState<McpServer>(initial);
  const [argsText, setArgsText] = useState((initial.args ?? []).join(" "));
  const [envText, setEnvText] = useState(
    Object.entries(initial.env ?? {}).map(([k, v]) => `${k}=${v}`).join("\n"),
  );

  const submit = async () => {
    const args = argsText.trim() ? argsText.split(/\s+/) : [];
    const env: Record<string, string> = {};
    envText.split(/\n/).forEach((line) => {
      const m = line.match(/^\s*([A-Z_][A-Z0-9_]*)\s*=\s*(.*)$/i);
      if (m) env[m[1]] = m[2];
    });
    try {
      await upsertServer({ ...s, args, env, updated_at: new Date().toISOString() });
      await onSaved();
      toast.success(`已保存 "${s.name}"`, { duration: 1500 });
    } catch (e) { toast.error(`保存失败: ${e}`); }
  };

  const applyPreset = (preset: "filesystem" | "fetch" | "github" | "memory") => {
    const presets: Record<string, Partial<McpServer>> = {
      filesystem: { name: "filesystem", description: "本地文件系统读写", transport: "stdio", command: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem", "."] },
      fetch:      { name: "fetch", description: "网页抓取", transport: "stdio", command: "uvx", args: ["mcp-server-fetch"] },
      github:     { name: "github", description: "GitHub API", transport: "stdio", command: "npx", args: ["-y", "@modelcontextprotocol/server-github"], env: { GITHUB_PERSONAL_ACCESS_TOKEN: "" } },
      memory:     { name: "memory", description: "基于知识图谱的长期记忆", transport: "stdio", command: "npx", args: ["-y", "@modelcontextprotocol/server-memory"] },
    };
    setS({ ...s, ...presets[preset] });
    setArgsText((presets[preset].args ?? []).join(" "));
    setEnvText(Object.entries(presets[preset].env ?? {}).map(([k, v]) => `${k}=${v}`).join("\n"));
  };

  return (
    <Modal
      open
      size="lg"
      title={initial.name ? t("servers.edit_title") : t("servers.add_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose}>{t("common.cancel")}</button>
          <button className="btn-primary" onClick={submit}>{t("common.save")}</button>
        </>
      }
    >
      <div className="space-y-3">
        {!initial.name && (
          <div>
            <div className="mb-1 text-xs text-fg-muted">{t("servers.presets_hint")}</div>
            <div className="flex flex-wrap gap-1">
              {(["filesystem","fetch","github","memory"] as const).map((p) => (
                <button key={p} className="btn-ghost text-xs" onClick={() => applyPreset(p)}>
                  {t(`servers.preset_${p}`)}
                </button>
              ))}
            </div>
          </div>
        )}
        <Field label={t("common.name")}>
          <input className="input" value={s.name} onChange={(e) => setS({ ...s, name: e.target.value })} />
        </Field>
        <Field label={t("common.description")}>
          <input className="input" value={s.description ?? ""} onChange={(e) => setS({ ...s, description: e.target.value })} />
        </Field>
        <Field label={t("projects.title")}>
          <select className="input max-w-md" value={s.project_id ?? ""} onChange={(e) => setS({ ...s, project_id: e.target.value || null })}>
            <option value="">未分组</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
        </Field>
        <Field label={t("common.transport")}>
          <div className="flex gap-2">
            {(["stdio","sse","http"] as Transport[]).map((tr) => (
              <button key={tr} className={s.transport === tr ? "btn-primary" : "btn-ghost"} onClick={() => setS({ ...s, transport: tr })}>{tr}</button>
            ))}
          </div>
        </Field>

        {s.transport === "stdio" && (
          <>
            <Field label={t("common.command")}>
              <input className="input font-mono" placeholder="npx / uvx / node / python …"
                value={s.command ?? ""} onChange={(e) => setS({ ...s, command: e.target.value })} />
            </Field>
            <Field label={t("common.args")}>
              <input className="input font-mono" placeholder="-y @org/server arg1 arg2"
                value={argsText} onChange={(e) => setArgsText(e.target.value)} />
            </Field>
            <Field label={t("common.env")}>
              <textarea className="input min-h-[80px] font-mono" placeholder={"KEY=value\nANOTHER=foo"}
                value={envText} onChange={(e) => setEnvText(e.target.value)} />
            </Field>
            <Field label={t("common.cwd")}>
              <input className="input font-mono" placeholder="(留空使用默认)"
                value={s.cwd ?? ""} onChange={(e) => setS({ ...s, cwd: e.target.value || undefined })} />
            </Field>
          </>
        )}

        {(s.transport === "sse" || s.transport === "http") && (
          <>
            <Field label={t("common.url")}>
              <input className="input font-mono" placeholder="https://example.com/mcp"
                value={s.url ?? ""} onChange={(e) => setS({ ...s, url: e.target.value })} />
            </Field>
            <Field label="Bearer Token (可选)">
              <input className="input font-mono" placeholder="ghp_xxx..."
                value={s.bearer_token ?? ""} onChange={(e) => setS({ ...s, bearer_token: e.target.value || undefined })} />
            </Field>
            <Field label={t("common.headers")}>
              <textarea className="input min-h-[60px] font-mono" placeholder={"Authorization=Bearer xxx"}
                value={Object.entries(s.headers ?? {}).map(([k,v]) => `${k}=${v}`).join("\n")}
                onChange={(e) => {
                  const h: Record<string,string> = {};
                  e.target.value.split(/\n/).forEach((line) => {
                    const m = line.match(/^\s*([\w-]+)\s*[:=]\s*(.*)$/);
                    if (m) h[m[1]] = m[2];
                  });
                  setS({ ...s, headers: h });
                }} />
            </Field>
          </>
        )}

        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={s.enabled} onChange={(e) => setS({ ...s, enabled: e.target.checked })} />
          {t("common.enabled_col")}
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={s.auto_start ?? false} onChange={(e) => setS({ ...s, auto_start: e.target.checked })} />
          应用启动时自动启动
        </label>
      </div>
    </Modal>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1 text-xs text-fg-muted">{label}</div>
      {children}
    </div>
  );
}

function ImportJsonModal({ onClose, onImported }: { onClose: () => void; onImported: () => Promise<void> }) {
  const [raw, setRaw] = useState("");
  const [parsed, setParsed] = useState<{ name: string; conf: any }[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  const tryParse = (text: string) => {
    setRaw(text);
    setError(null);
    if (!text.trim()) { setParsed(null); return; }
    try {
      const obj = JSON.parse(text);
      const servers = obj.mcpServers ?? obj;
      const entries = Object.entries(servers).map(([name, conf]) => ({ name, conf }));
      // 过滤掉没法入库的条目（不是 stdio / http / sse）
      const valid = entries.filter(({ conf }: any) => conf.command || conf.url || conf.type);
      const skipped = entries.length - valid.length;
      setParsed(valid);
      if (skipped > 0) {
        setError(`已跳过 ${skipped} 个无法识别的条目（缺少 command / url / type）`);
      }
    } catch (e: any) {
      setError(`JSON 解析失败: ${e.message}`);
      setParsed(null);
    }
  };

  const doImport = async () => {
    if (!parsed || parsed.length === 0) return;
    setImporting(true);
    let count = 0;
    try {
      for (const { name, conf } of parsed) {
        const c = conf as any;
        await upsertServer({
          id: crypto.randomUUID(),
          name,
          description: c.description,
          transport: c.type === "http" || c.type === "sse" ? c.type : "stdio",
          command: c.command ?? "",
          args: c.args ?? [],
          env: c.env ?? {},
          cwd: c.cwd,
          url: c.url,
          headers: c.headers,
          enabled: true,
          auto_start: false,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        } as McpServer);
        count++;
      }
      toast.success(`已导入 ${count} 个 server`);
      await onImported();
      onClose();
    } catch (e) {
      toast.error(`导入失败: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  // 一键填个示例
  const fillSample = () => tryParse(JSON.stringify({
    mcpServers: {
      filesystem: { command: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem", "C:\\Users\\admin\\Documents"] },
      memory:     { command: "uvx", args: ["mcp-server-memory"] },
      puppeteer:  { command: "npx", args: ["-y", "@modelcontextprotocol/server-puppeteer"], env: { ALLOW_DANGEROUS: "true" } },
    }
  }, null, 2));

  return (
    <Modal
      open
      title="从 JSON 导入 mcpServers"
      onClose={onClose}
      size="lg"
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={importing}>取消</button>
          <button
            className="btn-primary"
            onClick={doImport}
            disabled={!parsed || parsed.length === 0 || importing}
          >
            {importing ? "导入中..." : `导入 ${parsed?.length ?? 0} 个`}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div>
          <div className="mb-1 flex items-center justify-between">
            <label className="text-sm text-fg-muted">粘贴 mcpServers JSON</label>
            <button className="btn-ghost text-xs" onClick={fillSample}>填入示例</button>
          </div>
          <textarea
            className="input h-40 w-full font-mono text-xs"
            placeholder={`{\n  "mcpServers": {\n    "name": { "command": "...", "args": [...] }\n  }\n}`}
            value={raw}
            onChange={(e) => tryParse(e.target.value)}
            spellCheck={false}
          />
          <p className="mt-1 text-xs text-fg-muted">
            支持标准 <code className="text-fg">{"{ \"mcpServers\": {...} }"}</code> 格式，也兼容裸
            <code className="text-fg ml-1">{"{ \"name\": {...} }"}</code>。
            transport 自动识别：<code className="text-fg">stdio</code> / <code className="text-fg">http</code> / <code className="text-fg">sse</code>。
          </p>
        </div>

        {error && (
          <div className="rounded border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">
            {error}
          </div>
        )}

        {parsed && parsed.length > 0 && (
          <div className="space-y-2">
            <div className="text-sm text-fg-muted">预览（{parsed.length} 个 server）：</div>
            <div className="max-h-48 space-y-1 overflow-auto rounded border border-white/5 bg-bg-subtle p-2">
              {parsed.map(({ name, conf }) => {
                const c = conf as any;
                const t = c.type === "http" ? "http" : c.type === "sse" ? "sse" : "stdio";
                const summary = t === "stdio"
                  ? `${c.command ?? ""} ${(c.args ?? []).join(" ")}`.trim()
                  : `${c.url ?? ""}`;
                return (
                  <div key={name} className="flex items-center gap-2 rounded px-2 py-1 hover:bg-white/5">
                    <span className="badge-mute">{t}</span>
                    <span className="font-medium">{name}</span>
                    <code className="flex-1 truncate text-xs text-fg-muted">{summary}</code>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}
// ==================== Server 详情侧边栏 ====================

function ServerDetailDrawer({ server, onClose, onChanged }: {
  server: McpServer;
  onClose: () => void;
  onChanged: () => Promise<void>;
}) {
  const [tab, setTab] = useState<"general" | "tools">("general");
  const [tools, setTools] = useState<McpTool[] | null>(null);
  const [loadingTools, setLoadingTools] = useState(false);
  const [toolsErr, setToolsErr] = useState<string | null>(null);
  // 本地 tool 启用态覆盖：点 toggle 立刻翻，后台再保存。
  // 这样用户感觉是即时的；后端保存失败时回滚 + 弹错误 toast。
  const [localDisabled, setLocalDisabled] = useState<Set<string>>(
    () => new Set(server.disabled_tools ?? []),
  );
  // 当父组件更新 server（外部改了 disabled_tools）时同步本地态
  useEffect(() => {
    setLocalDisabled(new Set(server.disabled_tools ?? []));
  }, [server.disabled_tools]);

  const toggleTool = async (toolName: string, enabled: boolean) => {
    const prev = new Set(localDisabled);
    const next = new Set(localDisabled);
    if (enabled) next.delete(toolName); else next.add(toolName);
    setLocalDisabled(next);  // 立刻翻 —— 用户感觉是即时的
    try {
      await setToolEnabled(server.id, toolName, enabled);
      await onChanged();
      toast.success(enabled ? `已启用 ${toolName}` : `已关闭 ${toolName}`, { duration: 1200 });
    } catch (e) {
      setLocalDisabled(prev);  // 回滚
      toast.error(`设置失败: ${e}`);
    }
  };

  const loadTools = async () => {
    setLoadingTools(true);
    setToolsErr(null);
    try {
      const r = await fetchServerTools(server.id);
      setTools(r);
    } catch (e) {
      setToolsErr(String(e));
      setTools([]);
    } finally {
      setLoadingTools(false);
    }
  };

  useEffect(() => {
    if (tab === "tools" && tools === null && !loadingTools) {
      void loadTools();
    }
  }, [tab]);

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/50 backdrop-blur-sm" onClick={onClose}>
      <div
        className="w-full max-w-2xl rounded-lg border border-white/10 bg-bg-elev shadow-2xl max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-white/5 px-4 py-3">
          <div>
            <h2 className="text-base font-semibold">{server.name}</h2>
            {server.description && <div className="text-xs text-fg-muted">{server.description}</div>}
          </div>
          <button className="btn-ghost p-1" onClick={onClose}><X size={16} /></button>
        </div>

        {/* tabs */}
        <div className="flex border-b border-white/5 px-2">
          <button
            className={`px-3 py-2 text-sm ${tab === "general" ? "border-b-2 border-accent text-fg" : "text-fg-muted hover:text-fg"}`}
            onClick={() => setTab("general")}
          >
            General Settings
          </button>
          <button
            className={`px-3 py-2 text-sm ${tab === "tools" ? "border-b-2 border-accent text-fg" : "text-fg-muted hover:text-fg"}`}
            onClick={() => setTab("tools")}
          >
            Tools
            {localDisabled.size > 0 && (
              <span className="ml-1 badge-mute text-[10px]">{localDisabled.size} 关闭</span>
            )}
          </button>
        </div>

        <div className="flex-1 overflow-auto p-4">
          {tab === "general" && (
            <div className="space-y-3 text-sm">
              <DetailRow k="传输方式" v={<span className="badge-mute">{server.transport}</span>} />
              {server.transport === "stdio" ? (
                <>
                  <DetailRow k="命令" v={<code className="text-xs">{server.command ?? "—"}</code>} />
                  <DetailRow k="参数" v={
                    <code className="text-xs break-all">
                      {(server.args ?? []).join(" ") || "—"}
                    </code>
                  } />
                  {server.cwd && <DetailRow k="工作目录" v={<code className="text-xs">{server.cwd}</code>} />}
                  {server.env && Object.keys(server.env).length > 0 && (
                    <div className="rounded-md border border-white/10 bg-bg-subtle p-3">
                      <div className="mb-1 text-xs text-fg-muted">环境变量</div>
                      <div className="space-y-1">
                        {Object.entries(server.env).map(([k, v]) => (
                          <div key={k} className="flex gap-2 text-xs">
                            <code className="text-accent">{k}</code>
                            <span className="text-fg-muted">=</span>
                            <code className="break-all">{v}</code>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </>
              ) : (
                <>
                  <DetailRow k="URL" v={<code className="text-xs break-all">{server.url ?? "—"}</code>} />
                  {server.headers && Object.keys(server.headers).length > 0 && (
                    <div className="rounded-md border border-white/10 bg-bg-subtle p-3">
                      <div className="mb-1 text-xs text-fg-muted">请求头</div>
                      <div className="space-y-1">
                        {Object.entries(server.headers).map(([k, v]) => (
                          <div key={k} className="flex gap-2 text-xs">
                            <code className="text-accent">{k}</code>
                            <span className="text-fg-muted">:</span>
                            <code className="break-all">{v}</code>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </>
              )}
              <DetailRow k="自动启动" v={server.auto_start ? "✅ 是" : "❌ 否"} />
              <DetailRow k="状态" v={<span className={server.enabled ? "text-ok" : "text-fg-muted"}>{server.enabled ? "启用" : "禁用"}</span>} />
              <DetailRow k="创建时间" v={<span className="text-xs text-fg-muted">{new Date(server.created_at).toLocaleString()}</span>} />
            </div>
          )}

          {tab === "tools" && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-xs text-fg-muted">
                  {loadingTools ? "正在获取工具列表…" : tools ? `${tools.length} 个工具` : "—"}
                </div>
                <button className="btn-ghost text-xs" onClick={loadTools} disabled={loadingTools}>
                  <RefreshCw size={12} className={loadingTools ? "animate-spin" : ""}/>刷新
                </button>
              </div>

              {toolsErr && (
                <div className="rounded border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">
                  {toolsErr}
                </div>
              )}

              {tools && tools.length === 0 && !loadingTools && (
                <div className="text-xs text-fg-muted">没有获取到工具</div>
              )}

              <div className="space-y-1">
                {tools?.map((t) => {
                  const enabled = !localDisabled.has(t.name);
                  return (
                    <div key={t.name} className={`flex items-start gap-2 rounded-md border px-3 py-2 ${enabled ? "border-white/10 bg-bg-subtle" : "border-white/5 bg-bg-subtle/40 opacity-60"}`}>
                      <button
                        onClick={() => toggleTool(t.name, !enabled)}
                        className={`relative mt-1 h-5 w-9 shrink-0 rounded-full transition-colors ${enabled ? "bg-accent" : "bg-white/10"}`}
                        title={enabled ? "已启用" : "已禁用"}
                      >
                        <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all ${enabled ? "left-4" : "left-0.5"}`} />
                      </button>
                      <div className="min-w-0 flex-1">
                        <div className="text-sm font-medium">{t.name}</div>
                        {t.description && <div className="line-clamp-3 text-xs text-fg-muted">{t.description}</div>}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function DetailRow({ k, v }: { k: string; v: React.ReactNode }) {
  return (
    <div className="flex items-start gap-3">
      <div className="w-24 shrink-0 text-xs text-fg-muted">{k}</div>
      <div className="flex-1 min-w-0">{v}</div>
    </div>
  );
}