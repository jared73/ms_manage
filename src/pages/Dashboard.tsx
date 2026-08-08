import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import {
  Server, Zap, Activity, Wrench, ChevronDown,
  Share2, Plus, ArrowRight, Blocks, Wallet, ArrowUpRight,
} from "lucide-react";
import { toast } from "sonner";
import { Section, EmptyHint } from "@/components/Section";
import {
  getProxyStatus, getExposedTools,
  listServerStates, listSkills, appInfo,
  getUsageSummary,
} from "@/lib/ipc";
import type { ServerState, Skill, UsageSummary } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

// 今日 0 点的本地 epoch 秒（与 Usage 页 rangeToEpoch 对齐）
function todayStartEpoch(): number {
  const now = new Date();
  return Math.floor(new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000);
}

const FAILURE_RATE_THRESHOLD = 0.2;

export function Dashboard() {
  const { t, i18n } = useTranslation();
  const nav = useNavigate();
  const [proxy, setProxy] = useState<(ReturnType<typeof getProxyStatus> extends Promise<infer T> ? T : never) | null>(null);
  const [servers, setServers] = useState<ServerState[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [info, setInfo] = useState<{ version: string } | null>(null);
  const [exposedTools, setExposedTools] = useState<{ name: string; description?: string; __server: string }[]>([]);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [todayUsage, setTodayUsage] = useState<UsageSummary | null>(null);
  // 初始加载标识：首屏未拉到数据时显示骨架，避免闪烁 0 值
  const [initialLoad, setInitialLoad] = useState(true);
  // 任一 IPC 失败时显示错误条 + 重试按钮 —— 之前 4 个 `.catch(() => null)` 把错误全吞了,
  // 用户看到的就是永远 "0" / "未运行",无法判断是后端坏了还是前端没拿到数据。
  const [loadError, setLoadError] = useState<string | null>(null);
  // 失败率告警只用 useRef 触发一次,避免 5s 轮询刷屏
  const failureAlertedRef = useRef(false);

  const compactFmt = useMemo(
    () => new Intl.NumberFormat(i18n.language, { notation: "compact", maximumFractionDigits: 1 }),
    [i18n.language],
  );
  const intFmt = useMemo(
    () =>
      new Intl.NumberFormat(i18n.language, {
        maximumFractionDigits: 0,
      }),
    [i18n.language],
  );
  const costFmt = useMemo(
    () => new Intl.NumberFormat(i18n.language, { minimumFractionDigits: 4, maximumFractionDigits: 4 }),
    [i18n.language],
  );

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
      const [pr, sr, skr, , etr, ur] = await Promise.allSettled([
        getProxyStatus(),
        listServerStates(),
        listSkills(),
        // getToolsTotal 因为 dashboard 不显示总数,跳过这只 IPC(0 extra round trip)
        Promise.resolve(0) as Promise<number>,
        getExposedTools(),
        getUsageSummary(todayStartEpoch()),
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
      const u = ur.status === "fulfilled" ? ur.value : null;
      setTodayUsage(u);
      // 失败率告警：今日失败率超 20% 触发一次 toast
      if (u && u.requestCount > 0 && u.failureRate > FAILURE_RATE_THRESHOLD && !failureAlertedRef.current) {
        failureAlertedRef.current = true;
        toast.warning(t("dashboard.alert_high_failure_rate", {
          rate: (u.failureRate * 100).toFixed(1),
        }));
      }
    } finally {
      inFlightRef.current = false;
      setInitialLoad(false);
    }
  }, [t]);
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
      <div className="relative overflow-hidden rounded-2xl border border-border bg-gradient-to-br from-bg-subtle via-bg-elev to-bg-subtle">
        {/* 装饰光斑 */}
        <div className="pointer-events-none absolute -right-16 -top-16 h-48 w-48 rounded-full bg-accent/8 blur-3xl" />
        <div className="pointer-events-none absolute -bottom-12 -left-12 h-36 w-36 rounded-full bg-ok/6 blur-2xl" />
        <div className="relative p-7">
          <div className="flex items-start justify-between gap-6">
            <div className="space-y-2">
              <h1 className="text-2xl font-bold tracking-tight">{t("dashboard.welcome")}</h1>
              <p className="text-sm text-fg-muted">{t("app.tagline")}</p>
              <div className="mt-4 flex flex-wrap items-center gap-2.5 text-xs">
                <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 font-medium ${proxyOn ? "bg-ok/12 text-ok" : "bg-surface-muted/40 text-fg-muted"}`}>
                  <span className={`inline-block h-2 w-2 rounded-full ${proxyOn ? "bg-ok shadow-sm shadow-ok/40 animate-pulse" : "bg-fg-muted/50"}`} />
                  {proxyOn ? t("dashboard.proxy_running") : t("dashboard.proxy_stopped")}
                </span>
                {proxy?.endpoint && (
                  <code className="rounded-md bg-surface-muted/30 px-2 py-1 font-mono text-2xs text-fg-muted">{proxy.endpoint}</code>
                )}
              </div>
            </div>
            {info && (
              <div className="shrink-0 rounded-lg border border-border bg-bg-subtle/60 px-3 py-2 text-center">
                <div className="text-2xs text-fg-muted">{t("settings.version")}</div>
                <div className="mt-0.5 font-mono text-sm font-semibold text-fg">v{info.version}</div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 加载失败提示 */}
      {loadError && (
        <div className="rounded-xl border border-err/30 bg-err/5 px-4 py-3 text-sm flex items-center justify-between">
          <span className="text-err">{t("dashboard.load_error")}: {loadError}</span>
          <button className="btn-ghost text-err" onClick={() => { void load(); }}>{t("dashboard.retry")}</button>
        </div>
      )}

      {/* 今日用量迷你卡：3 项聚合 + 跳转 Usage */}
      <div className="rounded-xl border border-border bg-bg-subtle">
        <div className="flex items-center justify-between px-4 pt-3">
          <div className="text-xs font-medium text-fg-muted">{t("dashboard.usage_mini_title")}</div>
          <button
            onClick={() => nav("/usage")}
            className="flex items-center gap-1 text-2xs text-fg-muted hover:text-accent"
          >
            {t("dashboard.usage_mini_link")}
            <ArrowUpRight size={12} />
          </button>
        </div>
        <div className="grid grid-cols-3 divide-x divide-border px-2 py-3">
          <MiniCell
            icon={<Wallet size={14} />}
            label={t("dashboard.today_cost")}
            value={todayUsage ? (`¥${costFmt.format(Number(todayUsage.totalCost))}` as string) : dash("—")}
            color="accent"
          />
          <MiniCell
            label={t("dashboard.today_requests")}
            value={todayUsage ? (intFmt.format(todayUsage.requestCount) as string) : dash("—")}
          />
          <MiniCell
            label={t("dashboard.today_tokens")}
            value={todayUsage ? (compactFmt.format(todayUsage.totalTokens) as string) : dash("—")}
            sub={todayUsage ? (intFmt.format(todayUsage.totalTokens) as string) : undefined}
          />
        </div>
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatCard
          icon={<Server size={18} />}
          label={t("dashboard.stats.servers_total")}
          value={dash(servers.length)}
          color="fg"
        />
        <StatCard
          icon={<Activity size={18} />}
          label={t("dashboard.stats.servers_running")}
          value={dash(runningCount)}
          color="ok"
          accent={!initialLoad && runningCount > 0}
        />
        <StatCard
          icon={<Zap size={18} />}
          label={t("dashboard.stats.skills_total")}
          value={dash(skills.length)}
          color="warn"
        />
        <StatCard
          icon={<Blocks size={18} />}
          label={t("dashboard.stats.skills_enabled")}
          value={dash(enabledSkills)}
          color="accent"
          accent={!initialLoad && enabledSkills > 0}
        />
      </div>

      {/* 已暴露给客户端的工具 */}
      {exposedTools.length > 0 && (
        <div className="rounded-xl border border-border bg-bg-subtle">
          <button
            className="flex w-full items-center gap-2.5 px-5 py-3.5 text-left transition-colors hover:bg-bg-elev/50 cursor-pointer"
            onClick={() => setToolsOpen((o) => !o)}
          >
            <div className="grid h-7 w-7 shrink-0 place-items-center rounded-md bg-accent/12 text-accent">
              <Wrench size={14} />
            </div>
            <span className="text-sm font-semibold">{t("dashboard.exposed_tools")}</span>
            <span className="badge-mute text-3xs">
              {t("dashboard.exposed_tools_total", { total: exposedTools.length + totalDisabled })}
            </span>
            {totalDisabled > 0 && (
              <span className="badge-mute text-3xs">
                {t("dashboard.exposed_tools_breakdown", { enabled: exposedTools.length, disabled: totalDisabled })}
              </span>
            )}
            <span className="ml-auto shrink-0 text-2xs text-fg-muted">{t("dashboard.exposed_tools_hint")}</span>
            <ChevronDown
              size={15}
              className={`shrink-0 text-fg-muted transition-transform duration-200 ${toolsOpen ? "rotate-0" : "-rotate-90"}`}
            />
          </button>
          {toolsOpen && (
            <div className="max-h-64 overflow-y-auto border-t border-border px-5 py-3 space-y-1">
              {exposedTools.map((t) => (
                <div key={t.name} className="rounded-lg border border-border/60 bg-bg-elev/40 px-3.5 py-2.5 transition-colors hover:bg-bg-elev">
                  <div className="flex items-center gap-2 text-xs">
                    <span className="badge-accent shrink-0 text-3xs">{t.__server}</span>
                    <code className="font-mono text-fg">{t.name}</code>
                  </div>
                  {t.description && (
                    <div className="mt-1 line-clamp-2 pl-[52px] text-xs text-fg-muted">{t.description}</div>
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
            <div className="space-y-4">
              <div className="text-fg-muted">{t("dashboard.quick_start.empty_desc")}</div>
              <div className="flex justify-center gap-2">
                <button className="btn-primary" onClick={() => nav("/servers")}><Plus size={12}/>{t("dashboard.quick_start.add_server")}</button>
                <button className="btn-ghost" onClick={() => nav("/skills")}><Zap size={12}/>{t("dashboard.quick_start.add_skill")}</button>
                <button className="btn-ghost" onClick={() => nav("/connections")}>{t("dashboard.action_connect_title")} →</button>
              </div>
            </div>
          </EmptyHint>
        ) : (
          <div className="grid gap-3 sm:grid-cols-3">
            <ActionCard
              icon={<Server size={20} />}
              title={t("dashboard.action_add_mcp_title")}
              desc={t("dashboard.stats.servers_total") + `: ${servers.length}`}
              color="accent"
              onClick={() => nav("/servers")}
            />
            <ActionCard
              icon={<Zap size={20} />}
              title={t("dashboard.action_add_skill_title")}
              desc={t("dashboard.stats.skills_total") + `: ${skills.length}`}
              color="ok"
              onClick={() => nav("/skills")}
            />
            <ActionCard
              icon={<Share2 size={20} />}
              title={t("dashboard.action_connect_title")}
              desc={t("dashboard.action_connect_desc")}
              color="warn"
              onClick={() => nav("/connections")}
            />
          </div>
        )}
      </Section>
    </div>
  );
}

const colorMap = {
  fg:     { bg: "bg-fg-muted/10",    text: "text-fg-muted", bar: "bg-fg-muted" },
  ok:     { bg: "bg-ok/10",          text: "text-ok",       bar: "bg-ok" },
  warn:   { bg: "bg-warn/10",        text: "text-warn",     bar: "bg-warn" },
  accent: { bg: "bg-accent/10",      text: "text-accent",   bar: "bg-accent" },
} as const;

function StatCard({ icon, label, value, accent, color = "fg" }: { icon: React.ReactNode; label: string; value: number | string; accent?: boolean; color?: keyof typeof colorMap }) {
  const c = colorMap[color];
  return (
    <div className="group relative overflow-hidden rounded-xl border border-border bg-bg-subtle p-4 transition-all hover:-translate-y-0.5 hover:shadow-md hover:shadow-accent/5">
      <div className="flex items-center gap-3">
        <div className={`grid h-9 w-9 place-items-center rounded-lg ${c.bg} ${c.text} transition-colents`}>
          {icon}
        </div>
        <div>
          <div className="text-2xs text-fg-muted">{label}</div>
          <div className={`mt-0.5 text-2xl font-bold tracking-tight ${accent ? "text-accent" : "text-fg"}`}>{value}</div>
        </div>
      </div>
    </div>
  );
}

function MiniCell({
  icon,
  label,
  value,
  sub,
  color = "fg",
}: {
  icon?: React.ReactNode;
  label: string;
  value: string | number;
  sub?: string | number;
  color?: "fg" | "accent";
}) {
  return (
    <div className="flex flex-col items-center justify-center px-3 py-1">
      <div className="flex items-center gap-1.5 text-2xs text-fg-muted">
        {icon}
        {label}
      </div>
      <div className={`mt-0.5 text-lg font-bold tracking-tight ${color === "accent" ? "text-accent" : "text-fg"}`}>
        {value}
      </div>
      {sub != null && <div className="text-2xs text-fg-muted/70">{sub}</div>}
    </div>
  );
}

const actionColorMap = {
  accent: { bg: "bg-accent/10",  hoverBg: "group-hover:bg-accent",  border: "hover:border-accent/40",  text: "text-accent" },
  ok:     { bg: "bg-ok/10",      hoverBg: "group-hover:bg-ok",      border: "hover:border-ok/40",      text: "text-ok" },
  warn:   { bg: "bg-warn/10",    hoverBg: "group-hover:bg-warn",    border: "hover:border-warn/40",    text: "text-warn" },
} as const;

function ActionCard({ icon, title, desc, onClick, color = "accent" }: { icon: React.ReactNode; title: string; desc: string; onClick: () => void; color?: keyof typeof actionColorMap }) {
  const c = actionColorMap[color];
  return (
    <button
      onClick={onClick}
      className={`group flex items-center gap-4 rounded-xl border border-border bg-bg-subtle p-5 text-left transition-all hover:-translate-y-0.5 hover:shadow-lg hover:shadow-accent/5 ${c.border}`}
    >
      <div className={`grid h-12 w-12 shrink-0 place-items-center rounded-xl ${c.bg} ${c.text} transition-all ${c.hoverBg} group-hover:text-white group-hover:shadow-md`}>
        {icon}
      </div>
      <div className="flex-1">
        <div className="text-sm font-semibold">{title}</div>
        <div className="mt-0.5 text-xs text-fg-muted">{desc}</div>
      </div>
      <ArrowRight size={16} className="text-fg-muted transition-all group-hover:translate-x-1 group-hover:text-fg" />
    </button>
  );
}
