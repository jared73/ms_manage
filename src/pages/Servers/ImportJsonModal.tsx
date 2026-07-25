import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { upsertServer } from "@/lib/ipc";
import type { McpServer } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import DOMPurify from "dompurify";

/**
 * 从标准 mcpServers JSON 批量导入。
 * 支持 {"mcpServers": {...}} 与裸 {...} 两种格式;transport 自动识别。
 */
export function ImportJsonModal({
  onClose,
  onImported,
}: {
  onClose: () => void;
  onImported: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [raw, setRaw] = useState("");
  const [parsed, setParsed] = useState<{ name: string; conf: any }[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  const tryParse = (text: string) => {
    setRaw(text);
    setError(null);
    if (!text.trim()) {
      setParsed(null);
      return;
    }
    try {
      const obj: unknown = JSON.parse(text);
      const objAny = obj as { mcpServers?: Record<string, ImportedConfig> } & Record<string, ImportedConfig>;
      const servers = (objAny.mcpServers ?? objAny) as Record<string, ImportedConfig>;
      const entries = Object.entries(servers).map(([name, conf]) => ({ name, conf }));
      // 过滤掉没法入库的条目(不是 stdio / http / sse)
      const valid = entries.filter(({ conf }) => conf.command || conf.url || conf.type);
      const skipped = entries.length - valid.length;
      setParsed(valid);
      if (skipped > 0) {
        setError(t("servers.import_json_skip_notice", { count: skipped }) as string);
      }
    } catch (e) {
      setError(t("servers.import_json_parse_error", { msg: describeError(e) }) as string);
      setParsed(null);
    }
  };

  const doImport = async () => {
    if (!parsed || parsed.length === 0) return;
    setImporting(true);
    let count = 0;
    try {
      for (const { name, conf } of parsed) {
        await upsertServer(buildImportedServer(name, conf));
        count++;
      }
      toast.success(t("servers.toast_imported", { count }) as string);
      await onImported();
      onClose();
    } catch (e) {
      toast.error(t("servers.toast_import_failed", { msg: describeError(e) }) as string);
    } finally {
      setImporting(false);
    }
  };

  // 一键填个示例
  const fillSample = () =>
    tryParse(
      JSON.stringify(
        {
          mcpServers: {
            filesystem: {
              command: "npx",
              args: ["-y", "@modelcontextprotocol/server-filesystem", "C:\\Users\\admin\\Documents"],
            },
            memory: { command: "uvx", args: ["mcp-server-memory"] },
            puppeteer: {
              command: "npx",
              args: ["-y", "@modelcontextprotocol/server-puppeteer"],
              env: { ALLOW_DANGEROUS: "true" },
            },
          },
        },
        null,
        2,
      ),
    );

  return (
    <Modal
      open
      title={t("servers.import_json_dialog_title")}
      onClose={onClose}
      size="lg"
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={importing}>
            {t("servers.import_json_cancel")}
          </button>
          <button
            className="btn-primary"
            onClick={doImport}
            disabled={!parsed || parsed.length === 0 || importing}
          >
            {importing ? t("servers.import_json_busy") : t("servers.import_json_btn", { count: parsed?.length ?? 0 })}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <div>
          <div className="mb-1 flex items-center justify-between">
            <label className="text-sm text-fg-muted">{t("servers.import_json_dialog_hint")}</label>
            <button className="btn-ghost text-xs" onClick={fillSample}>
              {t("servers.import_json_sample_btn")}
            </button>
          </div>
          <textarea
            className="input h-40 w-full font-mono text-xs"
            placeholder={`{\n  "mcpServers": {\n    "name": { "command": "...", "args": [...] }\n  }\n}`}
            value={raw}
            onChange={(e) => tryParse(e.target.value)}
            spellCheck={false}
          />
          <p className="mt-1 text-xs text-fg-muted"
            dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(t("servers.import_json_description") as string) }}
          />
        </div>

        {error && (
          <div className="rounded border border-err/30 bg-err/10 px-3 py-2 text-xs text-err">
            {error}
          </div>
        )}

        {parsed && parsed.length > 0 && (
          <div className="space-y-2">
            <div className="text-sm text-fg-muted">{t("servers.import_json_preview", { count: parsed.length })}</div>
            <div className="max-h-48 space-y-1 overflow-auto rounded border border-border bg-bg-subtle p-2">
              {parsed.map(({ name, conf }) => {
                const tp = conf.type === "http" ? "http" : conf.type === "sse" ? "sse" : "stdio";
                const summary =
                  tp === "stdio"
                    ? `${conf.command ?? ""} ${(conf.args ?? []).join(" ")}`.trim()
                    : `${conf.url ?? ""}`;
                return (
                  <div
                    key={name}
                    className="flex items-center gap-2 rounded px-2 py-1 hover:bg-overlay-soft"
                  >
                    <span className="badge-mute">{tp}</span>
                    <span className="font-medium">{name}</span>
                    <code className="flex-1 truncate text-xs text-fg-muted">{summary}</code>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}

/**
 * 把标准 mcpServers 的一项原始 JSON 转成入库的 McpServer。
 * transport 推断:c.type 优先,否则按是否带 url 区分;都不带则 stdio。
 *
 * `c` 用最小本地类型描述导入结构 — 避免从 `any` 再开始 soaks,保留
 * 导入容错性(缺字段时给空数组/默认值),用窄字段取代 `any`。
 */
interface ImportedConfig {
  type?: "stdio" | "http" | "sse" | string;
  description?: string;
  command?: string;
  args?: unknown[];
  env?: Record<string, string>;
  cwd?: string;
  url?: string;
  headers?: Record<string, string>;
}

function buildImportedServer(name: string, c: ImportedConfig): McpServer {
  const transport = c.type === "http" || c.type === "sse" ? c.type : "stdio";
  const now = new Date().toISOString();
  const args = (Array.isArray(c.args) ? c.args : []).filter((x): x is string => typeof x === "string");
  return {
    id: crypto.randomUUID(),
    name,
    description: c.description,
    transport: transport as McpServer["transport"],
    command: c.command ?? "",
    args,
    env: c.env ?? {},
    cwd: c.cwd,
    url: c.url,
    headers: c.headers,
    enabled: true,
    auto_start: false,
    created_at: now,
    updated_at: now,
  };
}
