# @ms-manage/cli

MS Manage 客户端 CLI —— 通过 stdio ↔ HTTP 转发，让任意 MCP 客户端（Claude Code / Cursor / Cline / Windsurf / Qoder 等）一键接入 [MS Manage](https://github.com/local/ms-manage)。

## 用法

在 MCP 客户端的 `mcpServers` 里添加：

```json
{
  "mcpServers": {
    "ms-manage": {
      "command": "npx",
      "args": ["-y", "@ms-manage/cli@latest", "connect"],
      "env": {
        "MS_MANAGE_URL": "http://127.0.0.1:39231/mcp",
        "MS_MANAGE_TOKEN": "mcpr_xxx"
      }
    }
  }
}
```

MS Manage 必须保持运行。客户端不需要安装 MS Manage 主程序，只需要 Node.js 18+。

## 环境变量

| 变量 | 必填 | 说明 |
| --- | --- | --- |
| `MS_MANAGE_URL` | 是 | MS Manage HTTP endpoint，例如 `http://127.0.0.1:39231/mcp` |
| `MS_MANAGE_TOKEN` | 否 | Bearer token（MS Manage 设置里的 mcp_token） |
| `MS_MANAGE_TIMEOUT` | 否 | 请求超时毫秒，默认 `30000` |
| `MS_MANAGE_DEBUG` | 否 | 设 `1` 开启 stderr 调试日志 |

## 命令

```bash
npx -y @ms-manage/cli@latest connect   # stdio↔HTTP 转发模式
npx -y @ms-manage/cli@latest --help    # 帮助
npx -y @ms-manage/cli@latest --version # 版本
```

## License

MIT
