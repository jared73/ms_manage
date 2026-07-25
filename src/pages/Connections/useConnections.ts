/**
 * Connections 页数据 hook:拉 settings / info / routerConfig / proxy,组装 npxConfig / httpConfig。
 *
 * 安全:默认 npxConfig / httpConfig 不包含完整 token —— 它们是用户"点确认后"
 * 才显示的 mask-only 版本。要拿真实 token + 完整 config,走 reveal_token 命令。
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { getSettings, getRouterConfig, appInfo, getProxyStatus, revealToken } from "@/lib/ipc";
import type { AppSettings } from "@/lib/ipc";

type RouterConfig = Awaited<ReturnType<typeof getRouterConfig>> & {
  /** 后端新增字段(掩码 token 改动):token 是否已生成 —— UI 根据它决定"显示 token"按钮要不要渲染。 */
  token_present?: boolean;
};
type ProxyStatus = Awaited<ReturnType<typeof getProxyStatus>>;

export interface UseConnections {
  settings: AppSettings | null;
  info: { data_dir: string } | null;
  routerConfig: RouterConfig | null;
  proxy: ProxyStatus | null;
  npxConfig: string | null;
  httpConfig: string | null;
  /** 用户点"显示真实 token"后才会拿到的完整 token。否则 null。 */
  revealedToken: string | null;
  /** 首屏拉取是否完成 —— false 时显示骨架而非空状态 */
  loading: boolean;
  /** 首屏拉取中至少有一个 IPC 报错;UI 显示"重试"按钮 */
  loadError: boolean;
  /** 触发重新拉所有数据,可用于 IPC 出错时的"重试"按钮 */
  reload: () => void;
  revealToken: () => Promise<void>;
  clearRevealedToken: () => void;
}

export function useConnections(): UseConnections {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ data_dir: string } | null>(null);
  const [routerConfig, setRouterConfig] = useState<RouterConfig | null>(null);
  const [proxy, setProxy] = useState<ProxyStatus | null>(null);
  const [revealedToken, setRevealedToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  // 重试令牌:每次变化都重新触发加载 effect
  const [reloadTick, setReloadTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(false);
    void (async () => {
      // 用 settled 跟踪而不是 try/catch,因为单个 IPC 失败不能让整页崩
      const settled = await Promise.allSettled([
        getSettings(),
        appInfo(),
        getRouterConfig(),
        getProxyStatus(),
      ]);
      if (cancelled) return;
      let anyError = false;
      // settings 失败 → 整页都没法用,标记 loadError;其他失败 → 仅置空,UI 显示 "—"
      const [s, i, r, p] = settled;
      if (s.status === "fulfilled") setSettings(s.value);
      else { anyError = true; setSettings(null); }
      if (i.status === "fulfilled") setInfo({ data_dir: i.value.data_dir });
      else { anyError = true; setInfo(null); }
      if (r.status === "fulfilled") setRouterConfig(r.value);
      else { anyError = true; setRouterConfig(null); }
      if (p.status === "fulfilled") setProxy(p.value);
      else { anyError = true; setProxy(null); }
      setLoadError(anyError);
      setLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, [reloadTick]);

  const reload = useCallback(() => setReloadTick((n) => n + 1), []);

  /** 用户显式点击"显示真实 token"按钮后才调用 —— 真实 token 一进 store 就走。 */
  const revealTokenFn = useCallback(async () => {
    try {
      const t = await revealToken();
      setRevealedToken(t);
    } catch {
      setRevealedToken(null);
      // 不 rethrow —— 调用方如果关心可以轮询 revealedToken === null + 长时间如此 → 报错
    }
  }, []);

  const clearRevealedToken = useCallback(() => setRevealedToken(null), []);

  // npx 形式的客户端配置 —— 默认 token 占位,reveal 之后替换为真值
  const npxConfig = useMemo(() => {
    if (!settings) return null;
    const port = settings.proxy_http_port ?? 39231;
    const tokenVal = revealedToken ?? "<MS_MANAGE_TOKEN>";
    const obj = {
      mcpServers: {
        "ms-manage": {
          command: "npx",
          args: ["-y", "@ms-manage/cli@latest", "connect"],
          env: {
            MS_MANAGE_URL: `http://127.0.0.1:${port}/mcp`,
            MS_MANAGE_TOKEN: tokenVal,
          },
        },
      },
    };
    return JSON.stringify(obj, null, 2);
  }, [settings, revealedToken]);

  // HTTP 直连配置 —— 同样默认不填 token,reveal 后填
  // 用窄类型替代 Record<string, any>:
  // - 之前 cast 到 any 后,如果后端 mcpServers 是 null/非对象,JSON.stringify 仍合法但
  //   Claude Desktop 那边 headers 字段缺失,粘过去客户端崩。这里给个明确 null 退出。
  const httpConfig = useMemo(() => {
    if (!routerConfig) return null;
    const existing = routerConfig.mcpServers;
    if (typeof existing !== "object" || existing === null || Array.isArray(existing)) {
      // 后端 schema 错误:不拼接,前端让用户看到那段说明但不输出错误 JSON
      return null;
    }
    const obj = {
      mcpServers: {
        ...(existing as Record<string, unknown>),
        "ms-manage": {
          ...((existing as Record<string, unknown>)["ms-manage"] as Record<string, unknown> | undefined ?? {}),
          headers: { Authorization: `Bearer ${revealedToken ?? "<MS_MANAGE_TOKEN>"}` },
        },
      },
    };
    return JSON.stringify(obj, null, 2);
  }, [routerConfig, revealedToken]);

  return {
    settings,
    info,
    routerConfig,
    proxy,
    npxConfig,
    httpConfig,
    revealedToken,
    loading,
    loadError,
    reload,
    revealToken: revealTokenFn,
    clearRevealedToken,
  };
}
