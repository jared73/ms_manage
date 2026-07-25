import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { upsertServer } from "@/lib/ipc";
import type { McpServer, Project, Transport } from "@/lib/ipc";
import { Field } from "./types";
import { describeError } from "@/lib/errors";

/** 新建 / 编辑 server 的弹窗:含预置模板(fieldsystem/fetch/github/memory)。 */
export function ServerFormModal({
  initial,
  projects,
  onClose,
  onSaved,
}: {
  initial: McpServer;
  projects: Project[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [s, setS] = useState<McpServer>(initial);
  const [argsText, setArgsText] = useState((initial.args ?? []).join(" "));
  const [envText, setEnvText] = useState(
    Object.entries(initial.env ?? {})
      .map(([k, v]) => `${k}=${v}`)
      .join("\n"),
  );
  // 表单校验:把错误信息收集到 errors Map,Save 时校验;
  // 用户在 Save 前看不到一堆红框,只在字段下方显示第一条错误
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  const validate = (): Record<string, string> => {
    const e: Record<string, string> = {};
    const name = s.name.trim();
    if (!name) e.name = t("servers.validation_name_required");
    else if (!/^[A-Za-z0-9._\-\s]{1,64}$/.test(name)) {
      // 限制字符集:对应 mcp server id 用法,避免奇怪的 unicode / shell 注入字符
      e.name = t("servers.validation_name_charset");
    }
    if (s.transport === "stdio") {
      const cmd = (s.command ?? "").trim();
      if (!cmd) e.command = t("servers.validation_command_required");
      else if (/^[A-Za-z0-9._\-/:]+$/.test(cmd) === false) {
        // 允许 PATH 解析,所以接受 /: 和 .
        e.command = t("servers.validation_command_charset");
      }
    } else {
      const url = (s.url ?? "").trim();
      if (!url) e.url = t("servers.validation_url_required");
      else if (!/^https?:\/\//i.test(url)) {
        e.url = t("servers.validation_url_scheme");
      }
    }
    return e;
  };

  const submit = async () => {
    const v = validate();
    setErrors(v);
    if (Object.keys(v).length > 0) {
      toast.error(t("servers.toast_validation_failed") as string);
      return;
    }
    const args = argsText.trim() ? argsText.split(/\s+/) : [];
    const env: Record<string, string> = {};
    envText.split(/\n/).forEach((line) => {
      const m = line.match(/^\s*([A-Z_][A-Z0-9_]*)\s*=\s*(.*)$/i);
      if (m) env[m[1]] = m[2];
    });
    setBusy(true);
    try {
      await upsertServer({ ...s, args, env, updated_at: new Date().toISOString() });
      await onSaved();
      toast.success(t("servers.toast_saved", { name: s.name }) as string, { duration: 1500 });
    } catch (e) {
      toast.error(t("servers.toast_save_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusy(false);
    }
  };

  const applyPreset = (preset: "filesystem" | "fetch" | "github" | "memory") => {
    const presets: Record<string, Partial<McpServer>> = {
      filesystem: {
        name: "filesystem",
        description: t("servers.preset_filesystem"),
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", "."],
      },
      fetch: {
        name: "fetch",
        description: t("servers.preset_fetch"),
        transport: "stdio",
        command: "uvx",
        args: ["mcp-server-fetch"],
      },
      github: {
        name: "github",
        description: t("servers.preset_github"),
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-github"],
        env: { GITHUB_PERSONAL_ACCESS_TOKEN: "" },
      },
      memory: {
        name: "memory",
        description: t("servers.preset_memory"),
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-memory"],
      },
    };
    setS({ ...s, ...presets[preset] });
    setArgsText((presets[preset].args ?? []).join(" "));
    setEnvText(
      Object.entries(presets[preset].env ?? {})
        .map(([k, v]) => `${k}=${v}`)
        .join("\n"),
    );
  };

  return (
    <Modal
      open
      size="lg"
      title={initial.name ? t("servers.edit_title") : t("servers.add_title")}
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </button>
          <button className="btn-primary" onClick={submit} disabled={busy}>
            {busy ? <Loader2 size={14} className="inline animate-spin mr-1" /> : null}
            {t("common.save")}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        {!initial.name && (
          <div>
            <div className="mb-1 text-xs text-fg-muted">{t("servers.presets_hint")}</div>
            <div className="flex flex-wrap gap-1">
              {(["filesystem", "fetch", "github", "memory"] as const).map((p) => (
                <button key={p} className="btn-ghost text-xs" onClick={() => applyPreset(p)}>
                  {t(`servers.preset_${p}`)}
                </button>
              ))}
            </div>
          </div>
        )}
        <Field label={t("common.name")} error={errors.name}>
          <input
            className="input"
            value={s.name}
            onChange={(e) => setS({ ...s, name: e.target.value })}
            aria-invalid={errors.name ? "true" : undefined}
          />
        </Field>
        <Field label={t("common.description")}>
          <input
            className="input"
            value={s.description ?? ""}
            onChange={(e) => setS({ ...s, description: e.target.value })}
          />
        </Field>
        <Field label={t("projects.title")}>
          <select
            className="input max-w-md"
            value={s.project_id ?? ""}
            onChange={(e) => setS({ ...s, project_id: e.target.value || null })}
          >
            <option value="">{t("servers.no_group_option")}</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t("common.transport")}>
          <div className="flex gap-2">
            {(["stdio", "sse", "http"] as Transport[]).map((tr) => (
              <button
                key={tr}
                className={s.transport === tr ? "btn-primary" : "btn-ghost"}
                onClick={() => setS({ ...s, transport: tr })}
              >
                {tr}
              </button>
            ))}
          </div>
        </Field>

        {s.transport === "stdio" && (
          <>
            <Field label={t("common.command")} error={errors.command}>
              <input
                className="input font-mono"
                placeholder={t("servers.server_form_command_placeholder")}
                value={s.command ?? ""}
                onChange={(e) => setS({ ...s, command: e.target.value })}
                aria-invalid={errors.command ? "true" : undefined}
              />
            </Field>
            <Field label={t("common.args")}>
              <input
                className="input font-mono"
                placeholder={t("servers.server_form_args_placeholder")}
                value={argsText}
                onChange={(e) => setArgsText(e.target.value)}
              />
            </Field>
            <Field label={t("common.env")}>
              <textarea
                className="input min-h-[80px] font-mono"
                placeholder={"KEY=value\nANOTHER=foo"}
                value={envText}
                onChange={(e) => setEnvText(e.target.value)}
              />
            </Field>
            <Field label={t("common.cwd")}>
              <input
                className="input font-mono"
                placeholder={t("servers.server_form_cwd_placeholder")}
                value={s.cwd ?? ""}
                onChange={(e) => setS({ ...s, cwd: e.target.value || undefined })}
              />
            </Field>
          </>
        )}

        {(s.transport === "sse" || s.transport === "http") && (
          <>
            <Field label={t("common.url")} error={errors.url}>
              <input
                className="input font-mono"
                placeholder={t("servers.server_form_url_placeholder")}
                value={s.url ?? ""}
                onChange={(e) => setS({ ...s, url: e.target.value })}
                aria-invalid={errors.url ? "true" : undefined}
              />
            </Field>
            <Field label={t("common.headers")}>
              <textarea
                className="input min-h-[60px] font-mono"
                placeholder={t("servers.server_form_headers_placeholder")}
                value={Object.entries(s.headers ?? {})
                  .map(([k, v]) => `${k}=${v}`)
                  .join("\n")}
                onChange={(e) => {
                  const h: Record<string, string> = {};
                  e.target.value.split(/\n/).forEach((line) => {
                    const m = line.match(/^\s*([\w-]+)\s*[:=]\s*(.*)$/);
                    if (m) h[m[1]] = m[2];
                  });
                  setS({ ...s, headers: h });
                }}
              />
            </Field>
          </>
        )}

        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={s.enabled}
            onChange={(e) => setS({ ...s, enabled: e.target.checked })}
          />
          {t("common.enabled_col")}
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={s.auto_start ?? false}
            onChange={(e) => setS({ ...s, auto_start: e.target.checked })}
          />
          {t("servers.autostart_checkbox")}
        </label>
      </div>
    </Modal>
  );
}

/** 给"新建"按钮用的空白 server。 */
export function blankServer(): McpServer {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    name: "",
    transport: "stdio",
    command: "",
    args: [],
    env: {},
    enabled: true,
    created_at: now,
    updated_at: now,
  };
}
