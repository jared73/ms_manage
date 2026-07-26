/**
 * 客户端接入页:展示 npx 一行配置 / HTTP 直连(Advanced)。
 *
 * 设计:
 * - 中英文文案全部走 t(),避免硬编码
 * - 数据获取 + 计算复制目标在 useConnections hook 中
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Check, ChevronDown, Terminal, Plug, RefreshCw, Loader2 } from "lucide-react";
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

  const copy = async (text: string | null, key: string, label: string) => {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // 剪贴板写入失败(浏览器策略 / 非安全上下文 / 权限被拒)——
      // 之前是 .catch(() => {}) 静默吞掉,用户以为已复制实际没有。这里给个明确的 fallback toast。
      toast.warning(
        t("servers.toast_clipboard_failed", { label }) as string,
        { duration: 4000 },
      );
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

  // 1500ms 后清掉 "已复制" 标记;组件卸载/再次点击时清掉旧 timer,避免对已卸载组件 setState
  useEffect(() => {
    if (!copied) return;
    const handle = window.setTimeout(() => setCopied(""), 1500);
    return () => window.clearTimeout(handle);
  }, [copied]);

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("connections.title")}
        action={
          <div className="flex items-center gap-3">
            <span className="inline-flex items-center gap-1.5 text-xs text-fg-muted">
              {loading ? (
                <>
                  <Loader2 size={12} className="animate-spin" />
                  {t("connections.loading")}
                </>
              ) : (
                <>
                  <span
                    className={`inline-block h-2 w-2 rounded-full ${
                      routerConfig?.running ? "bg-ok" : "bg-err"
                    }`}
                  />
                  {routerConfig?.running ? t("connections.proxy_running") : t("connections.proxy_stopped")}
                </>
              )}
            </span>
            <button
              className="btn-primary"
              onClick={() => npxConfig && copy(npxConfig, "npx", t("connections.copy_config_npx"))}
              disabled={!npxConfig}
              title={t("connections.copy_tooltip") as string}
            >
              {copied === "npx" ? (
                <span><Check size={12} aria-hidden="true" /> {t("common.copied")}</span>
              ) : (
                <>
                  <Copy size={14} />
                  {t("connections.copy_config")}
                </>
              )}
            </button>
          </div>
        }
      >
        {/* 全页骨架:首屏数据未返回时显示,避免空内容 */}
        {loading && !loadError && (
          <div className="space-y-3">
            <div className="h-4 w-2/3 animate-pulse rounded bg-bg-subtle" />
            <div className="h-4 w-1/2 animate-pulse rounded bg-bg-subtle" />
            <div className="h-24 animate-pulse rounded bg-bg-subtle" />
            <div className="h-32 animate-pulse rounded bg-bg-subtle" />
          </div>
        )}
        {/* 拉取失败:显示错误提示 + 重试按钮 */}
        {!loading && loadError && (
          <div className="rounded-md border border-err/40 bg-err/5 p-4 text-sm">
            <div className="mb-2 font-medium text-err">{t("connections.load_error_title")}</div>
            <div className="mb-3 text-xs text-fg-muted">{t("connections.load_error_desc")}</div>
            <button className="btn-primary" onClick={reload}>
              <RefreshCw size={14} />
              {t("connections.retry")}
            </button>
          </div>
        )}
        {/* 拉到数据后才显示内容(loading 期间显示骨架;loadError 显示错误 UI) */}
        {!loading && (
        <div className="space-y-3 text-sm">
          <p className="text-fg-muted">{t("connections.intro")}</p>

          <ul className="list-disc space-y-1 pl-5 text-xs text-fg-muted">
            <li>{t("connections.bullet_no_install")}</li>
            <li>{t("connections.bullet_running")}</li>
            <li>{t("connections.bullet_cross_platform")}</li>
            <li>{t("connections.bullet_shared")}</li>
          </ul>

          <div className="rounded-lg border border-border bg-bg-subtle px-4 py-3 text-xs text-fg-muted">
            <div className="mb-1.5 font-medium text-fg">{t("connections.prereq_title")}</div>
            <ul className="list-disc space-y-1 pl-5">
              <li>{t("connections.prereq_node")}</li>
              <li>{t("connections.prereq_network")}</li>
              <li>{t("connections.prereq_cache")}</li>
            </ul>
          </div>

          <div className="rounded-lg border border-border bg-bg-subtle px-4 py-3 text-xs text-fg-muted">
          <div className="mb-1.5 font-medium text-fg">{t("connections.token_reveal_title")}</div>
          <p className="mb-2">{t("connections.token_reveal_desc")}</p>
          {routerConfig?.token_present && (
            revealedToken ? (
              <button className="btn-ghost text-xs" onClick={clearRevealedToken}>
                {t("connections.token_hide")}
              </button>
            ) : (
              <button
                className="btn-primary text-xs disabled:opacity-60"
                onClick={handleReveal}
                disabled={revealBusy}
              >
                {revealBusy ? (
                  <>
                    <Loader2 size={12} className="inline animate-spin mr-1" />
                    {t("connections.revealing")}
                  </>
                ) : t("connections.token_show_btn")}
              </button>
            )
          )}
          {!routerConfig?.token_present && (
            <p className="text-err">{t("connections.token_not_yet")}</p>
          )}
        </div>

        <details className="group rounded-lg border border-border bg-bg-subtle" open>
            <summary className="flex cursor-pointer items-center justify-between px-4 py-3 text-sm font-medium hover:bg-overlay-soft">
              <span className="flex items-center gap-2">
                <ChevronDown size={14} className="transition-transform group-open:rotate-180" />
                <Terminal size={14} />
                {t("connections.npx_card_title")}
              </span>
              <button
                className="btn-ghost px-2 py-0.5 text-2xs"
                onClick={(e) => {
                  e.preventDefault();
                  if (npxConfig) void copy(npxConfig, "npx", t("connections.npx_card_title"));
                }}
                disabled={!npxConfig}
              >
                {copied === "npx" ? <span><Check size={12} aria-hidden="true" /> {t("common.copied")}</span> : <><Copy size={12} />{t("common.copy")}</>}
              </button>
            </summary>
            <div className="border-t border-border p-4">
              <pre className="max-h-48 overflow-auto rounded bg-bg p-3 text-2xs leading-relaxed">
                {npxConfig ?? t("connections.waiting_settings")}
              </pre>
            </div>
          </details>
        </div>
        )}
      </Section>

      {routerConfig && (
        <details className="group rounded-lg border border-border bg-bg-subtle">
          <summary className="flex cursor-pointer items-center justify-between px-4 py-3 text-sm font-medium hover:bg-overlay-soft">
            <span className="flex items-center gap-2">
              <ChevronDown size={14} className="transition-transform group-open:rotate-180" />
              {t("connections.advanced_title")}
            </span>
            <span className="text-xs text-fg-muted">{t("connections.advanced_hint")}</span>
          </summary>
          <div className="space-y-3 border-t border-border p-4 text-sm">
            <div className="grid grid-cols-1 gap-2 text-xs md:grid-cols-2">
              <div>
                <span className="text-fg-muted">{t("connections.endpoint_label")}</span>
                <code className="text-fg break-all">{routerConfig.endpoint}</code>
              </div>
              <div>
                <span className="text-fg-muted">{t("connections.token_label")}</span>
                <code className="text-fg break-all">
                  {revealedToken ?? routerConfig.token ?? t("settings.token_not_generated")}
                </code>
                {revealedToken && (
                  <button
                    className="btn-ghost ml-2 px-2 py-0.5 text-2xs"
                    onClick={() => copy(revealedToken, "token", "token")}
                  >
                    {copied === "token" ? (
                      <span aria-live="polite">{t("common.copied")}</span>
                    ) : (
                      <>
                        <Copy size={12} />
                        {t("common.copy")}
                      </>
                    )}
                  </button>
                )}
              </div>
              <div>
                <span className="text-fg-muted">{t("connections.total_requests")}</span>
                <span className="text-fg">{proxy?.total_requests ?? 0}</span>
              </div>
              <div>
                <span className="text-fg-muted">{t("connections.last_request")}</span>
                <span className="text-fg">
                  {proxy?.last_request_at
                    ? new Date(proxy.last_request_at).toLocaleTimeString()
                    : "—"}
                </span>
              </div>
            </div>

            <div>
              <div className="mb-1 flex items-center justify-between text-2xs text-fg-muted">
                <span className="flex items-center gap-1">
                  <Plug size={12} />
                  {t("connections.http_card_title")}
                </span>
                <button
                  className="btn-ghost px-2 py-0.5 text-2xs"
                  onClick={() => {
                    httpConfig && void copy(httpConfig, "http", t("connections.http_card_title"));
                  }}
                >
                  {copied === "http" ? (
                    <span><Check size={12} aria-hidden="true" /> {t("common.copied")}</span>
                  ) : (
                    <>
                      <Copy size={12} />
                      {t("common.copy")}
                    </>
                  )}
                </button>
              </div>
              <pre className="max-h-40 overflow-auto rounded bg-bg p-3 text-2xs leading-relaxed">
                {httpConfig}
              </pre>
              {routerConfig.instructions && (
                <div className="mt-2 text-2xs text-fg-muted">{routerConfig.instructions}</div>
              )}
            </div>
          </div>
        </details>
      )}

      {info?.data_dir && (
        <div className="text-xs text-fg-muted">
          {t("connections.data_dir_label")} <code className="text-fg">{info.data_dir}</code>
        </div>
      )}
    </div>
  );
}
