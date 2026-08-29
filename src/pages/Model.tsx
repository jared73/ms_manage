import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Save, RefreshCw, Eye, EyeOff, Loader2, CheckCircle2, AlertCircle, AlertTriangle } from "lucide-react";
import { Section } from "@/components/Section";
import { toast } from "sonner";
import {
  listModelPresets, getModelConfig, saveModelConfig, fetchModelList,
  getProviderKeys,
  type ModelPreset, type ModelConfig,
} from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function Model() {
  const { t } = useTranslation();
  const [presets, setPresets] = useState<ModelPreset[]>([]);
  const [config, setConfig] = useState<ModelConfig>({
    provider: "deepseek",
    apiKey: "",
    baseUrl: "",
    model: "",
    contextSize: 8192,
  });
  const [showKey, setShowKey] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [autoFetching, setAutoFetching] = useState(false);  // 静默拉取中的状态(用于 spinner)
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [fetchErr, setFetchErr] = useState<string | null>(null);
  // 配置迁移提示:已保存的 baseUrl 与当前 preset 默认不一致(常见于预设更新后老配置残留)
  const [urlMismatch, setUrlMismatch] = useState<{ saved: string; expected: string } | null>(null);
  // per-provider API Key 备份 —— 切来切去不丢 key
  // (用户反馈:"切一下就变成其他的了",原来的清空策略太激进)
  const [providerKeys, setProviderKeys] = useState<Record<string, string>>({});

  // 加载 preset + 已保存配置
  useEffect(() => {
    (async () => {
      try {
        const [ps, saved, savedKeys] = await Promise.all([
          listModelPresets(), getModelConfig(), getProviderKeys(),
        ]);
        setPresets(ps);
        setProviderKeys(savedKeys);
        if (saved) {
          // 把已存的 model_config.api_key 也合并进 provider_keys(老数据兼容)
          if (saved.apiKey && !savedKeys[saved.provider]) {
            savedKeys[saved.provider] = saved.apiKey;
            setProviderKeys({ ...savedKeys });
          }
          // 当前显示的 key 取自 provider_keys(可能比 saved.apiKey 更新,因为切过别的 provider)
          const activeKey = savedKeys[saved.provider] ?? saved.apiKey ?? "";
          setConfig({ ...saved, apiKey: activeKey });
          const p = ps.find((x) => x.id === saved.provider);
          if (p) {
            // 模型下拉候选 = [saved.model, 默认, preset.known_models, 远端拉到的] 去重保序
            // 用户反馈"应该能看到所有可选的模型吧" —— 即使 API 拉失败,known_models
            // 也能提供候选;saved.model 保证用户保存的值不会丢
            const initialModels = Array.from(new Set(
              [saved.model, p.defaultModel, ...p.knownModels].filter(Boolean),
            ));
            setModels(initialModels);
            if (!saved.baseUrl.trim().endsWith("/anthropic") && p.defaultBaseUrl.endsWith("/anthropic")) {
              setUrlMismatch({ saved: saved.baseUrl, expected: p.defaultBaseUrl });
            } else if (saved.baseUrl !== p.defaultBaseUrl) {
              setUrlMismatch({ saved: saved.baseUrl, expected: p.defaultBaseUrl });
            }
            // 有 apiKey 就自动拉一次 —— 用户反馈"应该能看到所有可选的模型吧",
            // 不用每次都手动点"获取模型列表"
            if (activeKey.trim()) {
              void autoFetchModels({ ...saved, apiKey: activeKey }, p);
            }
          }
        } else {
          // 用第一个 preset 作为初始
          const first = ps[0];
          if (first) {
            setConfig({
              provider: first.id,
              apiKey: "",
              baseUrl: first.defaultBaseUrl,
              model: first.defaultModel,
              contextSize: first.defaultContextSize,
            });
            setModels(Array.from(new Set([first.defaultModel, ...first.knownModels].filter(Boolean))));
          }
        }
      } catch (e) {
        toast.error(describeError(e));
      }
    })();
  }, []);

  const preset = presets.find((p) => p.id === config.provider);

  // 切 preset → 不再清 key —— 用 providerKeys 缓存切换前的 key,新供应商有缓存
  // 就恢复出来,没缓存就空。配合后端 provider_keys 持久化,真正做到"切来切去不丢"。
  // (用户反馈:"切一下就变成其他的了" — 之前的清空策略太激进)
  const handlePresetChange = (id: string) => {
    const p = presets.find((x) => x.id === id);
    if (!p) return;
    // 一次性算出新 key map:把当前 key 存回旧 provider,新 provider 的 key 读出来
    const newKeys = { ...providerKeys };
    if (config.apiKey.trim()) newKeys[config.provider] = config.apiKey;
    const restoredKey = newKeys[id] ?? "";
    setProviderKeys(newKeys);
    setConfig({
      provider: id,
      apiKey: restoredKey,
      baseUrl: p.defaultBaseUrl,
      model: p.defaultModel,
      contextSize: p.defaultContextSize,
    });
    setModels(Array.from(new Set([p.defaultModel, ...p.knownModels].filter(Boolean))));
    setFetchErr(null);
  };

  // 拉模型列表
  const handleFetchModels = async () => {
    if (!config.apiKey.trim()) {
      toast.error(t("model.api_key_required") as string);
      return;
    }
    setLoadingModels(true);
    setFetchErr(null);
    try {
      const list = await fetchModelList(config);
      const ids = Array.from(new Set(
        [config.model, preset?.defaultModel, ...(preset?.knownModels ?? []), ...list.map((m) => m.id)]
          .filter((x): x is string => Boolean(x)),
      ));
      setModels(ids);
      if (list.length === 0) {
        setFetchErr(t("model.fetch_empty_warn") as string);
      } else {
        toast.success(t("model.fetch_success", { count: list.length }) as string);
      }
    } catch (e) {
      setFetchErr(describeError(e));
    } finally {
      setLoadingModels(false);
    }
  };

  // 静默版拉模型 —— load 时自动调用,不弹 toast,失败也不打扰用户
  const autoFetchModels = async (cfg: ModelConfig, _preset: ModelPreset) => {
    setAutoFetching(true);
    try {
      const list = await fetchModelList(cfg);
      // 合并:已保存模型 + 默认 + preset 内置清单 + 远端拉到的,去重保序
      const ids = Array.from(new Set(
        [cfg.model, _preset.defaultModel, ..._preset.knownModels, ...list.map((m) => m.id)].filter(Boolean),
      ));
      setModels(ids);
    } catch {
      // 静默失败:用户没主动点就不弹错
    } finally {
      setAutoFetching(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveModelConfig(config);
      setSaved(true);
      toast.success(t("model.saved") as string);
      setTimeout(() => setSaved(false), 1500);
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-4 p-6">
      <Section title={t("model.title")}>
        <p className="text-sm text-fg-muted">{t("model.subtitle")}</p>
        {urlMismatch && (
          // 配置迁移提示:已保存的 baseUrl 与当前 preset 默认不一致(常见于预设升级后
          // 老配置残留,典型:MiniMax 域名从 api.MiniMax.cn 改为 api.minimaxi.com)。
          // 一键更新到当前默认,保留用户的 key/model 不变。
          <div className="mt-3 flex items-start gap-2 rounded-md border border-warn/40 bg-warn/10 p-3">
            <AlertTriangle className="icon-sm mt-0.5 shrink-0 text-warn" />
            <div className="min-w-0 flex-1 text-xs">
              <div className="font-medium text-warn">{t("model.url_mismatch_title")}</div>
              <div className="mt-1 text-fg-muted">
                {t("model.url_mismatch_saved")}: <code className="font-mono">{urlMismatch.saved}</code>
                <br />
                {t("model.url_mismatch_expected")}: <code className="font-mono">{urlMismatch.expected}</code>
              </div>
              <div className="mt-2 flex gap-2">
                <button
                  className="btn-primary"
                  onClick={() => {
                    setConfig((c) => ({ ...c, baseUrl: urlMismatch.expected }));
                    setUrlMismatch(null);
                    toast.success(t("model.url_mismatch_applied") as string);
                  }}
                >
                  {t("model.url_mismatch_apply")}
                </button>
                <button
                  className="btn-ghost"
                  onClick={() => setUrlMismatch(null)}
                >
                  {t("model.url_mismatch_ignore")}
                </button>
              </div>
            </div>
          </div>
        )}
      </Section>

      <Section title={t("model.provider")}>
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          <div>
            <label className="mb-1 block text-xs text-fg-muted">{t("model.provider_label")}</label>
            <select
              className="input w-full"
              value={config.provider}
              onChange={(e) => handlePresetChange(e.target.value)}
            >
              {presets.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
            {preset && (
              <p className="mt-1 text-2xs text-fg-muted/70">
                {t("model.protocol_label")}: <code>{preset.protocol}</code>
              </p>
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs text-fg-muted">{t("model.base_url_label")}</label>
            <input
              className="input w-full font-mono text-xs"
              value={config.baseUrl}
              onChange={(e) => setConfig({ ...config, baseUrl: e.target.value })}
              placeholder="https://..."
              spellCheck={false}
            />
          </div>
        </div>
      </Section>

      <Section title={t("model.api_key_title")}>
        <div className="max-w-md">
          <div className="relative">
            <input
              type={showKey ? "text" : "password"}
              className="input w-full pr-10 font-mono text-xs"
              value={config.apiKey}
              onChange={(e) => setConfig({ ...config, apiKey: e.target.value })}
              placeholder="sk-..."
              spellCheck={false}
              autoComplete="off"
            />
            <button
              type="button"
              className="absolute right-1 top-1/2 -translate-y-1/2 rounded p-1 text-fg-muted hover:text-fg"
              onClick={() => setShowKey((s) => !s)}
              title={showKey ? t("model.hide_key") as string : t("model.show_key") as string}
            >
              {showKey ? <EyeOff className="icon-sm" /> : <Eye className="icon-sm" />}
            </button>
          </div>
          <p className="mt-1 text-2xs text-fg-muted">{t("model.api_key_hint")}</p>
        </div>
      </Section>

      <Section
        title={t("model.model_title")}
        action={
          <button
            className="btn-ghost"
            onClick={handleFetchModels}
            disabled={loadingModels}
          >
            {loadingModels ? <Loader2 className="icon-sm animate-spin" /> : <RefreshCw className="icon-sm" />}
            {t("model.fetch_btn")}
          </button>
        }
      >
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
          <div>
            <label className="mb-1 block text-xs text-fg-muted">{t("model.model_label")}</label>
            {/* select(全展示) + "其他"选项展开自定义输入。
                替代之前的 datalist+芯片区组合 —— 芯片区视觉臃肿像一堆输入框(用户反馈),
                而 datalist 默认按输入过滤看不到全部选项。 */}
            <div className="relative">
              <select
                className="input w-full appearance-none pr-10 font-mono text-xs"
                value={models.includes(config.model) ? config.model : "__other__"}
                onChange={(e) => {
                  if (e.target.value === "__other__") return; // 由下方输入框处理
                  setConfig({ ...config, model: e.target.value });
                }}
              >
                {models.map((m) => <option key={m} value={m}>{m}</option>)}
                <option value="__other__">{t("model.model_other")}</option>
              </select>
              <div className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1 text-fg-muted">
                {autoFetching && <Loader2 className="icon-xs animate-spin" />}
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                  <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                </svg>
                <button
                  type="button"
                  className="btn-ghost pointer-events-auto p-1 text-fg-muted hover:text-fg disabled:opacity-50"
                  onClick={(e) => { e.stopPropagation(); void handleFetchModels(); }}
                  disabled={loadingModels || !config.apiKey.trim()}
                  title={t("model.fetch_btn") as string}
                >
                  {loadingModels ? <Loader2 className="icon-xs animate-spin" /> : <RefreshCw className="icon-xs" />}
                </button>
              </div>
            </div>
            {/* 当前模型不在候选里(自定义名)→ 显示一个独立文本输入框,用户可改名 */}
            {!models.includes(config.model) && config.model.trim() !== "" && (
              <input
                className="input mt-1 w-full font-mono text-xs"
                value={config.model}
                onChange={(e) => setConfig({ ...config, model: e.target.value })}
                placeholder="自定义模型名"
                spellCheck={false}
              />
            )}
            {fetchErr && (
              <p className="mt-1 flex items-center gap-1 text-2xs text-warn">
                <AlertCircle className="icon-xs" /> {fetchErr}
              </p>
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs text-fg-muted">{t("model.context_label")}</label>
            <input
              type="number"
              min={1024}
              max={1_048_576}
              step={1024}
              className="input w-full"
              value={config.contextSize}
              onChange={(e) => setConfig({ ...config, contextSize: Math.max(1024, Math.min(1_048_576, Number(e.target.value) || 0)) })}
            />
            {/* 快捷预设 chip —— 用户反馈"上下文大小这块也不对":不同模型的 context 窗口不同,
                用 chip 一键选比下拉更直观,也能手动输入自定义值 */}
            <div className="mt-1.5 flex flex-wrap gap-1">
              {(preset?.contextOptions ?? [8192, 32000, 128000, 200000]).map((c) => (
                <button
                  key={c}
                  type="button"
                  className={`rounded border px-2 py-0.5 text-2xs transition-colors ${
                    config.contextSize === c
                      ? "border-accent bg-accent/15 text-accent"
                      : "border-border text-fg-muted hover:border-accent/40 hover:text-fg"
                  }`}
                  onClick={() => setConfig({ ...config, contextSize: c })}
                >
                  {c >= 1024 ? `${(c / 1024).toFixed(0)}K` : `${c}`}
                </button>
              ))}
            </div>
            <p className="mt-1 text-2xs text-fg-muted/70">{t("model.context_hint")}</p>
          </div>
        </div>
      </Section>

      <div className="flex items-center justify-end gap-3 pt-2">
        {saved && (
          <span className="flex items-center gap-1 text-xs text-ok">
            <CheckCircle2 className="icon-sm" />{t("model.saved_indicator")}
          </span>
        )}
        <button
          className="btn-primary"
          onClick={handleSave}
          disabled={saving || !config.apiKey.trim() || !config.model.trim() || !config.baseUrl.trim()}
        >
          {saving ? <Loader2 className="icon-sm animate-spin" /> : <Save className="icon-sm" />}
          {t("common.save")}
        </button>
      </div>
    </div>
  );
}