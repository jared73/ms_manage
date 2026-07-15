# MS Manage

> MCP server 与 Skill 的统一管理工具 — 类似 [mcp-router](https://github.com/mcp-router/mcp-router) 的桌面应用，Windows 优先，中文界面。

---

## ✨ 功能

- **MCP Server 管理** — stdio / SSE / HTTP 三种 transport，添加、编辑、启动、停止、连通性测试
- **Skill 管理** — Markdown + YAML frontmatter 形式的 Skill 文件，新建/编辑/删除/启用/禁用
- **Claude Desktop 一键集成** — 导入/同步 `claude_desktop_config.json`，支持代理模式与逐个导出
- **在线市场** — 浏览/安装社区 MCP server 和 Skill
- **本地代理** — 本应用作为单一 stdio MCP server 暴露给 Claude Desktop，统一路由到多个子 MCP server

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
- **前端** — React 18 + TypeScript + Vite + Tailwind CSS
- **后端** — Rust + tokio + serde
- **国际化** — react-i18next（中/英）
- **打包** — NSIS `.exe` 安装包（perMachine 安装模式）

## 📂 目录结构

```
ms_manage/
├── src/                      # 前端 (React)
│   ├── components/           # Layout, StatusBadge, Modal, Section
│   ├── pages/                # Dashboard, Servers, Skills, ClaudeDesktop, Market, Logs, Settings
│   ├── stores/               # zustand
│   ├── i18n/                 # zh.ts + en.ts
│   ├── lib/ipc.ts            # 与 Rust 后端的 IPC 契约
│   └── App.tsx
├── src-tauri/                # 后端 (Rust)
│   ├── src/
│   │   ├── main.rs           # CLI 路由（gui / proxy stdio / proxy http）
│   │   ├── lib.rs            # Tauri Builder 装配
│   │   ├── cli.rs            # 命令行参数解析
│   │   ├── state.rs          # 全局 AppState
│   │   ├── cmd_*.rs          # Tauri command（每个域一个文件）
│   │   ├── claude_legacy.rs  # Claude Desktop 集成
│   │   ├── proxy/
│   │   │   ├── mod.rs        # 子 server 注册表 + 懒启动
│   │   │   ├── stdio_proxy.rs# stdio 代理主循环
│   │   │   └── routing.rs
│   │   └── storage/          # servers.json + skills/*.md + settings.json
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── Cargo.toml
├── icons/                    # 应用图标 (已生成)
├── docs/PROJECT_PLAN.md
└── package.json
```

## 💾 数据存储

| 路径                                                          | 内容                  |
| ------------------------------------------------------------- | --------------------- |
| `%LOCALAPPDATA%\ms-manage\servers.json`                       | MCP server 配置       |
| `%LOCALAPPDATA%\ms-manage\skills\*.md`                        | Skill Markdown 文件   |
| `%LOCALAPPDATA%\ms-manage\settings.json`                      | 应用设置              |
| `%LOCALAPPDATA%\ms-manage\claude-backups\*.bak.json`         | Claude Desktop 配置备份 |

## 🚀 开发

```bash
# 一次性：装依赖（国内镜像，已配 .npmrc）
npm install

# 开发模式（热重载）
npm run tauri:dev

# 仅前端开发（不开 Rust 窗口）
npm run dev

# 类型检查 + 生产前端构建
npm run build
```

## 📦 构建 Windows 安装包

```bash
# NSIS .exe 安装包（产物在 src-tauri/target/release/bundle/nsis/）
npm run tauri:build

# 输出位置：
#   src-tauri/target/release/ms_manage.exe          # 主程序（约 7MB）
#   src-tauri/target/release/bundle/nsis/AI Manage_0.1.0_x64-setup.exe
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

## 📝 License

待定（建议 MIT）。
