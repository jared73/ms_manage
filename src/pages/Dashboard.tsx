import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Server, Sparkles, Plug, Activity, Copy, RefreshCw, FileCode } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import {
  getProxyStatus, getRouterConfig,
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
  const [routerConfig, setRouterConfig] = useState<Awaited<ReturnType<typeof getRouterConfig>> | null>(null);
  const [copied, setCopied] = useState<string>("");

  const load = async () => {
    const [p, ss, sk, i, rc] = await Promise.all([
      getProxyStatus().catch(() => null),
      listServerStates().catch(() => []),
      listSkills().catch(() => []),
      appInfo().catch(() => null),
      getRouterConfig().catch(() => null),
    ]);
    setProxy(p as any);
    setServers(ss);
    setSkills(sk);
    setInfo(i ? { version: i.version } : null);
    setRouterConfig(rc);
  };

  useEffect(() => {
    // 1) 立即拉一次（不等），如果拿到 running=true 就直接停
    // 2) 否则每 500ms 轮询一次，最多 10 次（5 秒兜底）
    // 3) 兜底之后如果还没成功，保持 1.5s 慢轮询
    let count = 0;
    let stopped = false;
    const tryLoad = async () => {
      if (stopped) return;
      await load();
      count++;
      // load() 后 proxy 已是 running=true 就停
      if (count > 50) stopped = true;  // 兜底 25s 后强制停
    };
    const fast = setInterval(tryLoad, 500);
    return () => clearInterval(fast);
  }, []);

  const copy = (text: string, key: string) => {
    navigator.clipboard.writeText(text);
    setCopied(key);
    setTimeout(() => setCopied(""), 1500);
  };

  const runningCount = servers.filter((s) => s.status === "running").length;
  const enabledSkills = skills.filter((s) => s.enabled).length;

  return (
    <div className="space-y-6 p-6">
      <Section title={t("dashboard.welcome")} />

      {/* MCP Router 服务卡片（学 mcp-router） */}
      <div className="card space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className={`grid h-10 w-10 place-items-center rounded-lg ${proxy?.running ? "bg-ok/15 text-ok" : "bg-white/5 text-fg-muted"}`}>
              <Plug size={20} />
            </div>
            <div>
              <div className="font-semibold">MCP Router 服务</div>
              <div className="text-xs text-fg-muted">
                {proxy?.running
                  ? <>运行中 · endpoint <code className="text-fg">{proxy.endpoint}</code></>
                  : "未运行（启动失败请看日志，关掉重开 GUI 会自动重试）"}
              </div>
            </div>
          </div>
          {/* 启动/停止按钮已移除 —— 跟 mcp-router 一样，代理跟 GUI 生命周期绑定 */}
        </div>
        {routerConfig && (
          <div className="space-y-2 rounded-md bg-bg-subtle p-3">
            <div className="grid grid-cols-1 gap-2 text-xs md:grid-cols-2">
              <div>
                <span className="text-fg-muted">HTTP endpoint：</span>
                <code className="text-fg">{routerConfig.endpoint}</code>
              </div>
              <div>
                <span className="text-fg-muted">Token：</span>
                <code className="text-fg break-all">{routerConfig.token}</code>
                <button
                  className="btn-ghost ml-2 px-2 py-0.5 text-[11px]"
                  onClick={() => copy(routerConfig.token, "token")}
                >
                  {copied === "token" ? "✓" : <Copy size={10}/>}
                </button>
              </div>
              <div>
                <span className="text-fg-muted">已处理请求：</span>
                <span className="text-fg">{proxy?.total_requests ?? 0}</span>
              </div>
              <div>
                <span className="text-fg-muted">最近请求：</span>
                <span className="text-fg">{proxy?.last_request_at ? new Date(proxy.last_request_at).toLocaleTimeString() : "—"}</span>
              </div>
            </div>
            <div className="mt-2">
              <div className="mb-1 flex items-center justify-between text-[11px] text-fg-muted">
                <span className="flex items-center gap-1"><FileCode size={11}/>mcpServers JSON（直接给客户端用）</span>
                <button
                  className="btn-ghost px-2 py-0.5 text-[11px]"
                  onClick={() => copy(JSON.stringify({ mcpServers: routerConfig.mcpServers }, null, 2), "json")}
                >
                  {copied === "json" ? "✓ 已复制" : <><Copy size={10}/>复制</>}
                </button>
              </div>
              <pre className="max-h-40 overflow-auto rounded bg-bg p-3 text-[11px] leading-relaxed">
{JSON.stringify({ mcpServers: routerConfig.mcpServers }, null, 2)}
              </pre>
              <div className="mt-2 text-[11px] text-fg-muted">
                {routerConfig.instructions}
              </div>
            </div>
          </div>
        )}
        <div className="flex flex-wrap gap-2 border-t border-white/5 pt-2">
          <button className="btn-ghost text-xs" onClick={load}><RefreshCw size={12}/>刷新</button>
          <button className="btn-ghost text-xs" onClick={() => nav("/claude")}>Claude Desktop 集成 →</button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatCard icon={<Server size={16} />} label={t("dashboard.stats.servers_total")} value={servers.length} />
        <StatCard icon={<Activity size={16} />} label={t("dashboard.stats.servers_running")} value={runningCount} accent={runningCount > 0} />
        <StatCard icon={<Sparkles size={16} />} label={t("dashboard.stats.skills_total")} value={skills.length} />
        <StatCard icon={<Sparkles size={16} />} label={t("dashboard.stats.skills_enabled")} value={enabledSkills} accent={enabledSkills > 0} />
      </div>

      {/* Quick start */}
      <Section title={t("dashboard.quick_start.title")}>
        <>
        {servers.length === 0 && skills.length === 0 ? (
          <EmptyHint>
            <div className="space-y-2">
              <div>{t("dashboard.quick_start.add_server")}</div>
              <div className="flex justify-center gap-2">
                <button className="btn-primary" onClick={() => nav("/servers")}>{t("dashboard.quick_start.add_server")}</button>
                <button className="btn-ghost" onClick={() => nav("/skills")}>{t("dashboard.quick_start.add_skill")}</button>
                <button className="btn-ghost" onClick={() => nav("/claude")}>{t("dashboard.quick_start.connect_claude")}</button>
              </div>
            </div>
          </EmptyHint>
        ) : (
          <div className="flex gap-2">
            <button className="btn-ghost" onClick={() => nav("/servers")}>{t("common.add")} MCP</button>
            <button className="btn-ghost" onClick={() => nav("/skills")}>{t("common.add")} Skill</button>
            <button className="btn-ghost" onClick={() => nav("/market")}>{t("nav.market")}</button>
          </div>
        )}
        </>
      </Section>

      {info && (
        <div className="text-xs text-fg-muted">{t("settings.version")}: {info.version}</div>
      )}
    </div>
  );
}

function StatCard({ icon, label, value, accent }: { icon: React.ReactNode; label: string; value: number; accent?: boolean }) {
  return (
    <div className="card">
      <div className="flex items-center gap-2 text-xs text-fg-muted">{icon}{label}</div>
      <div className={`mt-1 text-2xl font-semibold ${accent ? "text-accent" : ""}`}>{value}</div>
    </div>
  );
}
