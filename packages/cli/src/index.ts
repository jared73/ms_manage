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
import { connect } from "./connect.js";

const require = createRequire(import.meta.url);
const subcommand = process.argv[2];

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
  try {
    const pkg = require("../package.json") as { version?: string };
    process.stdout.write(`ms-manage ${pkg.version ?? "?"}\n`);
  } catch {
    process.stdout.write("ms-manage (dev)\n");
  }
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
  // 任何位置参数或者无参数 = connect 模式
  await connect();
}

main().catch((err) => {
  process.stderr.write(`[ms-manage] fatal: ${err?.message ?? err}\n`);
  process.exit(1);
});