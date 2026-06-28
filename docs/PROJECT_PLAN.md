# MS Manage �?项目计划

## 定位
类似 mcp-router �?MCP server & skill 管理工具。Windows 桌面应用，中文优先�?本工具作为本地代理，�?Claude Desktop 通过单一 endpoint 连接所有受管的 MCP server�?
## 技术栈
- 桌面框架: Tauri 2 (Rust + WebView)
- 前端: React 18 + TypeScript + Vite + Tailwind CSS + shadcn/ui 风格
- 后端: Rust + tokio + serde_json
- 状�? Zustand (前端) + tokio::sync (后端)
- 国际�? react-i18next (�?�?
- 编辑�? Monaco Editor (skill markdown 编辑)
- 打包: Tauri bundler (NSIS .exe + .zip)

## 功能模块
1. **MCP Server 管理** �?stdio / SSE / HTTP 三种 transport，添�?编辑/启动/停止/测试/日志
2. **Skill 管理** �?Markdown + YAML frontmatter，新�?编辑/删除/启用/禁用
3. **Claude Desktop 集成** �?导入/同步 claude_desktop_config.json，一键配置代理模�?4. **在线市场** �?浏览/安装社区 MCP server �?skill
5. **本地代理** �?Tauri 后端暴露 stdio �?HTTP endpoint �?Claude Desktop，转发到多个 MCP server
6. **中文�?* �?�?UI 中文优先，可切换英文

## 目录结构 (规划)
```
ms_manage/
├── src/                      # 前端 (React)
�?  ├── components/
�?  ├── pages/
�?  ├── stores/
�?  ├── i18n/
�?  └── lib/
├── src-tauri/                # 后端 (Rust)
�?  ├── src/
�?  �?  ├── main.rs
�?  �?  ├── mcp/              # MCP 协议代理
�?  �?  ├── server/           # server 生命周期
�?  �?  ├── skill/            # skill 文件系统
�?  �?  ├── claude/           # Claude Desktop 集成
�?  �?  └── market/           # 市场
�?  ├── tauri.conf.json
�?  └── Cargo.toml
├── package.json
└── vite.config.ts
```

## 数据存储
- `~/\.ms-manage/servers.json` �?MCP server 配置
- `~/\.ms-manage/skills/` �?skill Markdown 文件
- `~/\.ms-manage/logs/` �?调用日志
- `~/\.ms-manage/settings.json` �?应用设置

## 分发
- `ms-manage_<ver>_x64-setup.exe` �?NSIS 安装�?- `ms-manage_<ver>_x64.zip` �?绿色�?(解压即用)
