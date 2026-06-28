import { useEffect, useState } from "react";
import { Copy, RefreshCw, Plug, FileCode } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { getSettings, getRouterConfig, appInfo, getProxyStatus } from "@/lib/ipc";
import type { AppSettings } from "@/lib/ipc";

type RouterConfig = Awaited<ReturnType<typeof getRouterConfig>>;
type ProxyStatus = Awaited<ReturnType<typeof getProxyStatus>>;

export function Connections() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ data_dir: string } | null>(null);
  const [routerConfig, setRouterConfig] = useState<RouterConfig | null>(null);
  const [proxy, setProxy] = useState<ProxyStatus | null>(null);
  const [copied, setCopied] = useState<string>("");

  const load = async () => {
    setSettings(await getSettings());
    setInfo(await appInfo());
    setRouterConfig(await getRouterConfig().catch(() => null));
    setProxy(await getProxyStatus().catch(() => null));
  };

  useEffect(() => { void load(); }, []);

  const httpEndpoint = settings ? `http://127.0.0.1:${settings.proxy_http_port}/mcp` : "";
  const token = settings?.mcp_token ?? "";

  const copy = (text: string, key: string, label: string) => {
    if (!text) return;
    navigator.clipboard.writeText(text);
    setCopied(key);
    toast.success(`已复制 ${label}`, { duration: 1500 });
    setTimeout(() => setCopied(""), 1500);
  };

  // 给"已处理请求/最近请求"等数据用的简易访问
  void proxy;

  return (
    <div className="space-y-4 p-6">
      {/* MCP Router 服务卡片（学 mcp-router）—— 之前在 Dashboard，现在统一搬到这里 */}
      <Section
        title="MCP Router 服务"
        action={
          <div className="flex gap-2">
            <button className="btn-ghost" onClick={load}><RefreshCw size={14}/>刷新</button>
            <button className="btn-primary" onClick={() => (window.location.href = "/")}>回到概览 →</button>
          </div>
        }
      >
        <div className="card space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="grid h-10 w-10 place-items-center rounded-lg bg-ok/15 text-ok">
                <Plug size={20} />
              </div>
              <div>
                <div className="font-semibold">MCP Router 服务</div>
                <div className="text-xs text-fg-muted">
                  {routerConfig?.running
                    ? <>运行中 · endpoint <code className="text-fg">{routerConfig.endpoint}</code></>
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
                    onClick={() => copy(routerConfig.token, "token", "token")}
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
                    onClick={() => copy(JSON.stringify({ mcpServers: routerConfig.mcpServers }, null, 2), "json", "mcpServers JSON")}
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
        </div>
      </Section>

      {info?.data_dir && (
        <div className="text-xs text-fg-muted">
          数据目录：<code className="text-fg">{info.data_dir}</code>
        </div>
      )}
    </div>
  );
}