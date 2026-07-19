#!/usr/bin/env node
/**
 * MS Manage CLI —— stdio ↔ HTTP MCP 转发器
 *
 * 用法（在 mcpServers JSON 里）：
 * {
 *   "command": "npx",
 *   "args": ["-y", "@ms-manage/cli@latest", "connect"],
 *   "env": {
 *     "MS_MANAGE_URL":   "http://127.0.0.1:39231/mcp",
 *     "MS_MANAGE_TOKEN": "mcpr_xxx"
 *   }
 * }
 *
 * 工作模型：
 *   Claude Desktop (stdio) ─→ 本 CLI ─HTTP POST/SSE─→ MS Manage GUI (HTTP MCP)
 *
 * 协议：MS Manage 走 HTTP transport（POST 单请求 + SSE 流式响应）。
 * 客户端发 newline-delimited JSON-RPC，我们代理到 HTTP POST，
 * 收到的 SSE 事件转回 newline-delimited JSON-RPC 写到 stdout。
 */

import { createRequire } from "node:module";
import { fetch } from "undici";
import { connect } from "./connect.js";

const require = createRequire(import.meta.url);
const subcommand = process.argv[2];

function currentVersion(): string {
  try {
    const pkg = require("../package.json") as { version?: string };
    return pkg.version ?? "0.0.0";
  } catch {
    return "0.0.0";
  }
}

/** 比较语义化版本：a < b 返回 true。非标准格式按字符串兜底。 */
function isOlder(a: string, b: string): boolean {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? 0;
    const y = pb[i] ?? 0;
    if (x !== y) return x < y;
  }
  return false;
}

/**
 * 后台检查 @ms-manage/cli 是否有新版，有则在 stderr 提示升级。
 * 联网失败 / 超时 / 无新版都静默，绝不阻塞 connect 转发。
 * npx 有本地缓存，装过旧版的人不会自动升级 —— 靠这个提示引导清缓存。
 */
function checkUpdateInBackground(): void {
  const self = currentVersion();
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), 3000);
  if (typeof (t as { unref?: () => void }).unref === "function") (t as { unref: () => void }).unref();
  void (async () => {
    try {
      const res = await fetch("https://registry.npmjs.org/@ms-manage/cli/latest", {
        signal: ctrl.signal,
        headers: { Accept: "application/json" },
      });
      if (!res.ok) return;
      const data = (await res.json()) as { version?: string };
      const latest = data.version;
      if (latest && isOlder(self, latest)) {
        process.stderr.write(
          `[ms-manage] 提示：当前 CLI v${self}，已有新版 v${latest}。升级：npx clear-npx-cache 后重连，或将配置里的 @latest 改为 @${latest}。\n`
        );
      }
    } catch {
      // 静默：离线 / 超时 / 解析失败都不影响转发
    }
  })();
}

function printHelp(): void {
  process.stdout.write(`ms-manage CLI — MS Manage 客户端代理

用法:
  ms-manage connect                    stdio↔HTTP 转发模式（默认，给 mcpServers 用）
  ms-manage --help / -h                显示帮助
  ms-manage --version / -v             显示版本

环境变量（connect 模式）:
  MS_MANAGE_URL     必填。MS Manage HTTP endpoint，例如 http://127.0.0.1:39231/mcp
  MS_MANAGE_TOKEN   可选。Bearer token（MS Manage 设置里的 mcp_token）
  MS_MANAGE_TIMEOUT 可选。请求超时（毫秒），默认 30000
  MS_MANAGE_DEBUG   可选。设 1 开启 stderr 调试日志

更多信息：https://github.com/local/ms-manage
`);
}

function printVersion(): void {
  process.stdout.write(`ms-manage ${currentVersion()}\n`);
}

async function main(): Promise<void> {
  if (subcommand === "--help" || subcommand === "-h" || subcommand === "help") {
    printHelp();
    return;
  }
  if (subcommand === "--version" || subcommand === "-v" || subcommand === "version") {
    printVersion();
    return;
  }
  // Node 版本自检：给出明确提示，而不是在旧 Node 上静默崩成「连不上」。
  const [majRaw] = process.versions.node.split(".");
  const maj = parseInt(majRaw ?? "0", 10) || 0;
  if (maj < 16) {
    process.stderr.write(
      `[ms-manage] fatal: 需要 Node.js >= 16（当前 ${process.versions.node}）。请升级 Node 后重试。\n`
    );
    process.exit(2);
  }
  // 任何位置参数或者无参数 = connect 模式
  checkUpdateInBackground();
  await connect();
}

main().catch((err) => {
  process.stderr.write(`[ms-manage] fatal: ${err?.message ?? err}\n`);
  process.exit(1);
});