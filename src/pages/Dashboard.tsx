import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Server, Sparkles, Activity, Wrench, ChevronDown, ChevronRight, Plug, Plus, ArrowRight } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import {
  getProxyStatus, getToolsTotal, getExposedTools,
  listServerStates, listSkills, appInfo,
} from "@/lib/ipc";
import type { ServerState, Skill } from "@/lib/ipc";

export function Dashboard() {
  const { t } = useTranslation();
  const nav = useNavigate();
  const [proxy, setProxy] = useState<(ReturnType<typeof getProxyStatus> extends Promise<infer T> ? T : never) | null>(null);
  const [servers, setServers] = useState<ServerState[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [info, setInfo] = useState<{ version: string } | null>(null);
  const [toolsTotal, setToolsTotal] = useState(0);
  const [exposedTools, setExposedTools] = useState<{ name: string; description?: string; __server: string }[]>([]);
  const [toolsOpen, setToolsOpen] = useState(false);

  const load = async () => {
    const [p, ss, sk, i, tt, et] = await Promise.all([
      getProxyStatus().catch(() => null),
      listServerStates().catch(() => []),
      listSkills().catch(() => []),
      appInfo().catch(() => null),
      getToolsTotal().catch(() => 0),
      getExposedTools().catch(() => []),
    ]);
    setProxy(p as any);
    setServers(ss);
    setSkills(sk);
    setInfo(i ? { version: i.version } : null);
    setToolsTotal(typeof tt === "number" ? tt : 0);
    setExposedTools(Array.isArray(et) ? et : []);
  };

  useEffect(() => {
    void load();
    const t = setInterval(() => { void load(); }, 1500);
    return () => clearInterval(t);
  }, []);

  const runningCount = servers.filter((s) => s.status === "running").length;
  const enabledSkills = skills.filter((s) => s.enabled).length;
  const totalDisabled = servers.reduce((acc, s) => acc + (s.server.disabled_tools?.length ?? 0), 0);
  const proxyOn = !!proxy?.running;

  return (
    <div className="space-y-6 p-6">
      {/* Hero 横幅 */}
      <div className="relative overflow-hidden rounded-xl border border-border bg-gradient-to-r from-bg-subtle to-bg-elev p-6">
        <div className="absolute right-0 top-0 h-full w-1/3 bg-gradient-to-l from-accent/10 to-transparent" />
        <div className="relative flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h1 className="text-2xl font-bold">{t("dashboard.welcome")}</h1>
            <p className="text-sm text-fg-muted">{t("app.tagline")}</p>
            <div className="mt-3 flex flex-wrap gap-2 text-xs">
              <span className={`badge ${proxyOn ? "badge-ok" : "badge-mute"}`}>
                <span className={`inline-block h-1.5 w-1.5 rounded-full ${proxyOn ? "bg-ok animate-pulse" : "bg-fg-muted"}`} />
                {proxyOn ? t("dashboard.proxy_running") : t("dashboard.proxy_stopped")}
              </span>
              {proxy?.endpoint && (
                <span className="badge-mute">
                  <code className="font-mono">{proxy.endpoint}</code>
                </span>
              )}
            </div>
          </div>
          {info && (
            <div className="shrink-0 text-right text-xs text-fg-muted">
              <div className="text-fg-muted">{t("settings.version")}</div>
              <div className="font-mono text-fg">{info.version}</div>
            </div>
          )}
        </div>
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatCard icon={<Server size={16} />} label={t("dashboard.stats.servers_total")} value={servers.length} bar="bg-fg-muted" />
        <StatCard icon={<Activity size={16} />} label={t("dashboard.stats.servers_running")} value={runningCount} bar="bg-ok" accent={runningCount > 0} />
        <StatCard icon={<Sparkles size={16} />} label={t("dashboard.stats.skills_total")} value={skills.length} bar="bg-warn" />
        <StatCard icon={<Sparkles size={16} />} label={t("dashboard.stats.skills_enabled")} value={enabledSkills} bar="bg-accent" accent={enabledSkills > 0} />
      </div>

      {/* 已暴露给客户端的工具（按 server 聚合） */}
      {exposedTools.length > 0 && (
        <div className="card space-y-2">
          <button
            className="flex w-full items-center justify-between text-left"
            onClick={() => setToolsOpen((o) => !o)}
          >
            <div className="flex items-center gap-2">
              {toolsOpen ? <ChevronDown size={14} className="text-fg-muted" /> : <ChevronRight size={14} className="text-fg-muted" />}
              <Wrench size={16} className="text-accent" />
              <span className="text-sm font-semibold">已暴露的工具</span>
              <span className="badge-mute text-[10px]">总数 {exposedTools.length + totalDisabled}</span>
              {totalDisabled > 0 ? (
                <span className="badge-mute text-[10px]">{exposedTools.length} 个启用 · {totalDisabled} 已关闭</span>
              ) : (
                <span className="badge-mute text-[10px]">{exposedTools.length} 个</span>
              )}
            </div>
            <span className="text-[11px] text-fg-muted">Qoder / Claude Code / Cursor 都能调用</span>
          </button>
          {toolsOpen && (
            <div className="max-h-72 space-y-1 overflow-auto">
              {exposedTools.map((t) => (
                <div key={t.name} className="rounded-md border border-border bg-bg-subtle/60 px-3 py-2">
                  <div className="flex items-center gap-2 text-xs">
                    <span className="badge-accent shrink-0 text-[10px]">{t.__server}</span>
                    <code className="font-mono text-fg">{t.name}</code>
                  </div>
                  {t.description && (
                    <div className="mt-1 line-clamp-2 text-xs text-fg-muted">{t.description}</div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* 快速操作卡片 */}
      <Section title={t("dashboard.quick_start.title")}>
        {servers.length === 0 && skills.length === 0 ? (
          <EmptyHint>
            <div className="space-y-3">
              <div>{t("dashboard.quick_start.add_server")}</div>
              <div className="flex justify-center gap-2">
                <button className="btn-primary" onClick={() => nav("/servers")}>{t("dashboard.quick_start.add_server")}</button>
                <button className="btn-ghost" onClick={() => nav("/skills")}>{t("dashboard.quick_start.add_skill")}</button>
                <button className="btn-ghost" onClick={() => nav("/connections")}>客户端接入 →</button>
              </div>
            </div>
          </EmptyHint>
        ) : (
          <div className="grid gap-3 sm:grid-cols-3">
            <ActionCard
              icon={<Plus size={18} />}
              title={t("common.add") + " MCP"}
              desc={t("dashboard.stats.servers_total") + `：${servers.length}`}
              onClick={() => nav("/servers")}
            />
            <ActionCard
              icon={<Sparkles size={18} />}
              title={t("common.add") + " Skill"}
              desc={t("dashboard.stats.skills_total") + `：${skills.length}`}
              onClick={() => nav("/skills")}
            />
            <ActionCard
              icon={<Plug size={18} />}
              title="客户端接入"
              desc="Qoder / Claude Code / Cursor"
              onClick={() => nav("/connections")}
            />
          </div>
        )}
      </Section>
    </div>
  );
}

function StatCard({ icon, label, value, accent, bar }: { icon: React.ReactNode; label: string; value: number; accent?: boolean; bar: string }) {
  return (
    <div className="card relative overflow-hidden pl-5 transition-transform hover:-translate-y-0.5">
      <div className={`absolute left-0 top-0 h-full w-1 ${bar}`} />
      <div className="flex items-center gap-2 text-xs text-fg-muted">{icon}{label}</div>
      <div className={`mt-1 text-2xl font-semibold ${accent ? "text-accent" : ""}`}>{value}</div>
    </div>
  );
}

function ActionCard({ icon, title, desc, onClick }: { icon: React.ReactNode; title: string; desc: string; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="group flex items-center gap-3 rounded-lg border border-border bg-bg-subtle p-4 text-left transition-all hover:border-accent/50 hover:bg-bg-elev"
    >
      <div className="grid h-10 w-10 shrink-0 place-items-center rounded-md bg-accent/15 text-accent transition-colors group-hover:bg-accent group-hover:text-white">
        {icon}
      </div>
      <div className="flex-1">
        <div className="text-sm font-semibold">{title}</div>
        <div className="text-xs text-fg-muted">{desc}</div>
      </div>
      <ArrowRight size={16} className="text-fg-muted transition-transform group-hover:translate-x-1" />
    </button>
  );
}
