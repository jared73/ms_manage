import { useEffect, useMemo, useState } from "react";
import { Copy, RefreshCw, Terminal, ChevronDown, Plug } from "lucide-react";
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

  const token = settings?.mcp_token ?? "";

  // npx 形式的客户端配置（stdio ↔ HTTP 转发器，推荐）
  const npxConfig = useMemo(() => {
    if (!settings) return null;
    const obj = {
      mcpServers: {
        "ms-manage": {
          command: "npx",
          args: ["-y", "@ms-manage/cli@latest", "connect"],
          env: {
            MS_MANAGE_URL: `http://127.0.0.1:${settings.proxy_http_port}/mcp`,
            ...(token ? { MS_MANAGE_TOKEN: token } : {}),
          },
        },
      },
    };
    return JSON.stringify(obj, null, 2);
  }, [settings, token]);

  // HTTP 直连配置（advanced 折叠区，给自写客户端用）
  const httpConfig = useMemo(() => {
    if (!routerConfig) return null;
    return JSON.stringify({ mcpServers: routerConfig.mcpServers }, null, 2);
  }, [routerConfig]);

  const copy = (text: string, key: string, label: string) => {
    if (!text) return;
    navigator.clipboard.writeText(text);
    setCopied(key);
    toast.success(`已复制 ${label}`, { duration: 1500 });
    setTimeout(() => setCopied(""), 1500);
  };

  void proxy;

  return (
    <div className="space-y-4 p-6">
      {/* 主卡片：npx 一行配置（推荐） */}
      <Section
        title="客户端接入"
        action={
          <div className="flex items-center gap-3">
            {/* 服务运行状态指示器 */}
            <span className="inline-flex items-center gap-1.5 text-xs text-fg-muted">
              <span className={`inline-block h-2 w-2 rounded-full ${routerConfig?.running ? "bg-ok" : "bg-err"}`} />
              {routerConfig?.running ? "服务运行中" : "服务未运行"}
            </span>
            <button
              className="btn-primary"
              onClick={() => npxConfig && copy(npxConfig, "npx", "mcpServers JSON")}
              disabled={!npxConfig}
              title="复制到剪贴板，粘到客户端的 mcpServers 里"
            >
              {copied === "npx" ? "✓ 已复制" : <><Copy size={14}/>复制配置</>}
            </button>
          </div>
        }
      >
        <div className="space-y-3 text-sm">
          <p className="text-fg-muted">
            把下面这行配置粘到任何 MCP 客户端（Claude Desktop / Cursor / Cline / Continue …）
            的 <code className="text-fg">mcpServers</code> 里就行。
            客户端会按需 <code className="text-fg">npx</code> 下载 <code className="text-fg">@ms-manage/cli</code>（约 100 行的 stdio↔HTTP 转发器），
            转发到本机 MS Manage 的 HTTP 端口。
          </p>
          <ul className="list-disc space-y-1 pl-5 text-xs text-fg-muted">
            <li>客户端不需要安装 MS Manage 主程序，只需要能跑 <code className="text-fg">npx</code>（Node.js 14+）</li>
            <li>MS Manage 必须保持运行（关 GUI = 关代理 = 客户端断连）</li>
            <li>跨平台一致（macOS / Linux / Windows 同一份配置）</li>
            <li>多人可共享同一台 MS Manage 实例</li>
          </ul>

          {/* 客户端前提提示 */}
          <div className="rounded-lg border border-border bg-bg-subtle px-4 py-3 text-xs text-fg-muted">
            <div className="mb-1.5 font-medium text-fg">客户端机器需要满足：</div>
            <ul className="list-disc space-y-1 pl-5">
              <li>
                <b className="text-fg">已安装 Node.js</b>（14 或更高）。没装的话去
                <code className="text-fg"> nodejs.org </code>下载；命令行跑 <code className="text-fg">node -v</code> 能显示版本即 OK。
                这是最常见的失败原因——机器上根本没有 Node。
              </li>
              <li>
                <b className="text-fg">首次需联网</b>下载 <code className="text-fg">@ms-manage/cli</code>。
                国内如果卡在下载，先给 npm 换国内源：
                <code className="text-fg">npm config set registry https://registry.npmmirror.com</code>
              </li>
              <li>
                <b className="text-fg">之前装过旧版（0.1.0）要清缓存</b>再连，否则还是旧版：
                <code className="text-fg">npx clear-npx-cache</code>，或把配置里的 <code className="text-fg">@latest</code> 改成 <code className="text-fg">@0.2.1</code> 强制指定。
              </li>
            </ul>
          </div>

          <details className="rounded-lg border border-border bg-bg-subtle" open>
            <summary className="flex cursor-pointer items-center justify-between px-4 py-3 text-sm font-medium hover:bg-overlay-soft">
              <span className="flex items-center gap-2">
                <ChevronDown size={14} className="transition-transform group-open:rotate-180" />
                <Terminal size={14}/>mcpServers JSON（推荐 · npx）
              </span>
              <button
                className="btn-ghost px-2 py-0.5 text-[11px]"
                onClick={(e) => { e.preventDefault(); npxConfig && copy(npxConfig, "npx", "mcpServers JSON"); }}
                disabled={!npxConfig}
              >
                {copied === "npx" ? "✓ 已复制" : <><Copy size={10}/>复制</>}
              </button>
            </summary>
            <div className="border-t border-border p-4">
              <pre className="max-h-48 overflow-auto rounded bg-bg p-3 text-[11px] leading-relaxed">
{npxConfig ?? "（等待 settings 加载）"}
              </pre>
            </div>
          </details>
        </div>
      </Section>

      {/* Advanced：HTTP 直连（自写客户端调试用） */}
      {routerConfig && (
        <details className="rounded-lg border border-border bg-bg-subtle">
          <summary className="flex cursor-pointer items-center justify-between px-4 py-3 text-sm font-medium hover:bg-overlay-soft">
            <span className="flex items-center gap-2">
              <ChevronDown size={14} className="transition-transform group-open:rotate-180" />
              Advanced · HTTP endpoint + 直连 JSON
            </span>
            <span className="text-xs text-fg-muted">给自写客户端 / 调试用</span>
          </summary>
          <div className="space-y-3 border-t border-border p-4 text-sm">
            <div className="grid grid-cols-1 gap-2 text-xs md:grid-cols-2">
              <div>
                <span className="text-fg-muted">HTTP endpoint：</span>
                <code className="text-fg break-all">{routerConfig.endpoint}</code>
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

            <div>
              <div className="mb-1 flex items-center justify-between text-[11px] text-fg-muted">
                <span className="flex items-center gap-1"><Plug size={11}/>HTTP mcpServers JSON（直连，不走 npx CLI）</span>
                <button
                  className="btn-ghost px-2 py-0.5 text-[11px]"
                  onClick={() => httpConfig && copy(httpConfig, "http", "HTTP mcpServers JSON")}
                >
                  {copied === "http" ? "✓ 已复制" : <><Copy size={10}/>复制</>}
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
          数据目录：<code className="text-fg">{info.data_dir}</code>
        </div>
      )}
    </div>
  );
}