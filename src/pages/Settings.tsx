import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Save, FolderOpen, RefreshCw, Power } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { getSettings, updateSettings, appInfo, openDataDir, regenerateToken, rewriteClientConfigs, enableAutostart, disableAutostart, isAutostartEnabled } from "@/lib/ipc";
import type { AppSettings } from "@/lib/ipc";
import { useApp, type ThemeName } from "@/stores/app";

export function Settings() {
  const { t, i18n } = useTranslation();
  const setLang = useApp((s) => s.setLang);
  const storeTheme = useApp((s) => s.theme);
  const setStoreTheme = useApp((s) => s.setTheme);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ version: string; data_dir: string } | null>(null);
  const [saved, setSaved] = useState(false);
  const [autoLaunch, setAutoLaunchState] = useState(false);

  useEffect(() => {
    void (async () => {
      setSettings(await getSettings());
      setInfo(await appInfo());
      setAutoLaunchState(await isAutostartEnabled().catch(() => false));
    })();
  }, []);

  const handleAutoLaunch = async (next: boolean) => {
    try {
      if (next) { await enableAutostart(); } else { await disableAutostart(); }
      setAutoLaunchState(next);
      toast.success(next ? "已开启开机自启" : "已关闭开机自启", { duration: 1500 });
    } catch (e) {
      toast.error(`设置失败: ${e}`);
    }
  };

  const save = async () => {
    if (!settings) return;
    await updateSettings(settings);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
    void i18n.changeLanguage(settings.language);
    setLang(settings.language);
  };

  const switchTheme = async (theme: ThemeName) => {
    if (!settings || theme === storeTheme) return;
    setStoreTheme(theme);
    const updated = { ...settings, theme };
    setSettings(updated);
    try {
      await updateSettings(updated);
      toast.success(t("settings.theme_switched") as string, { duration: 1500 });
    } catch (e) {
      toast.error(`Failed: ${e}`);
    }
  };

  const regen = async () => {
    try {
      const newToken = await regenerateToken();
      setSettings({ ...settings!, mcp_token: newToken });
      const written = await rewriteClientConfigs(true);
      toast.success(`Token 已重置，已更新 ${written.join("、")} 配置`);
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
          <Row label={t("settings.logs_max_entries")}>
            <input type="number" className="input max-w-[160px]" value={settings.logs_max_entries}
              onChange={(e) => setSettings({ ...settings, logs_max_entries: parseInt(e.target.value, 10) || 1000 })} />
          </Row>
        </div>

        {/* NPM / Registry —— 只保留"自动注入"勾选，URL 和 4 个镜像按钮都删了
            （硬编码使用淘宝镜像，URL 改了也不生效）*/}
        <div className="card space-y-3">
          <h3 className="font-semibold">NPM / 镜像源</h3>
          <Row label="自动注入">
            <input type="checkbox" checked={settings.auto_inject_npm_registry ?? true}
              onChange={(e) => {
                const updated = { ...settings, auto_inject_npm_registry: e.target.checked };
                setSettings(updated);
                void updateSettings(updated); // 自动保存，不用点"保存"按钮
              }} />
            <span className="ml-2 text-xs text-fg-muted">
              启动 stdio 子 server 时自动注入 <code className="text-fg">NPM_CONFIG_REGISTRY=淘宝镜像</code>
            </span>
          </Row>
        </div>

        {/* 主题选择 */}
        <div className="card space-y-3">
          <div>
            <h3 className="font-semibold">{t("settings.theme_card_title")}</h3>
            <div className="text-xs text-fg-muted">{t("settings.theme_card_desc")}</div>
          </div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {([
              { id: "light",    label: t("settings.theme_light"),    swatch: "#7c3aed", bg: "#f8fafc" },
              { id: "purple",   label: t("settings.theme_purple"),   swatch: "#7c5cff", bg: "#0f1115" },
              { id: "midnight", label: t("settings.theme_midnight"), swatch: "#38bdf8", bg: "#0a1628" },
              { id: "forest",   label: t("settings.theme_forest"),   swatch: "#34d399", bg: "#0f1f15" },
            ] as const).map(({ id, label, swatch, bg }) => {
              const active = storeTheme === id;
              return (
                <button
                  key={id}
                  onClick={() => switchTheme(id)}
                  className={`group relative overflow-hidden rounded-lg border px-3 py-3 text-left transition-all ${
                    active ? "border-accent ring-2 ring-accent/40" : "border-border hover:border-accent/50"
                  }`}
                >
                  <div className="mb-2 h-10 w-full rounded-md" style={{ background: bg }}>
                    <div className="flex h-full w-full items-center justify-end pr-2">
                      <span className="inline-block h-4 w-4 rounded-full" style={{ background: swatch }} />
                    </div>
                  </div>
                  <div className="text-sm font-medium">{label}</div>
                  {active && (
                    <span className="absolute right-1 top-1 inline-flex h-4 w-4 items-center justify-center rounded-full bg-accent text-[10px] text-white">✓</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* 开机自启动 */}
        <div className="card space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Power size={16} className="text-accent" />
              <div>
                <div className="text-sm font-medium">开机自启</div>
                <div className="text-xs text-fg-muted">
                  系统启动时自动打开 MS Manage（窗口最小化到托盘）
                </div>
              </div>
            </div>
            <button
              onClick={() => handleAutoLaunch(!autoLaunch)}
              className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${autoLaunch ? "bg-accent" : "bg-overlay-soft"}`}
              title={autoLaunch ? "已启用，点此关闭" : "已关闭，点此启用"}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full bg-bg-subtle shadow-sm transition-all ${autoLaunch ? "left-5" : "left-0.5"}`}
              />
            </button>
          </div>
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
