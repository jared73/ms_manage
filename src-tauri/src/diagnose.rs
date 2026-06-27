//! 环境诊断：检查 PATH 里的常用 MCP server 工具是否存在

use std::path::PathBuf;

pub fn diagnose() -> Vec<DiagnosticResult> {
    let checks = [
        ("node", "Node.js 运行时（npx 需要）"),
        ("npm", "npm 包管理器"),
        ("npx", "npx 命令执行器"),
        ("pnpm", "pnpm（可选，更快）"),
        ("uv", "uv（Python 包管理器，运行 mcp-server-fetch 等）"),
        ("uvx", "uvx（Python 工具直跑）"),
        ("python", "Python 3"),
        ("docker", "Docker（运行 container MCP server）"),
        ("git", "Git（部分 server 需要 clone）"),
    ];
    checks.iter().map(|(cmd, desc)| {
        DiagnosticResult {
            command: cmd.to_string(),
            description: desc.to_string(),
            found: which(cmd).is_some(),
            path: which(cmd),
        }
    }).collect()
}

pub fn which(cmd: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';').map(|s| s.to_string()).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(cmd);
        if candidate.is_file() { return Some(candidate); }
        if cfg!(windows) {
            for ext in &exts {
                let with_ext = dir.join(format!("{cmd}{ext}"));
                if with_ext.is_file() { return Some(with_ext); }
            }
        }
    }
    None
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnosticResult {
    pub command: String,
    pub description: String,
    pub found: bool,
    pub path: Option<PathBuf>,
}

/// 给 spawn 错误加友好提示
pub fn friendly_spawn_error(cmd: &str, err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            let hint = match cmd {
                "npx" | "npm" | "node" | "pnpm" => {
                    "Node.js 未安装或不在 PATH 中。请到 https://nodejs.org/ 下载安装 Node.js 18+"
                }
                "uv" | "uvx" => {
                    "uv 未安装。请运行 `pip install uv` 或到 https://github.com/astral-sh/uv 安装"
                }
                "python" | "python3" => {
                    "Python 3 未安装。请到 https://www.python.org/ 下载安装"
                }
                "docker" => "Docker 未安装或不在 PATH 中",
                _ => "可执行文件未找到，请检查 PATH 或确认命令路径是否正确",
            };
            format!("找不到命令 `{}`：{}", cmd, hint)
        }
        std::io::ErrorKind::PermissionDenied => {
            format!("权限被拒绝 `{}`：请检查文件是否有执行权限", cmd)
        }
        _ => format!("启动 `{}` 失败：{}", cmd, err),
    }
}
