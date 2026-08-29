# MS Manage

> MCP server 与 Skill 的统一管理工具 — 类似 [mcp-router](https://github.com/mcp-router/mcp-router) 的桌面应用，Windows 优先，中文界面。

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-blueviolet)](https://tauri.app)
[![React 18](https://img.shields.io/badge/React-18-61dafb)](https://react.dev)

---

## ✨ 功能

### MCP 服务管理
- **三种 transport** — stdio / SSE / HTTP，添加、编辑、启动、停止、连通性测试
- **在线市场** — 浏览/安装社区 MCP server

### Skill 管理（Claude Code / Cursor 风格）
- **目录制 Skill** — 每个 skill 一个文件夹，含 `SKILL.md` + 可选脚本/模板
- **新建 / 编辑 / 删除 / 启用 / 禁用** — 全文搜索 + 卡片/列表两种视图
- **扫描外部目录** — 一键扫描 `~/.claude/skills/`、`~/.opencode/`、`~/.codex/` 等，自动勾选导入
- **文件列表 + 路径快链** — 卡片显示 skill 目录下的所有附件（脚本、模板），点 📂 直达文件管理器
- **LLM 中文摘要**（可选）— 配置国内 LLM（MiniMax / DeepSeek / Kimi），自动给英文描述生成 1-2 句中文摘要，结果独立缓存不污染 `SKILL.md`

### Claude Desktop 集成
- **一键导入 / 同步** `claude_desktop_config.json`，支持代理模式与逐个导出
- **本地代理** — 本应用作为单一 stdio MCP server 暴露给 Claude Desktop，统一路由到多个子 MCP server

### 使用统计
- 各 provider 的请求量 / token 用量 / 成本统计
- 自动按定价表换算（人民币）
- 历史数据备份与回放

### 其它
- 中英文双语界面（i18next）
- 全局记忆文件（Memory）管理
- 日志查看 + 清理
- 客户端接入向导（生成 Claude Desktop / Cursor / 其他 IDE 的配置 JSON）

## 🏗 架构

```
┌─────────────────┐  stdin/stdout  ┌────────────────────┐  stdin/stdout  ┌────────────────┐
│  Claude Desktop │ ─────────────► │  MS Manage (proxy) │ ─────────────► │  filesystem    │
│                 │ ◄───────────── │   stdio 模式        │ ◄───────────── │  fetch         │
└─────────────────┘                │                    │                │  github        │
                                   │  路由 + 合并 tools │                │  memory        │
                                   │  tools 前缀        │                │  ...           │
                                   │  <server>__<tool>  │                └────────────────┘
                                   └────────────────────┘
```

工具命名空间：子 server `filesystem` 的 `read_file` 工具，在代理层暴露为 `filesystem__read_file`。

## 🧱 技术栈

- **桌面框架** — Tauri 2 (Rust + WebView2)
- **前端** — React 18 + TypeScript + Vite + Tailwind CSS + lucide-react
- **后端** — Rust + tokio + serde + reqwest
- **国际化** — react-i18next（中/英）
- **打包** — NSIS `.exe` 安装包（perMachine 安装模式）

## 📂 目录结构

```
ms_manage/
├── src/                      # 前端 (React)
│   ├── components/           # Layout, Modal, Section, ToastSentinel...
│   ├── pages/                # Dashboard, Servers, Skills, Model, Logs, Settings, ...
│   ├── stores/               # zustand
│   ├── i18n/                 # zh.ts + en.ts
│   ├── lib/                  # ipc.ts (IPC 契约), errors, toast
│   └── App.tsx
├── src-tauri/                # 后端 (Rust)
│   ├── src/
│   │   ├── main.rs           # CLI 路由（gui / proxy stdio / proxy http）
│   │   ├── lib.rs            # Tauri Builder 装配
│   │   ├── cli.rs            # 命令行参数解析
│   │   ├── state.rs          # 全局 AppState
│   │   ├── cmd_*.rs          # Tauri command（按域拆分: skills / servers / model / ...）
│   │   ├── model/            # LLM 客户端（Anthropic 协议 + OpenAI 兼容 GET /v1/models）
│   │   ├── proxy/            # 子 server 注册表 + stdio 代理主循环
│   │   ├── storage/          # JSON 持久化层
│   │   └── usage/            # 使用统计聚合
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── Cargo.toml
├── icons/                    # 应用图标
├── docs/                     # 设计文档
└── package.json
```

## 💾 数据存储

应用数据存到系统数据目录（不污染仓库）：

| 路径 | 内容 |
|------|------|
| `%LOCALAPPDATA%\com.msmanage.desktop\servers.json` | MCP server 配置 |
| `%LOCALAPPDATA%\com.msmanage.desktop\skills\*.md` | Skill Markdown 文件（每个 skill 一个子目录） |
| `%LOCALAPPDATA%\com.msmanage.desktop\skill_analyses.json` | LLM 中文摘要缓存（独立文件，不写回 SKILL.md） |
| `%LOCALAPPDATA%\com.msmanage.desktop\settings.json` | 应用设置（含 LLM 配置、provider keys） |
| `%LOCALAPPDATA%\com.msmanage.desktop\claude-backups\*.bak.json` | Claude Desktop 配置备份 |

## 🚀 开发

```bash
# 一次性：装依赖（已配 .npmrc 国内镜像）
npm install

# 开发模式（热重载）
npm run tauri:dev

# 仅前端开发（不开 Rust 窗口）
npm run dev

# 类型检查 + ESLint + i18n 一致性 + 单元测试
npx tsc --noEmit
npm run lint
npm run check:i18n
npm test

# 类型检查 + 生产前端构建
npm run build
```

## 📦 构建 Windows 安装包

```bash
# NSIS .exe 安装包（产物在 src-tauri/target/release/bundle/nsis/）
npm run tauri:build

# 仅 debug 构建（不 bump 版本号）
npm run tauri:build:debug
```

## 🔌 接入 MCP 客户端

推荐方式（不需要在客户端机器安装 MS Manage 主程序）：

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

把 `MS_MANAGE_TOKEN` 换成 MS Manage **设置**页显示的 token。MS Manage 必须保持运行。

或在 MS Manage 应用的 **客户端接入** 页一键复制配置。

## 🤝 贡献

欢迎 PR、Issue、discussion。提交前请跑：

```bash
npx tsc --noEmit && npm run lint && npm test
```

## 📝 License

[MIT](LICENSE) — Copyright (c) 2024-2026 MS Manage Contributors