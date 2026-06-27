import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Save, FolderOpen, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { getSettings, updateSettings, appInfo, openDataDir, regenerateToken } from "@/lib/ipc";
import type { AppSettings } from "@/lib/ipc";
import { useApp } from "@/stores/app";

export function Settings() {
  const { t, i18n } = useTranslation();
  const setLang = useApp((s) => s.setLang);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ version: string; data_dir: string } | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    void (async () => {
      setSettings(await getSettings());
      setInfo(await appInfo());
    })();
  }, []);

  const save = async () => {
    if (!settings) return;
    await updateSettings(settings);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
    void i18n.changeLanguage(settings.language);
    setLang(settings.language);
  };

  const regen = async () => {
    try {
      const newToken = await regenerateToken();
      setSettings({ ...settings!, mcp_token: newToken });
      toast.success("Token 已重置");
    } catch (e) {
      toast.error(`重置失败: ${e}`);
    }
  };

  if (!settings) return null;

  return (
    <div className="space-y-4 p-6">
      <Section title={t("settings.title")} action={
        <button className="btn-primary" onClick={save}><Save size={14}/>{t("common.save")}{saved && " ✓"}</button>
      }>
        {/* General */}
        <div className="card space-y-3">
          <h3 className="font-semibold">{t("settings.general")}</h3>
          <Row label={t("common.language")}>
            <select className="input max-w-[200px]" value={settings.language} onChange={(e) => setSettings({ ...settings, language: e.target.value as "zh" | "en" })}>
              <option value="zh">中文</option>
              <option value="en">English</option>
            </select>
          </Row>
          <Row label={t("settings.auto_launch")}>
            <input type="checkbox" checked={settings.auto_launch} onChange={(e) => setSettings({ ...settings, auto_launch: e.target.checked })} />
          </Row>
          <Row label={t("settings.logs_max_entries")}>
            <input type="number" className="input max-w-[160px]" value={settings.logs_max_entries}
              onChange={(e) => setSettings({ ...settings, logs_max_entries: parseInt(e.target.value, 10) || 1000 })} />
          </Row>
        </div>

        {/* Proxy */}
        <div className="card space-y-3">
          <h3 className="font-semibold">{t("settings.proxy")}</h3>
          <Row label={t("settings.proxy_mode_stdio").split(" ")[0]}>
            <div className="flex gap-2">
              <button className={settings.proxy_mode === "stdio" ? "btn-primary" : "btn-ghost"} onClick={() => setSettings({ ...settings, proxy_mode: "stdio" })}>{t("settings.proxy_mode_stdio")}</button>
              <button className={settings.proxy_mode === "http"  ? "btn-primary" : "btn-ghost"} onClick={() => setSettings({ ...settings, proxy_mode: "http"  })}>{t("settings.proxy_mode_http")}</button>
            </div>
          </Row>
          {settings.proxy_mode === "http" && (
            <Row label={t("settings.proxy_http_port")}>
              <input type="number" className="input max-w-[160px]" value={settings.proxy_http_port ?? 39231}
                onChange={(e) => setSettings({ ...settings, proxy_http_port: parseInt(e.target.value, 10) || 39231 })} />
            </Row>
          )}
          <Row label="自动启动代理">
            <input type="checkbox" checked={settings.auto_start_proxy ?? false}
              onChange={(e) => setSettings({ ...settings, auto_start_proxy: e.target.checked })} />
            <span className="ml-2 text-xs text-fg-muted">应用启动时自动启动 MCP Router 服务</span>
          </Row>
        </div>

        {/* NPM / Registry */}
        <div className="card space-y-3">
          <h3 className="font-semibold">NPM / 镜像源</h3>
          <Row label="自动注入">
            <input type="checkbox" checked={settings.auto_inject_npm_registry ?? true}
              onChange={(e) => setSettings({ ...settings, auto_inject_npm_registry: e.target.checked })} />
            <span className="ml-2 text-xs text-fg-muted">启动 stdio 子 server 时自动注入 NPM_CONFIG_REGISTRY</span>
          </Row>
          <Row label="镜像 URL">
            <input className="input max-w-md" value={settings.npm_registry ?? ""}
              onChange={(e) => setSettings({ ...settings, npm_registry: e.target.value })}
              placeholder="https://registry.npmmirror.com" />
          </Row>
          <div className="flex flex-wrap gap-2 pl-32">
            {[
              ["registry.npmmirror.com", "淘宝镜像 (推荐)"],
              ["registry.npmjs.org", "官方 (国外)"],
              ["mirrors.cloud.tencent.com/npm", "腾讯云"],
              ["mirrors.huaweicloud.com/repository/npm", "华为云"],
            ].map(([u, label]) => (
              <button key={u} className="btn-ghost text-xs" onClick={() => setSettings({ ...settings, npm_registry: `https://${u}` })}>
                {label}
              </button>
            ))}
          </div>
        </div>

        {/* Market */}
        <div className="card space-y-3">
          <h3 className="font-semibold">{t("market.title")}</h3>
          <Row label={t("settings.registry_url")}>
            <input className="input max-w-md" value={settings.registry_url ?? ""}
              onChange={(e) => setSettings({ ...settings, registry_url: e.target.value || undefined })}
              placeholder="https://registry.npmmirror.com/-/v1/search?text=keyword:mcp" />
          </Row>
        </div>

        {/* About */}
        <div className="card space-y-3">
          <h3 className="font-semibold">{t("settings.about")}</h3>
          <Row label={t("settings.version")}>{info?.version ?? "—"}</Row>
          <Row label={t("settings.data_dir")}>
            <div className="flex items-center gap-2">
              <code className="text-xs text-fg-muted">{info?.data_dir ?? "—"}</code>
              <button className="btn-ghost p-1.5" onClick={() => openDataDir()} title={t("settings.open_data_dir") as string}><FolderOpen size={14}/></button>
            </div>
          </Row>
          <Row label="MCPR_TOKEN">
            <div className="flex items-center gap-2">
              <code className="text-xs text-fg-muted break-all">{settings.mcp_token ?? "(未生成)"}</code>
              {settings.mcp_token && (
                <button className="btn-ghost text-xs" onClick={() => navigator.clipboard.writeText(settings.mcp_token!)}>复制</button>
              )}
              <button
                className="btn-ghost p-1.5"
                onClick={regen}
                title="重置 token（所有客户端需更新配置）"
              >
                <RefreshCw size={14}/>
              </button>
            </div>
          </Row>
        </div>
      </Section>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <div className="w-32 shrink-0 text-sm text-fg-muted">{label}</div>
      <div className="flex-1">{children}</div>
    </div>
  );
}
