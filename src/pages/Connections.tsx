import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Terminal, Copy, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { getSettings, appInfo } from "@/lib/ipc";
import type { AppSettings } from "@/lib/ipc";

export function Connections() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ data_dir: string } | null>(null);

  useEffect(() => {
    void (async () => {
      setSettings(await getSettings());
      setInfo(await appInfo());
    })();
  }, []);

  const httpEndpoint = settings ? `http://127.0.0.1:${settings.proxy_http_port}/mcp` : "";
  const token = settings?.mcp_token ?? "";

  // 复制 + toast 提示，label 是给用户看的具体复制了啥
  const copy = (text: string, label: string) => {
    if (!text) return;
    navigator.clipboard.writeText(text);
    toast.success(`已复制 ${label}`, { duration: 1500 });
  };

  // 一键复制 mcpServers JSON 片段（给 HTTP-capable 客户端粘贴用）
  const copyMcpServersSnippet = () => {
    if (!settings) return;
    const snippet = JSON.stringify({
      mcpServers: {
        "ms-manage": {
          type: "http",
          url: httpEndpoint,
          headers: { Authorization: `Bearer ${token}` },
        },
      },
    }, null, 2);
    copy(snippet, "mcpServers JSON");
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title="客户端接入"
        action={<button className="btn-ghost" onClick={() => location.reload()}><RefreshCw size={14}/>刷新</button>}
      >
        {/* HTTP 端点 + token —— 任何支持 mcpServers 的客户端都能用 */}
        <div className="card space-y-3">
          <div className="flex items-center gap-2">
            <Terminal size={16} className="text-accent" />
            <h3 className="font-semibold">HTTP 代理</h3>
          </div>
          <p className="text-xs text-fg-muted">
            GUI 启动后会自动 listen <code className="text-fg">{httpEndpoint}</code>。
            任何支持 mcpServers + http transport 的客户端（Claude Code / Cursor / Cline / Windsurf / VS Code / Qoder 等）都能连。
          </p>

          <div className="rounded-md border border-white/10 bg-bg-subtle p-3 space-y-2">
            <FieldRow label="HTTP 端点" value={httpEndpoint} onCopy={() => copy(httpEndpoint, "HTTP 端点")} />
            <FieldRow label="Auth token" value={token} onCopy={() => copy(token, "Auth token")} />
            <FieldRow
              label="mcpServers 片段"
              value={`<点右侧复制按钮，粘贴到任意客户端配置>`}
              onCopy={copyMcpServersSnippet}
              buttonLabel="复制 JSON"
            />
          </div>
        </div>

        {/* 各客户端一行命令接入（CLI 模式） */}
        <div className="card space-y-3">
          <h3 className="font-semibold flex items-center gap-2"><Terminal size={16}/>一行命令接入</h3>
          <p className="text-xs text-fg-muted">
            通过 stdio 模式把 MS Manage 作为 MCP server 直接 spawn 进客户端。需要在 PATH 里能找到 <code className="text-fg">ms_manage.exe</code>。
          </p>
          <div className="space-y-2">
            <CmdRow label="启动代理"   cmd={`ms_manage proxy http --port ${settings?.proxy_http_port ?? 39231}`} onCopy={(c) => copy(c, "启动代理命令")} />
            <CmdRow label="Claude Code" cmd={`ms_manage connect claude-code`} onCopy={(c) => copy(c, "Claude Code 命令")} />
            <CmdRow label="Cursor"      cmd={`ms_manage connect cursor`} onCopy={(c) => copy(c, "Cursor 命令")} />
            <CmdRow label="Windsurf"    cmd={`ms_manage connect windsurf`} onCopy={(c) => copy(c, "Windsurf 命令")} />
            <CmdRow label="VS Code"     cmd={`ms_manage connect vscode`} onCopy={(c) => copy(c, "VS Code 命令")} />
            <CmdRow label="Qoder"       cmd={`ms_manage connect qoder`} onCopy={(c) => copy(c, "Qoder 命令")} />
          </div>
        </div>

        {info?.data_dir && (
          <div className="text-xs text-fg-muted">
            数据目录：<code className="text-fg">{info.data_dir}</code>
          </div>
        )}
      </Section>
    </div>
  );
}

function FieldRow({ label, value, onCopy, buttonLabel }: { label: string; value: string; onCopy: () => void; buttonLabel?: string }) {
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="text-fg-muted shrink-0 w-28">{label}</span>
      <code className="flex-1 truncate">{value}</code>
      <button className="btn-ghost p-1.5" onClick={onCopy} title="复制">
        <Copy size={12}/>
        {buttonLabel && <span className="ml-1 text-[11px]">{buttonLabel}</span>}
      </button>
    </div>
  );
}

function CmdRow({ label, cmd, onCopy }: { label: string; cmd: string; onCopy: (c: string) => void }) {
  return (
    <div className="flex items-center gap-2 rounded-md bg-bg-subtle px-3 py-2">
      <div className="w-24 text-xs text-fg-muted">{label}</div>
      <code className="flex-1 truncate text-sm">{cmd}</code>
      <button className="btn-ghost p-1.5" onClick={() => onCopy(cmd)} title="复制">
        <Copy size={14}/>
      </button>
    </div>
  );
}