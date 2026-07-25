import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Save, FolderOpen, RefreshCw, Power, Stethoscope, CheckCircle2, AlertTriangle, XCircle, Loader2, Copy } from "lucide-react";
import { toast } from "sonner";
import { Section } from "@/components/Section";
import { Modal } from "@/components/Modal";
import { getSettings, updateSettings, appInfo, openDataDir, regenerateToken, revealToken, rewriteClientConfigs, enableAutostart, disableAutostart, isAutostartEnabled, doctorCheck } from "@/lib/ipc";
import type { AppSettings, DoctorItem } from "@/lib/ipc";
import { useApp, previewTheme, readPersistedUi, type ThemeName } from "@/stores/app";
import { describeError } from "@/lib/errors";

export function Settings() {
  const { t, i18n } = useTranslation();
  const storeSettings = useApp((s) => s.settings);
  const setStoreSettings = useApp((s) => s.setSettings);
  // 后端 settings 还没回来时使用 localStorage 的 fallback(避免空白)
  const persistedTheme = readPersistedUi().theme;
  const activeTheme = (storeSettings?.theme as ThemeName | undefined) ?? persistedTheme;
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ version: string; data_dir: string } | null>(null);
  // Token 重置双确认:用户首次点"重新生成"时弹窗说明影响 + 让用户输入当前的 token 末 4 位
  // (避免误点一刀斩断所有客户端的连接)
  const [confirmRegen, setConfirmRegen] = useState(false);
  const [regenCode, setRegenCode] = useState("");
  const [regenBusy, setRegenBusy] = useState(false);
  // 打开 modal 时,把当时 token 的末 4 位冻结到 local state —— 防止用户输入期间
  // 后端 token 被另一条路径改了(理论上不会,但 modal 期间派生会让占位与校验脱节)。
  const [regenTokenTail, setRegenTokenTail] = useState("");
  const [saved, setSaved] = useState(false);
  const [autoLaunch, setAutoLaunchState] = useState(false);
  const [doctor, setDoctor] = useState<DoctorItem[] | null>(null);
  const [doctorRunning, setDoctorRunning] = useState(false);

  // "已保存" 提示 2000ms 自动收起 —— 卸载时清掉,避免 setState 在已卸载组件上
  useEffect(() => {
    if (!saved) return;
    const handle = window.setTimeout(() => setSaved(false), 2000);
    return () => window.clearTimeout(handle);
  }, [saved]);

  const runDoctor = async () => {
    setDoctorRunning(true);
    try {
      setDoctor(await doctorCheck());
    } catch (e) {
      toast.error(t("settings.doctor_failed", { msg: describeError(e) }) as string);
    } finally {
      setDoctorRunning(false);
    }
  };

  useEffect(() => {
    void (async () => {
      try {
        setSettings(await getSettings());
        setInfo(await appInfo());
        setAutoLaunchState(await isAutostartEnabled().catch(() => false));
      } catch (e) {
        toast.error(t("settings.load_failed", { msg: describeError(e) }) as string);
      }
    })();
  }, []);

  const handleAutoLaunch = async (next: boolean) => {
    try {
      if (next) { await enableAutostart(); } else { await disableAutostart(); }
      setAutoLaunchState(next);
      toast.success(t(next ? "settings.autostart_on" : "settings.autostart_off") as string, { duration: 1500 });
    } catch (e) {
      toast.error(t("settings.autostart_failed", { msg: describeError(e) }) as string);
    }
  };

  const save = async () => {
    if (!settings) return;
    try {
      await updateSettings(settings);
      setStoreSettings(settings);
      setSaved(true);
      void i18n.changeLanguage(settings.language);
    } catch (e) {
      toast.error(t("settings.save_failed", { msg: describeError(e) }) as string);
    }
  };

  const switchTheme = async (theme: ThemeName) => {
    if (!settings || theme === activeTheme) return;
    previewTheme(theme); // 立即反映到 DOM,等 setStoreSettings 兜底
    const updated = { ...settings, theme };
    setSettings(updated);
    try {
      await updateSettings(updated);
      setStoreSettings(updated);
      toast.success(t("settings.theme_switched") as string, { duration: 1500 });
    } catch (e) {
      toast.error(t("settings.theme_update_failed", { msg: describeError(e) }) as string);
    }
  };

  const regen = async () => {
    // 用户先在确认弹窗里点 "确认重新生成" 才能进这里 —— 二段确认
    setRegenBusy(true);
    try {
      const { token: newToken } = await regenerateToken();
      // regen 返回完整 token —— 我们只在用户"复制"那一刻把它放进剪贴板,
      // 不写回 settings store(那里只放掩码版)。下一次 get_settings 会拿到新掩码。
      setSettings({
        ...settings!,
        mcp_token: newToken.slice(0, 5) + "***" + newToken.slice(-4),
        mcp_token_present: true,
      });
      const written = await rewriteClientConfigs(true);
      toast.success(t("settings.token_reset_success", { targets: written.join(", ") }) as string);
      setConfirmRegen(false);
      setRegenCode("");
    } catch (e) {
      toast.error(t("settings.token_reset_failed", { msg: describeError(e) }) as string);
    } finally {
      setRegenBusy(false);
    }
  };

  /** 当前 token 末 4 位 —— 不在这里直接派生,而是冻结到 regenTokenTail,
   * 因为 modal 开期间外部不应让校验逻辑漂移。 */

  /** 用户点"复制 token"时,显式问后端要真实 token(不会自动出现在 DOM/store 里)。 */
  const copyToken = async () => {
    try {
      const realToken = await revealToken();
      await navigator.clipboard.writeText(realToken);
      toast.success(t("settings.copied"), { duration: 1200 });
    } catch (e) {
      toast.error(t("settings.token_reveal_failed", { msg: describeError(e) }) as string);
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
              <option value="zh">{t("settings.lang_zh")}</option>
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
          <h3 className="font-semibold">{t("settings.npm_card_title")}</h3>
          <Row label={t("settings.auto_inject_npm_registry")}>
            <input type="checkbox" checked={settings.auto_inject_npm_registry ?? true}
              onChange={(e) => {
                const updated = { ...settings, auto_inject_npm_registry: e.target.checked };
                setSettings(updated);
                updateSettings(updated).catch((err) => {
                  toast.error(t("settings.save_failed", { msg: describeError(err) }) as string);
                });
              }} />
            <span className="ml-2 text-xs text-fg-muted">
              {t("settings.npm_registry_card_desc")} <code className="text-fg">NPM_CONFIG_REGISTRY={"registry.npmmirror.com"}</code>
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
              const active = activeTheme === id;
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
                <div className="text-sm font-medium">{t("settings.auto_launch")}</div>
                <div className="text-xs text-fg-muted">
                  {t("settings.autostart_card_desc")}
                </div>
              </div>
            </div>
            <button
              onClick={() => handleAutoLaunch(!autoLaunch)}
              role="switch"
              aria-checked={autoLaunch}
              aria-label={t("settings.autostart_card_title")}
              className={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${autoLaunch ? "bg-accent" : "bg-overlay-soft"}`}
              title={t(autoLaunch ? "common.enabled" : "common.disabled")}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full bg-bg-subtle shadow-sm transition-all ${autoLaunch ? "left-5" : "left-0.5"}`}
              />
            </button>
          </div>
        </div>

        {/* 一键体检 */}
        <div className="card space-y-3">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-semibold">{t("settings.doctor")}</h3>
              <div className="text-xs text-fg-muted">{t("settings.doctor_desc")}</div>
            </div>
            <button className="btn-primary inline-flex items-center gap-1.5" onClick={runDoctor} disabled={doctorRunning}>
              {doctorRunning ? <Loader2 size={14} className="animate-spin"/> : <Stethoscope size={14}/>}
              {doctorRunning ? t("settings.doctor_running") : doctor ? t("settings.doctor_rerun") : t("settings.doctor_run")}
            </button>
          </div>
          {doctor && (
            <ul className="space-y-2">
              {doctor.map((it) => (
                <li key={it.id} className="rounded-lg border border-border bg-bg-subtle px-3 py-2">
                  <div className="flex items-start gap-2.5">
                    <span className="mt-0.5 shrink-0">
                      {it.status === "ok" ? <CheckCircle2 size={15} className="text-ok"/>
                        : it.status === "warn" ? <AlertTriangle size={15} className="text-amber-400"/>
                        : <XCircle size={15} className="text-err"/>}
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-baseline gap-2">
                        <span className="text-sm font-medium">{it.title}</span>
                        <span className="text-xs text-fg-muted">{it.detail}</span>
                      </div>
                      {it.suggestion && (
                        <div className="mt-1 flex items-start gap-1.5 text-xs text-amber-300">
                          <span className="flex-1 break-all">{it.suggestion}</span>
                          <button
                            className="btn-ghost shrink-0 p-0.5"
                            title={t("settings.copy_suggestion") as string}
                            onClick={() => { navigator.clipboard.writeText(it.suggestion!).catch(() => {}); toast.success(t("settings.copied") as string, { duration: 1200 }); }}
                          >
                            <Copy size={11}/>
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
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
              <code className="text-xs text-fg-muted break-all">
                {settings.mcp_token_present ? settings.mcp_token : t("settings.token_not_generated")}
              </code>
              {settings.mcp_token_present && (
                <button className="btn-ghost text-xs" onClick={copyToken}>{t("common.copy")}</button>
              )}
              <button
                className="btn-ghost p-1.5"
                onClick={() => {
                  // 打开 modal 时冻结末 4 位,关闭时清空
                  const tail = (settings?.mcp_token ?? "").split("***").pop() ?? "";
                  setRegenTokenTail(tail);
                  setRegenCode("");
                  setConfirmRegen(true);
                }}
                title={t("settings.token_regen_tooltip") as string}
              >
                <RefreshCw size={14}/>
              </button>
            </div>
          </Row>
        </div>
      </Section>

      {/* 重新生成 Token 双确认弹窗 */}
      {confirmRegen && (
        <Modal
          open
          size="sm"
          title={t("settings.token_regen_confirm_title")}
          onClose={() => { if (!regenBusy) { setConfirmRegen(false); setRegenCode(""); setRegenTokenTail(""); } }}
          footer={
            <>
              <button
                className="btn-ghost"
                onClick={() => { setConfirmRegen(false); setRegenCode(""); setRegenTokenTail(""); }}
                disabled={regenBusy}
              >
                {t("common.cancel")}
              </button>
              <button
                className="btn-primary bg-err hover:bg-err/80 text-bg disabled:opacity-60"
                onClick={regen}
                disabled={regenBusy || regenCode.trim() !== regenTokenTail}
                title={regenCode.trim() !== regenTokenTail ? t("settings.token_regen_match_required") as string : undefined}
              >
                {regenBusy ? <Loader2 size={14} className="inline animate-spin mr-1" /> : null}
                {t("settings.token_regen_confirm_btn")}
              </button>
            </>
          }
        >
          <p className="text-sm text-fg-muted">{t("settings.token_regen_confirm_desc")}</p>
          <div className="mt-3 rounded-md border border-border bg-bg-subtle p-3">
            <div className="mb-1 text-[11px] text-fg-muted">{t("settings.token_regen_input_label")}</div>
            <input
              className="input font-mono"
              value={regenCode}
              onChange={(e) => setRegenCode(e.target.value)}
              placeholder={regenTokenTail}
              autoComplete="off"
              spellCheck={false}
            />
            <div className="mt-1 text-[11px] text-fg-muted">
              {t("settings.token_regen_input_hint", { tail: regenTokenTail })}
            </div>
          </div>
        </Modal>
      )}
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
