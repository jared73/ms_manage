import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { X, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { fetchServerTools, setToolEnabled } from "@/lib/ipc";
import type { McpServer, McpTool } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

const TOOLS_CACHE_TTL_MS = 30_000;

type Tab = "general" | "tools";

/**
 * Server 详情侧栏:General(只读字段)+ Tools(开关 + 刷新)。
 * Tools 列表有 30s 内存缓存,切走再切回不会重复拉。
 */
export function ServerDetailDrawer({
  server,
  onClose,
  onChanged,
}: {
  server: McpServer;
  onClose: () => void;
  onChanged: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("general");
  const [tools, setTools] = useState<McpTool[] | null>(null);
  const [loadingTools, setLoadingTools] = useState(false);
  const [toolsErr, setToolsErr] = useState<string | null>(null);
  const [toolsFetchedAt, setToolsFetchedAt] = useState<number>(0);
  // 本地 tool 启用态覆盖:点 toggle 立刻翻,后台再保存。
  // 这样用户感觉是即时的;后端保存失败时回滚 + 弹错误 toast。
  const [localDisabled, setLocalDisabled] = useState<Set<string>>(
    () => new Set(server.disabled_tools ?? []),
  );
  // 当父组件更新 server(外部改了 disabled_tools)时同步本地态
  useEffect(() => {
    setLocalDisabled(new Set(server.disabled_tools ?? []));
  }, [server.disabled_tools]);

  const toggleTool = async (toolName: string, enabled: boolean) => {
    const prev = new Set(localDisabled);
    const next = new Set(localDisabled);
    if (enabled) next.delete(toolName);
    else next.add(toolName);
    setLocalDisabled(next); // 立刻翻 —— 用户感觉是即时的
    try {
      await setToolEnabled(server.id, toolName, enabled);
      await onChanged();
      toast.success(enabled ? t("servers.toast_tool_enabled", { name: toolName }) as string : t("servers.toast_tool_disabled", { name: toolName }) as string, { duration: 1200 });
    } catch (e) {
      setLocalDisabled(prev); // 回滚
      toast.error(t("servers.toast_refresh_failed", { msg: describeError(e) }) as string);
    }
  };

  const loadTools = async (force = false) => {
    if (!force && tools !== null && Date.now() - toolsFetchedAt < TOOLS_CACHE_TTL_MS) {
      return; // 30s 内复用缓存
    }
    setLoadingTools(true);
    setToolsErr(null);
    try {
      const r = await fetchServerTools(server.id);
      setTools(r);
      setToolsFetchedAt(Date.now());
    } catch (e) {
      setToolsErr(describeError(e));
      setTools([]);
    } finally {
      setLoadingTools(false);
    }
  };

  useEffect(() => {
    if (tab === "tools" && (tools === null || Date.now() - toolsFetchedAt > TOOLS_CACHE_TTL_MS)) {
      void loadTools(false);
    }
  }, [tab]);

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-overlay backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="w-full max-w-2xl rounded-lg border border-border bg-bg-elev shadow-2xl max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div>
            <h2 className="text-base font-semibold">{server.name}</h2>
            {server.description && <div className="text-xs text-fg-muted">{server.description}</div>}
          </div>
          <button className="btn-ghost p-1" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        {/* tabs */}
        <div className="flex border-b border-border px-2">
          <button
            className={`px-3 py-2 text-sm ${tab === "general" ? "border-b-2 border-accent text-fg" : "text-fg-muted hover:text-fg"}`}
            onClick={() => setTab("general")}
          >
            {t("servers.detail_general_tab")}
          </button>
          <button
            className={`px-3 py-2 text-sm ${tab === "tools" ? "border-b-2 border-accent text-fg" : "text-fg-muted hover:text-fg"}`}
            onClick={() => setTab("tools")}
          >
            {t("servers.detail_tools_tab")}
            {localDisabled.size > 0 && (
              <span className="ml-1 badge-mute text-[10px]">{localDisabled.size} {t("common.disabled")}</span>
            )}
          </button>
        </div>

        <div className="flex-1 overflow-auto p-4">
          {tab === "general" && (
            <div className="space-y-3 text-sm">
              <DetailRow k={t("servers.detail_transport")} v={<span className="badge-mute">{server.transport}</span>} />
              {server.transport === "stdio" ? (
                <>
                  <DetailRow k={t("servers.detail_command")} v={<code className="text-xs">{server.command ?? "—"}</code>} />
                  <DetailRow k={t("servers.detail_args")} v={
                      <code className="text-xs break-all">
                        {(server.args ?? []).join(" ") || "—"}
                      </code>
                    }
                  />
                  {server.cwd && (
                    <DetailRow k={t("servers.detail_cwd")} v={<code className="text-xs">{server.cwd}</code>} />
                  )}
                  {server.env && Object.keys(server.env).length > 0 && (
                    <div className="rounded-md border border-border bg-bg-subtle p-3">
                      <div className="mb-1 text-xs text-fg-muted">{t("servers.detail_env")}</div>
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
                  <DetailRow k={t("servers.detail_url")} v={<code className="text-xs break-all">{server.url ?? "—"}</code>} />
                  {server.headers && Object.keys(server.headers).length > 0 && (
                    <div className="rounded-md border border-border bg-bg-subtle p-3">
                      <div className="mb-1 text-xs text-fg-muted">{t("servers.detail_headers")}</div>
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
              <DetailRow k={t("servers.detail_autostart")} v={server.auto_start ? t("servers.detail_autostart_on") : t("servers.detail_autostart_off")} />
              <DetailRow k={t("servers.detail_status")}
                v={
                  <span className={server.enabled ? "text-ok" : "text-fg-muted"}>
                    {server.enabled ? t("servers.detail_status_enabled") : t("servers.detail_status_disabled")}
                  </span>
                }
              />
              <DetailRow k={t("servers.detail_created_at")}
                v={
                  <span className="text-xs text-fg-muted">
                    {new Date(server.created_at).toLocaleString()}
                  </span>
                }
              />
            </div>
          )}

          {tab === "tools" && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <div className="text-xs text-fg-muted">
                  {loadingTools ? t("servers.detail_tools_loading") as string : tools ? t("servers.detail_tools_count", { count: tools.length }) as string : "—"}
                </div>
                <button
                  className="btn-ghost text-xs"
                  onClick={() => loadTools(true)}
                  disabled={loadingTools}
                >
                  <RefreshCw size={12} className={loadingTools ? "animate-spin" : ""} />
                  {t("common.refresh")}
                </button>
              </div>

              {toolsErr && (
                <div className="rounded border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">
                  {toolsErr}
                </div>
              )}

              {tools && tools.length === 0 && !loadingTools && (
                <div className="text-xs text-fg-muted">{t("servers.detail_tools_empty")}</div>
              )}

              <div className="space-y-1">
                {tools?.map((tool) => {
                  const enabled = !localDisabled.has(tool.name);
                  return (
                    <div
                      key={tool.name}
                      className={`flex items-start gap-2 rounded-md border px-3 py-2 ${enabled ? "border-border bg-bg-subtle" : "border-border bg-bg-subtle/40 opacity-60"}`}
                    >
                      <button
                        onClick={() => toggleTool(tool.name, !enabled)}
                        role="switch"
                        aria-checked={enabled}
                        aria-label={`${tool.name} ${enabled ? t("common.enabled") : t("common.disabled")}`}
                        className={`relative mt-1 h-5 w-9 shrink-0 rounded-full transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${enabled ? "bg-accent" : "bg-overlay-soft"}`}
                        title={enabled ? t("common.enabled") : t("common.disabled")}
                      >
                        <span
                          className={`absolute top-0.5 h-4 w-4 rounded-full bg-bg-subtle shadow-sm transition-all ${enabled ? "left-4" : "left-0.5"}`}
                        />
                      </button>
                      <div className="min-w-0 flex-1">
                        <div className="text-sm font-medium">{tool.name}</div>
                        {tool.description && (
                          <div className="line-clamp-3 text-xs text-fg-muted">{tool.description}</div>
                        )}
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
