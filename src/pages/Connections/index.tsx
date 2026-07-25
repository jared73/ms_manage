/**
 * 客户端接入页:展示 npx 一行配置 / HTTP 直连(Advanced)。
 *
 * 设计:
 * - 中英文文案全部走 t(),避免硬编码
 * - 数据获取 + 计算复制目标在 useConnections hook 中
 */
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, ChevronDown, Terminal, Plug } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { useConnections } from "./useConnections";

export function Connections() {
  const { t } = useTranslation();
  const { routerConfig, proxy, npxConfig, httpConfig, info, revealedToken, revealToken, clearRevealedToken } = useConnections();
  const [copied, setCopied] = useState<string>("");

  const copy = (text: string | null, key: string, label: string) => {
    if (!text) return;
    navigator.clipboard.writeText(text).catch(() => {});
    setCopied(key);
    toast.success(t("connections.copied", { label }), { duration: 1500 });
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
              <span
                className={`inline-block h-2 w-2 rounded-full ${
                  routerConfig?.running ? "bg-ok" : "bg-err"
                }`}
              />
              {routerConfig?.running ? t("connections.running") : t("connections.stopped")}
            </span>
            <button
              className="btn-primary"
              onClick={() => npxConfig && copy(npxConfig, "npx", t("connections.copy_config_npx"))}
              disabled={!npxConfig}
              title={t("connections.copy_tooltip") as string}
            >
              {copied === "npx" ? (
                <span>✓ {t("common.copied")}</span>
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
              <button className="btn-primary text-xs" onClick={() => void revealToken()}>
                {t("connections.token_show_btn")}
              </button>
            )
          )}
          {!routerConfig?.token_present && (
            <p className="text-err">{t("connections.token_not_yet")}</p>
          )}
        </div>

        <details className="rounded-lg border border-border bg-bg-subtle" open>
            <summary className="flex cursor-pointer items-center justify-between px-4 py-3 text-sm font-medium hover:bg-overlay-soft">
              <span className="flex items-center gap-2">
                <ChevronDown size={14} className="transition-transform group-open:rotate-180" />
                <Terminal size={14} />
                {t("connections.npx_card_title")}
              </span>
              <button
                className="btn-ghost px-2 py-0.5 text-[11px]"
                onClick={(e) => {
                  e.preventDefault();
                  if (npxConfig) copy(npxConfig, "npx", t("connections.npx_card_title"));
                }}
                disabled={!npxConfig}
              >
                {copied === "npx" ? <span>✓ {t("common.copied")}</span> : <><Copy size={10} />{t("common.copy")}</>}
              </button>
            </summary>
            <div className="border-t border-border p-4">
              <pre className="max-h-48 overflow-auto rounded bg-bg p-3 text-[11px] leading-relaxed">
                {npxConfig ?? t("connections.waiting_settings")}
              </pre>
            </div>
          </details>
        </div>
      </Section>

      {routerConfig && (
        <details className="rounded-lg border border-border bg-bg-subtle">
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
                  {revealedToken ?? routerConfig.token ?? "(未生成)"}
                </code>
                {revealedToken && (
                  <button
                    className="btn-ghost ml-2 px-2 py-0.5 text-[11px]"
                    onClick={() => copy(revealedToken, "token", "token")}
                  >
                    {copied === "token" ? "✓" : <Copy size={10} />}
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
              <div className="mb-1 flex items-center justify-between text-[11px] text-fg-muted">
                <span className="flex items-center gap-1">
                  <Plug size={11} />
                  {t("connections.http_card_title")}
                </span>
                <button
                  className="btn-ghost px-2 py-0.5 text-[11px]"
                  onClick={() =>
                    httpConfig && copy(httpConfig, "http", t("connections.http_card_title"))
                  }
                >
                  {copied === "http" ? (
                    <span>✓ {t("common.copied")}</span>
                  ) : (
                    <>
                      <Copy size={10} />
                      {t("common.copy")}
                    </>
                  )}
                </button>
              </div>
              <pre className="max-h-40 overflow-auto rounded bg-bg p-3 text-[11px] leading-relaxed">
                {httpConfig}
              </pre>
              {routerConfig.instructions && (
                <div className="mt-2 text-[11px] text-fg-muted">{routerConfig.instructions}</div>
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
