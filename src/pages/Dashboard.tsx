import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Server, Sparkles, Activity, Wrench, ChevronDown, ChevronRight, Plug, Plus, ArrowRight } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import {
  getProxyStatus, getExposedTools,
  listServerStates, listSkills, appInfo,
} from "@/lib/ipc";
import type { ServerState, Skill } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function Dashboard() {
  const { t } = useTranslation();
  const nav = useNavigate();
  const [proxy, setProxy] = useState<(ReturnType<typeof getProxyStatus> extends Promise<infer T> ? T : never) | null>(null);
  const [servers, setServers] = useState<ServerState[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [info, setInfo] = useState<{ version: string } | null>(null);
  const [exposedTools, setExposedTools] = useState<{ name: string; description?: string; __server: string }[]>([]);
  const [toolsOpen, setToolsOpen] = useState(false);
  // 初始加载标识：首屏未拉到数据时显示骨架，避免闪烁 0 值
  const [initialLoad, setInitialLoad] = useState(true);
  // 任一 IPC 失败时显示错误条 + 重试按钮 —— 之前 4 个 `.catch(() => null)` 把错误全吞了,
  // 用户看到的就是永远 "0" / "未运行",无法判断是后端坏了还是前端没拿到数据。
  const [loadError, setLoadError] = useState<string | null>(null);

  // 静态值（版本号）只取一次
  useEffect(() => {
    appInfo().then((i) => setInfo(i ? { version: i.version } : null)).catch(() => {});
  }, []);

  // 动态状态：5s 轮询（原来 1.5s 且每次还附带静态项 + 文件系统扫描，太重）。
  // 用 inFlightRef 避免上一次轮询还没回就触发下一次 —— 落后响应会覆盖最新数据。
  // 用 document.visibilitychange 在标签页隐藏时停掉轮询(IPC + listener 都是真资源),
  // 回到前台再做一次即时拉取 + 重启 timer。
  const inFlightRef = useRef(false);
  // load 提到 useEffect 外,这样错误条上的「重试」按钮能直接调用它。
  // 每次 IPC 失败时显示具体错原因 + 给用户立即重试的机会,而不是只能等到下一次 5s 轮询。
  const load = useCallback(async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    try {
      // 用 Promise.allSettled 而不是 Promise.all + .catch(()=>null):
      // 这样能区分「后端确实返回了空数组」和「IPC 抛了错」——
      // 空数组不显示错误条,但 IPC 失败必须显示并允许重试。
      const [pr, sr, skr, , etr] = await Promise.allSettled([
        getProxyStatus(),
        listServerStates(),
        listSkills(),
        // getToolsTotal 因为 dashboard 不显示总数,跳过这只 IPC(0 extra round trip)
        Promise.resolve(0) as Promise<number>,
        getExposedTools(),
      ]);
      const firstErr = [pr, sr, skr, etr].find((r) => r.status === "rejected") as PromiseRejectedResult | undefined;
      if (firstErr) {
        setLoadError(describeError(firstErr.reason));
      } else {
        setLoadError(null);
      }
      setProxy(pr.status === "fulfilled" ? pr.value : null);
      setServers(sr.status === "fulfilled" ? sr.value : []);
      setSkills(skr.status === "fulfilled" ? skr.value : []);
      setExposedTools(etr.status === "fulfilled" && Array.isArray(etr.value) ? etr.value : []);
    } finally {
      inFlightRef.current = false;
      setInitialLoad(false);
    }
  }, []);
  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | null = null;
    const startTimer = () => {
      if (timer) return;
      timer = setInterval(() => { void load(); }, 5000);
    };
    const stopTimer = () => {
      if (timer) { clearInterval(timer); timer = null; }
    };
    const handleVisibility = () => {
      if (document.hidden) {
        stopTimer();
      } else {
        // 回前台立即拉一次 + 启 timer —— 用户打开 dashboard 应该立刻看到最新状态
        void load();
        startTimer();
      }
    };
    void load();
    startTimer();
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      stopTimer();
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [load]);

  const runningCount = servers.filter((s) => s.status === "running").length;
  const enabledSkills = skills.filter((s) => s.enabled).length;
  const totalDisabled = servers.reduce((acc, s) => acc + (s.server.disabled_tools?.length ?? 0), 0);
  const proxyOn = !!proxy?.running;
  const dash = (v: number | string) => initialLoad ? "—" : v;

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

      {/* 加载失败提示:任一 IPC 抛错时显示,给用户「具体错什么 + 重试按钮」而不是冷冰冰的 0 */}
      {loadError && (
        <div className="card border-err/40 bg-err/5 p-3 text-sm flex items-center justify-between">
          <span className="text-err">
            {t("dashboard.load_error")}: {loadError}
          </span>
          <button className="btn-ghost" onClick={() => { void load(); }}>
            {t("dashboard.retry")}
          </button>
        </div>
      )}

      {/* Stat cards */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatCard icon={<Server size={16} />} label={t("dashboard.stats.servers_total")} value={dash(servers.length)} bar="bg-fg-muted" />
        <StatCard icon={<Activity size={16} />} label={t("dashboard.stats.servers_running")} value={dash(runningCount)} bar="bg-ok" accent={!initialLoad && runningCount > 0} />
        <StatCard icon={<Sparkles size={16} />} label={t("dashboard.stats.skills_total")} value={dash(skills.length)} bar="bg-warn" />
        <StatCard icon={<Sparkles size={16} />} label={t("dashboard.stats.skills_enabled")} value={dash(enabledSkills)} bar="bg-accent" accent={!initialLoad && enabledSkills > 0} />
      </div>

      {/* 已暴露给客户端的工具（按 server 聚合） */}
      {exposedTools.length > 0 && (
        <div className="card space-y-2">
          <button
            className="flex w-full flex-wrap items-center justify-between gap-y-1 text-left"
            onClick={() => setToolsOpen((o) => !o)}
          >
            <div className="flex items-center gap-2">
              {toolsOpen ? <ChevronDown size={14} className="text-fg-muted" /> : <ChevronRight size={14} className="text-fg-muted" />}
              <Wrench size={16} className="text-accent" />
              <span className="text-sm font-semibold">{t("dashboard.exposed_tools")}</span>
              <span className="badge-mute text-3xs">
                {t("dashboard.exposed_tools_total", { total: exposedTools.length + totalDisabled })}
              </span>
              {totalDisabled > 0 ? (
                <span className="badge-mute text-3xs">
                  {t("dashboard.exposed_tools_breakdown", {
                    enabled: exposedTools.length,
                    disabled: totalDisabled,
                  })}
                </span>
              ) : (
                <span className="badge-mute text-3xs">
                  {t("dashboard.exposed_tools_count", { count: exposedTools.length })}
                </span>
              )}
            </div>
            <span className="text-2xs text-fg-muted">{t("dashboard.exposed_tools_hint")}</span>
          </button>
          {toolsOpen && (
            <div className="max-h-72 space-y-1 overflow-auto">
              {exposedTools.map((t) => (
                <div key={t.name} className="rounded-md border border-border bg-bg-subtle/60 px-3 py-2">
                  <div className="flex items-center gap-2 text-xs">
                    <span className="badge-accent shrink-0 text-3xs">{t.__server}</span>
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
              <div>{t("dashboard.quick_start.empty_desc")}</div>
              <div className="flex justify-center gap-2">
                <button className="btn-primary" onClick={() => nav("/servers")}><Plus size={12}/>{t("dashboard.quick_start.add_server")}</button>
                <button className="btn-ghost" onClick={() => nav("/skills")}><Sparkles size={12}/>{t("dashboard.quick_start.add_skill")}</button>
                <button className="btn-ghost" onClick={() => nav("/connections")}>{t("dashboard.action_connect_title")} →</button>
              </div>
            </div>
          </EmptyHint>
        ) : (
          <div className="grid gap-3 sm:grid-cols-3">
            <ActionCard
              icon={<Plus size={18} />}
              title={t("dashboard.action_add_mcp_title")}
              desc={t("dashboard.stats.servers_total") + `: ${servers.length}`}
              onClick={() => nav("/servers")}
            />
            <ActionCard
              icon={<Sparkles size={18} />}
              title={t("dashboard.action_add_skill_title")}
              desc={t("dashboard.stats.skills_total") + `: ${skills.length}`}
              onClick={() => nav("/skills")}
            />
            <ActionCard
              icon={<Plug size={18} />}
              title={t("dashboard.action_connect_title")}
              desc={t("dashboard.action_connect_desc")}
              onClick={() => nav("/connections")}
            />
          </div>
        )}
      </Section>
    </div>
  );
}

function StatCard({ icon, label, value, accent, bar }: { icon: React.ReactNode; label: string; value: number | string; accent?: boolean; bar: string }) {
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
      className="group flex items-center gap-3 rounded-lg border border-border bg-bg-subtle p-4 text-left transition-colors hover:border-accent/50 hover:bg-bg-elev"
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
