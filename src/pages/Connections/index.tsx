/**
 * 客户端接入页:展示 npx 一行配置 / HTTP 直连。
 *
 * 布局:
 *   1. NPX 一键接入卡片(主要操作)
 *   2. 前提条件 + Token(次要信息,折叠)
 *   3. Advanced 详情(Section)
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check, ChevronDown, Terminal, Plug, RefreshCw, Loader2, Info } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { useConnections } from "./useConnections";
import { describeError } from "@/lib/errors";

export function Connections() {
  const { t } = useTranslation();
  const {
    routerConfig, proxy, npxConfig, httpConfig, info,
    revealedToken, revealToken, clearRevealedToken,
    loading, loadError, reload,
  } = useConnections();
  const [copied, setCopied] = useState<string>("");
  const [revealBusy, setRevealBusy] = useState(false);
  const [infoOpen, setInfoOpen] = useState(false);

  const copy = async (text: string | null, key: string, label: string) => {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      toast.warning(t("servers.toast_clipboard_failed", { label }) as string, { duration: 4000 });
      return;
    }
    setCopied(key);
    toast.success(t("connections.copied", { label }), { duration: 1500 });
  };

  const handleReveal = async () => {
    if (revealBusy) return;
    setRevealBusy(true);
    try {
      await revealToken();
    } catch (e) {
      toast.error(t("connections.reveal_failed", { msg: describeError(e) }) as string);
    } finally {
      setRevealBusy(false);
    }
  };

  useEffect(() => {
    if (!copied) return;
    const handle = window.setTimeout(() => setCopied(""), 1500);
    return () => window.clearTimeout(handle);
  }, [copied]);

  const proxyOn = !!routerConfig?.running;

  return (
    <div className="space-y-4 p-6">
      {/* ====== 1. 一键接入卡片 ====== */}
      <div className="overflow-hidden rounded-xl border border-border bg-gradient-to-br from-bg-subtle via-bg-elev to-bg-subtle shadow-sm">
        {/* 头部:状态 + 标题 */}
        <div className="flex items-center justify-between border-b border-border/60 px-6 py-4">
          <div className="flex items-center gap-3">
            <div className={`grid h-10 w-10 place-items-center rounded-xl ${proxyOn ? "bg-ok/10 text-ok" : "bg-err/10 text-err"}`}>
              <Plug size={20} />
            </div>
            <div>
              <h2 className="text-lg font-bold">{t("connections.title")}</h2>
              <span className={`inline-flex items-center gap-1.5 text-xs ${proxyOn ? "text-ok" : "text-err"}`}>
                {loading ? (
                  <><Loader2 size={12} className="animate-spin" />{t("connections.loading")}</>
                ) : (
                  <><span className={`inline-block h-2 w-2 rounded-full ${proxyOn ? "bg-ok" : "bg-err"}`} />{proxyOn ? t("connections.proxy_running") : t("connections.proxy_stopped")}</>
                )}
              </span>
            </div>
          </div>
          {proxy?.endpoint && (
            <code className="rounded-md bg-bg px-3 py-1.5 font-mono text-2xs text-fg-muted">{proxy.endpoint}</code>
          )}
        </div>

        {/* 加载态 / 错误态 */}
        {loading && !loadError && (
          <div className="space-y-3 p-6">
            <div className="h-4 w-2/3 animate-pulse rounded bg-bg-subtle" />
            <div className="h-16 animate-pulse rounded bg-bg-subtle" />
            <div className="h-8 w-32 animate-pulse rounded bg-bg-subtle" />
          </div>
        )}
        {!loading && loadError && (
          <div className="border-b border-err/20 bg-err/5 px-6 py-4">
            <div className="flex items-start gap-3">
              <Info size={16} className="mt-0.5 shrink-0 text-err" />
              <div className="flex-1">
                <div className="font-medium text-err text-sm">{t("connections.load_error_title")}</div>
                <div className="mt-1 text-xs text-fg-muted">{t("connections.load_error_desc")}</div>
              </div>
              <button className="btn-primary text-xs" onClick={reload}>
                <RefreshCw size={14} />{t("connections.retry")}
              </button>
            </div>
          </div>
        )}

        {/* NPX 一行命令 */}
        {!loading && (
          <div className="p-6">
            <div className="mb-3 flex items-center gap-2">
              <Terminal size={16} className="text-accent" />
              <span className="text-sm font-semibold">{t("connections.npx_card_title")}</span>
            </div>
            <div className="relative">
              <pre className="overflow-x-auto rounded-lg border border-border bg-bg px-4 py-3 font-mono text-xs leading-relaxed">
                {npxConfig ?? t("connections.waiting_settings")}
              </pre>
              <button
                className="absolute right-3 top-1/2 -translate-y-1/2 btn-primary"
                onClick={() => npxConfig && copy(npxConfig, "npx", t("connections.npx_card_title"))}
                disabled={!npxConfig}
              >
                {copied === "npx" ? <><Check size={14} />{t("common.copied")}</> : <><Copy size={14} />{t("common.copy")}</>}
              </button>
            </div>
          </div>
        )}
      </div>

      {/* ====== 2. 前提条件 + Token(可折叠) ====== */}
      <div className="overflow-hidden rounded-xl border border-border bg-bg-subtle">
        <button
          className="flex w-full items-center justify-between px-5 py-3.5 text-left transition-colors hover:bg-overlay-soft"
          onClick={() => setInfoOpen((o) => !o)}
        >
          <span className="flex items-center gap-2.5 text-sm font-semibold">
            <Info size={16} className="text-fg-muted" />
            {t("connections.prereq_title")}
          </span>
          <ChevronDown size={16} className={`text-fg-muted transition-transform ${infoOpen ? "rotate-180" : ""}`} />
        </button>
        {infoOpen && (
          <div className="border-t border-border px-5 py-4 space-y-4 text-sm">
            {/* 前提条件 */}
            <div>
              <p className="mb-2 text-xs text-fg-muted">{t("connections.intro")}</p>
              <ul className="grid gap-1.5 sm:grid-cols-2">
                {[
                  t("connections.bullet_no_install"),
                  t("connections.bullet_running"),
                  t("connections.bullet_cross_platform"),
                  t("connections.bullet_shared"),
                ].map((item, i) => (
                  <li key={i} className="flex items-center gap-2 rounded-md bg-bg-elev/50 px-3 py-2 text-xs text-fg-muted">
                    <span className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-accent/10 text-2xs text-accent">{i + 1}</span>
                    {item}
                  </li>
                ))}
              </ul>
            </div>

            {/* 网络/Node/缓存检测 */}
            <div className="rounded-lg border border-border bg-bg px-4 py-3 text-xs text-fg-muted">
              <div className="mb-2 font-medium text-fg">{t("connections.prereq_title")}</div>
              <ul className="space-y-1.5 pl-5 list-disc">
                <li>{t("connections.prereq_node")}</li>
                <li>{t("connections.prereq_network")}</li>
                <li>{t("connections.prereq_cache")}</li>
              </ul>
            </div>

            {/* Token 管理 */}
            <div className="rounded-lg border border-border bg-bg px-4 py-3 text-xs">
              <div className="mb-1.5 font-medium text-fg">{t("connections.token_reveal_title")}</div>
              <p className="mb-2 text-fg-muted">{t("connections.token_reveal_desc")}</p>
              {routerConfig?.token_present ? (
                revealedToken ? (
                  <div className="space-y-2">
                    <code className="block break-all rounded bg-bg-subtle p-2 font-mono text-xs">{revealedToken}</code>
                    <div className="flex gap-2">
                      <button className="btn-ghost text-xs" onClick={() => copy(revealedToken, "token", "Token")}>
                        {copied === "token" ? <><Check size={12} />{t("common.copied")}</> : <><Copy size={12} />{t("common.copy")}</>}
                      </button>
                      <button className="btn-ghost text-xs" onClick={clearRevealedToken}>{t("connections.token_hide")}</button>
                    </div>
                  </div>
                ) : (
                  <button className="btn-primary text-xs disabled:opacity-60" onClick={handleReveal} disabled={revealBusy}>
                    {revealBusy ? <><Loader2 size={12} className="inline animate-spin mr-1" />{t("connections.revealing")}</> : t("connections.token_show_btn")}
                  </button>
                )
              ) : (
                <p className="text-err">{t("connections.token_not_yet")}</p>
              )}
            </div>
          </div>
        )}
      </div>

      {/* ====== 3. Advanced 详情 ====== */}
      {routerConfig && (
        <Section title={t("connections.advanced_title")}>
          <div className="space-y-4 text-sm">
            <span className="text-xs text-fg-muted">{t("connections.advanced_hint")}</span>

            {/* 信息网格 */}
            <div className="grid grid-cols-1 gap-3 rounded-lg border border-border bg-bg px-4 py-3 text-xs sm:grid-cols-2">
              <div>
                <div className="mb-0.5 text-fg-muted">{t("connections.endpoint_label")}</div>
                <code className="break-all font-mono text-fg">{routerConfig.endpoint}</code>
              </div>
              <div>
                <div className="mb-0.5 text-fg-muted">{t("connections.token_label")}</div>
                <code className="break-all font-mono text-fg">
                  {revealedToken ?? routerConfig.token ?? t("settings.token_not_generated")}
                </code>
              </div>
              <div>
                <div className="mb-0.5 text-fg-muted">{t("connections.total_requests")}</div>
                <span className="font-mono font-semibold text-fg">{proxy?.total_requests ?? 0}</span>
              </div>
              <div>
                <div className="mb-0.5 text-fg-muted">{t("connections.last_request")}</div>
                <span className="font-mono text-fg">
                  {proxy?.last_request_at ? new Date(proxy.last_request_at).toLocaleTimeString() : "—"}
                </span>
              </div>
            </div>

            {/* HTTP 直连 */}
            <div className="rounded-lg border border-border bg-bg p-4">
              <div className="mb-2 flex items-center justify-between">
                <span className="flex items-center gap-1.5 text-xs font-medium">
                  <Plug size={14} />
                  {t("connections.http_card_title")}
                </span>
                <button
                  className="btn-ghost text-2xs"
                  onClick={() => httpConfig && copy(httpConfig, "http", t("connections.http_card_title"))}
                >
                  {copied === "http" ? <><Check size={12} />{t("common.copied")}</> : <><Copy size={12} />{t("common.copy")}</>}
                </button>
              </div>
              <pre className="max-h-40 overflow-auto rounded bg-bg-subtle p-3 text-2xs leading-relaxed font-mono">
                {httpConfig}
              </pre>
              <div className="mt-2 text-2xs text-fg-muted">{t("connections.http_instructions")}</div>
            </div>

            {info?.data_dir && (
              <div className="text-xs text-fg-muted">
                {t("connections.data_dir_label")} <code className="text-fg font-mono">{info.data_dir}</code>
              </div>
            )}
          </div>
        </Section>
      )}
    </div>
  );
}
