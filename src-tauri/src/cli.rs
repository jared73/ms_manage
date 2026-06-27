//! CLI 子命令路由：解析 `ms_manage <subcommand> [args]`，分发到对应功能。
//! 子命令：
//! - `proxy stdio` —— 启动 stdio MCP 代理（无 GUI）//! - `proxy http --port N` —— 启动 HTTP MCP 代理（无 GUI）//! - 无参 —— 启动 GUI 应用

use std::env;

pub enum Command {
    Gui,
    ProxyStdio,
    ProxyHttp { port: u16 },
    Serve,
    Connect { target: Option<String> },
    MarketInstall { id: String },
    SkillsList,
    SkillsReconcile,
    ProjectsList,
    Diagnose,
    ExportMcp { path: String, mode: String },
    Help,
}

pub fn parse() -> Command {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { return Command::Gui; }
    match args[1].as_str() {
        "proxy" => {
            if args.len() < 3 { return Command::Help; }
            match args[2].as_str() {
                "stdio" => Command::ProxyStdio,
                "http" => {
                    let mut port: u16 = 39231;
                    let mut i = 3;
                    while i < args.len() {
                        if args[i] == "--port" && i + 1 < args.len() {
                            port = args[i+1].parse().unwrap_or(39231);
                            i += 2;
                        } else { i += 1; }
                    }
                    Command::ProxyHttp { port }
                }
                _ => Command::Help,
            }
        }
        "connect" => {
            // ms_manage connect [claude-code | cursor | cline | windsurf | vscode | print]
            let target = args.get(2).cloned();
            Command::Connect { target }
        }
        "market-install" => {
            let id = args.get(2).cloned().unwrap_or_default();
            Command::MarketInstall { id }
        }
        "serve" => Command::Serve,
        "skills" => {
            match args.get(2).map(|s| s.as_str()).unwrap_or("") {
                "list"       => Command::SkillsList,
                "reconcile"  => Command::SkillsReconcile,
                _ => Command::Help,
            }
        }
        "projects" => {
            match args.get(2).map(|s| s.as_str()).unwrap_or("") {
                "list"       => Command::ProjectsList,
                _ => Command::Help,
            }
        }
        "diagnose" => Command::Diagnose,
        "export-mcp" => {
            // ms_manage export-mcp <path> [proxy|individual]
            let path = args.get(2).cloned().unwrap_or_else(|| "mcp_servers.json".to_string());
            let mode = args.get(3).cloned().unwrap_or_else(|| "proxy".to_string());
            Command::ExportMcp { path, mode }
        }
        "help" | "--help" | "-h" => Command::Help,
        _ => Command::Gui,
    }
}

pub fn print_help() {
    println!("MS Manage {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage:");
    println!("  ms_manage                                  启动 GUI 应用");
    println!("  ms_manage serve                             启动 stdio MCP server（给任意 client 当 stdio 子进程用）;");
    println!("  ms_manage proxy stdio                      等同 serve");
    println!("  ms_manage proxy http [--port N]            启动 HTTP MCP 代理（带 MCPR_TOKEN 鉴权）;");
    println!("  ms_manage connect [target]                 一键接入各种 AI 客户端（学 mcp-router）;");
    println!("                                               target: claude-code | cursor | cline | windsurf | vscode | qoder | print");
    println!("  ms_manage market-install <id>              安装市场 item");
    println!("  ms_manage skills list                      列出所有 skill");
    println!("  ms_manage skills reconcile                 手动触发 symlink 同步");
    println!("  ms_manage projects list                    列出所有 project");
    println!("  ms_manage diagnose                         检查 Node.js / uv / Python 等依赖是否在 PATH");
    println!("  ms_manage export-mcp <path> [mode]         导出 MCP 配置到任意路径（proxy|individual）;")
}

pub fn skills_list() {
    use crate::state::AppState;
    let state = match AppState::initialize() {
        Ok(s) => s,
        Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
    };
    match state.storage.load_skills() {
        Ok(list) => {
            if list.is_empty() {
                println!("(no skills)");
                return;
            }
            for s in &list {
                let marker = if s.enabled { "[x]" } else { "[ ]" };
                let desc = s.description.clone().unwrap_or_default();
                println!("{} {}  ({})", marker, s.name, desc);
                println!("    dir: {}", s.dir_path);
            }
        }
        Err(e) => eprintln!("load skills: {e}"),
    }
}

pub fn skills_reconcile() {
    use crate::state::AppState;
    let state = match AppState::initialize() {
        Ok(s) => s,
        Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
    };
    match crate::storage::skills::reconcile_after_change(&state.storage) {
        Ok(()) => println!("reconciled."),
        Err(e) => eprintln!("reconcile failed: {e}"),
    }
}

pub fn projects_list() {
    use crate::state::AppState;
    let state = match AppState::initialize() {
        Ok(s) => s,
        Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
    };
    match state.storage.load_projects() {
        Ok(list) => {
            println!("[__unassigned__]  (virtual, all servers without project)");
            if list.is_empty() {
                println!("(no projects)");
                return;
            }
            for p in &list {
                let desc = p.description.clone().unwrap_or_default();
                println!("[{}] {}  {}", p.id, p.name, desc);
            }
        }
        Err(e) => eprintln!("load projects: {e}"),
    }
}

/// `ms_manage connect [target]` —— 一键配置 AI 客户端
/// target 可选：
///   claude-code | cursor | cline | windsurf | vscode | print（默认）
///   留空则打印所有可用目标。
/// 与 mcp-router 的 connect 命令对齐。token 自动从 settings.json 读取。
pub fn connect(target: Option<String>) {
    use crate::state::AppState;
    let state = match AppState::initialize() {
        Ok(s) => s,
        Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
    };
    let settings = match state.storage.load_settings() {
        Ok(s) => s,
        Err(e) => { eprintln!("load settings: {e}"); std::process::exit(1); }
    };
    let token = settings.mcp_token.clone().unwrap_or_default();
    let port = settings.proxy_http_port;
    let host = "127.0.0.1";
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "ms_manage.exe".into());

    let target = target.unwrap_or_else(|| "print".into());
    match target.as_str() {
        "print" | "" => {
            println!("==============================================");
            println!("MS Manage MCP 接入指引");
            println!("==============================================");
            println!("HTTP endpoint:  http://{host}:{port}/mcp");
            println!("SSE endpoint:   http://{host}:{port}/mcp/sse");
            println!("Auth token:     {token}");
            println!();
            println!("可选 client (全部用 stdio 模式 spawn ms_manage.exe serve):");
            println!("  ms_manage connect claude-code   写入 ~/.claude/mcp_servers.json (Claude Code CLI)");
            println!("  ms_manage connect cursor        写入 ~/.cursor/mcp.json (Cursor)");
            println!("  ms_manage connect cline         写入 Cline 配置");
            println!("  ms_manage connect windsurf      写入 ~/.codeium/windsurf/mcp_config.json (Windsurf)");
            println!("  ms_manage connect vscode        输出 VS Code .vscode/mcp.json 片段");
            println!("  ms_manage connect qoder         写入 %APPDATA%\\Qoder\\SharedClientCache\\mcp.json (Qoder IDE)");
            println!("  ms_manage connect claude-desktop 写入 %APPDATA%\\Claude\\claude_desktop_config.json (Claude Desktop)");
        }
        "claude-code" => {
            // stdio 模式：Claude Code spawn ms_manage.exe serve
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ms_manage.exe".into());
            let cfg = serde_json::json!({
                "mcpServers": {
                    "ms-manage": {
                        "command": exe,
                        "args": ["serve"],
                        "env": { "MCPR_TOKEN": token }
                    }
                }
            });
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let p = home.join(".claude").join("mcp_servers.json");
            if let Err(e) = write_json(&p, &cfg) { eprintln!("write failed: {e}"); std::process::exit(1); }
            println!("。Wrote {} (stdio 模式)", p.display());
        }
        "cursor" => {
            // stdio 模式
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ms_manage.exe".into());
            let cfg = serde_json::json!({
                "mcpServers": {
                    "ms-manage": {
                        "command": exe,
                        "args": ["serve"],
                        "env": { "MCPR_TOKEN": token }
                    }
                }
            });
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let p = home.join(".cursor").join("mcp.json");
            if let Err(e) = write_json(&p, &cfg) { eprintln!("write failed: {e}"); std::process::exit(1); }
            println!("。Wrote {} (stdio 模式)", p.display());
        }
        "cline" => {
            // stdio 模式
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ms_manage.exe".into());
            let cfg = serde_json::json!({
                "mcpServers": {
                    "ms-manage": {
                        "command": exe,
                        "args": ["serve"],
                        "env": { "MCPR_TOKEN": token }
                    }
                }
            });
            println!("Cline 配置（stdio 模式，写。cline_mcp_settings.json）：");
            println!("{}", serde_json::to_string_pretty(&cfg).unwrap_or_default());
        }
        "windsurf" => {
            // stdio 模式
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ms_manage.exe".into());
            let cfg = serde_json::json!({
                "mcpServers": {
                    "ms-manage": {
                        "command": exe,
                        "args": ["serve"],
                        "env": { "MCPR_TOKEN": token }
                    }
                }
            });
            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let p = home.join(".codeium").join("windsurf").join("mcp_config.json");
            if let Err(e) = write_json(&p, &cfg) { eprintln!("write failed: {e}"); std::process::exit(1); }
            println!("。Wrote {} (stdio 模式)", p.display());
        }
        "vscode" => {
            // stdio 模式
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ms_manage.exe".into());
            let cfg = serde_json::json!({
                "servers": {
                    "ms-manage": {
                        "type": "stdio",
                        "command": exe,
                        "args": ["serve"],
                        "env": { "MCPR_TOKEN": token }
                    }
                }
            });
            println!("VS Code 配置（stdio 模式，写。.vscode/mcp.json）：");
            println!("{}", serde_json::to_string_pretty(&cfg).unwrap_or_default());
        }
        "claude-desktop" => {
            eprintln!("claude-desktop target no longer supported (Claude Desktop integration removed)");
            std::process::exit(2);
            // 注释保留：原 claude_legacy 写文件路。            // let storage = &state.storage;
            // if let Err(e) = crate::claude_legacy::export(storage, "proxy") {
            //     eprintln!("export failed: {e}");
            //     std::process::exit(1);
            // }
            // println!("。已写。Claude Desktop 配置（代理模式）");
        }
        "qoder" => {
            // Qoder 。%APPDATA%\Qoder\SharedClientCache\mcp.json
            let appdata = std::env::var("APPDATA").unwrap_or_default();
            let p = std::path::PathBuf::from(&appdata)
                .join("Qoder").join("SharedClientCache").join("mcp.json");
            if let Some(parent) = p.parent() { std::fs::create_dir_all(parent).ok(); }
            // 。mcp-router：stdio 模式（最稳，所。client 都支持）
            // command: ms_manage.exe 绝对路径
            // args: ["serve"] —。stdio MCP server 模式
            // env: { MCPR_TOKEN: ... } —。鉴权
            let exe = std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "ms_manage.exe".into());
            let cfg = serde_json::json!({
                "mcpServers": {
                    "ms-manage": {
                        "command": exe,
                        "args": ["serve"],
                        "env": { "MCPR_TOKEN": token }
                    }
                }
            });
            if let Err(e) = write_json(&p, &cfg) { eprintln!("write failed: {e}"); std::process::exit(1); }
            println!("。Wrote {} (stdio 模式)", p.display());
        }
        other => {
            eprintln!("unknown target: {other}\nRun `ms_manage connect` to see available targets.");
            std::process::exit(2);
        }
    }
    // 帮助提示：先启动 HTTP 代理
    let _ = exe; // suppress unused warning
    eprintln!();
    eprintln!("提示：先启动代理（ms_manage proxy http），再启动目标客户端。");
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let s = serde_json::to_string_pretty(value).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, s)
}

/// `ms_manage diagnose` —— 检查常见 MCP server 依赖是否可用
pub fn diagnose() {
    println!("========== 环境诊断 ==========");
    let results = crate::diagnose::diagnose();
    let mut missing_critical = 0;
    for r in &results {
        let marker = if r.found { "OK  " } else { "缺  " };
        let path_str = r.path.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(未找到)".to_string());
        println!("  [{}] {:<10} {:<40} {}", marker, r.command, r.description, path_str);
        if !r.found && matches!(r.command.as_str(), "node" | "npx" | "python" | "uv") {
            missing_critical += 1;
        }
    }
    println!();
    if missing_critical > 0 {
        println!("⚠ 缺失关键依赖。安装后 MCP server 才能正常启动。");
    } else {
        println!("✓ 关键依赖齐全。");
    }
}

/// `ms_manage export-mcp <path> [proxy|individual]`
///
/// 把当前所有 enabled server 导出到一个 mcpServers 配置 JSON。
/// 任意支持 mcpServers 字段的客户端（Claude Code / ccswitch / Cursor / Cline / Windsurf 等）
/// 都可以直接 import 这个文件，或者把它复制到对应客户端的配置文件路径。
pub fn export_mcp(path: String, mode: String) {
    use crate::state::AppState;
    let state = match AppState::initialize() {
        Ok(s) => s,
        Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
    };
    let settings = match state.storage.load_settings() {
        Ok(s) => s,
        Err(e) => { eprintln!("load settings: {e}"); std::process::exit(1); }
    };
    let servers = match state.storage.load_servers() {
        Ok(s) => s,
        Err(e) => { eprintln!("load servers: {e}"); std::process::exit(1); }
    };

    let mut mcp = serde_json::Map::new();
    match mode.as_str() {
        "proxy" => {
            // 单 entry 指向 ms-manage HTTP 代理
            let host = "127.0.0.1";
            let port = settings.proxy_http_port;
            let token = settings.mcp_token.clone().unwrap_or_default();
            mcp.insert("ms-manage".into(), serde_json::json!({
                "type": "http",
                "url": format!("http://{host}:{port}/mcp"),
                "headers": { "Authorization": format!("Bearer {token}") }
            }));
        }
        "individual" | _ => {
            for s in servers.iter().filter(|s| s.enabled && s.transport == "stdio") {
                let cmd = s.command.clone().unwrap_or_default();
                if cmd.is_empty() { continue; }
                let entry = serde_json::json!({
                    "command": cmd,
                    "args": s.args,
                    "env": s.env,
                });
                mcp.insert(s.name.clone(), entry);
            }
        }
    }
    let body = serde_json::json!({ "mcpServers": serde_json::Value::Object(mcp) });
    let p = std::path::PathBuf::from(&path);
    if let Err(e) = write_json(&p, &body) {
        eprintln!("write {path}: {e}");
        std::process::exit(1);
    }
    println!("。exported to {}", p.display());
}
