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
  revealToken: () => Promise<void>;
  clearRevealedToken: () => void;
}

export function useConnections(): UseConnections {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ data_dir: string } | null>(null);
  const [routerConfig, setRouterConfig] = useState<RouterConfig | null>(null);
  const [proxy, setProxy] = useState<ProxyStatus | null>(null);
  const [revealedToken, setRevealedToken] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [s, i, r, p] = await Promise.all([
          getSettings().catch(() => null),
          appInfo().catch(() => null),
          getRouterConfig().catch(() => null),
          getProxyStatus().catch(() => null),
        ]);
        if (cancelled) return;
        if (s) setSettings(s);
        if (i) setInfo(i);
        setRouterConfig(r);
        setProxy(p);
      } catch {
        // 所有 IPC 都失败时静默，页面会显示空状态
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  /** 用户显式点击"显示真实 token"按钮后才调用 —— 真实 token 一进 store 就走。 */
  const revealTokenFn = useCallback(async () => {
    try {
      const t = await revealToken();
      setRevealedToken(t);
    } catch {
      // 静默 —— UI 上会显示"未生成"提示
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
  const httpConfig = useMemo(() => {
    if (!routerConfig) return null;
    const tokenVal = revealedToken ?? "<MS_MANAGE_TOKEN>";
    const obj = {
      mcpServers: {
        ...(routerConfig.mcpServers as Record<string, any>),
        "ms-manage": {
          ...((routerConfig.mcpServers as Record<string, any>)?.["ms-manage"] ?? {}),
          headers: { Authorization: `Bearer ${tokenVal}` },
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
    revealToken: revealTokenFn,
    clearRevealedToken,
  };
}