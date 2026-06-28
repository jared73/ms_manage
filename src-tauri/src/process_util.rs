//! Windows 进程创建辅助
//!
//! 关键问题：`.cmd/.bat/.ps1` 文件被 cmd.exe 解释时会 allocate console 闪窗。
//! 解决：读 `.cmd` 文件内容，找出它真正调用的 `.exe`（如 `node.exe npx-cli.js`），
//! 直接 spawn 那个 `.exe`，绕过 cmd.exe。

use std::path::PathBuf;
use tokio::process::Command;

/// 创建进程时用的无窗口标志
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
pub fn apply_no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
pub fn apply_no_window(_cmd: &mut Command) {}

/// PATH 解析：`npx` → `C:\...\npx.cmd`
pub fn resolve_cmd(name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() || p.extension().is_some() { return p; }
    #[cfg(windows)]
    {
        let suffixes = ["exe", "cmd", "bat", "ps1", ""];
        if let Some(path_var) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_var) {
                for ext in &suffixes {
                    let candidate = if ext.is_empty() { dir.join(name) } else { dir.join(format!("{name}.{ext}")) };
                    if let Ok(meta) = candidate.metadata() {
                        if meta.is_file() && (meta.len() >= 4096 || (*ext != "" && *ext != "ps1")) {
                            return candidate;
                        }
                    }
                }
            }
        }
    }
    p
}

/// .cmd/.bat 文件解析：读内容找出真实 .exe 路径 + 脚本路径
/// 返回 (真实 exe 路径, 传给 exe 的参数)
/// 如果解析失败，返回 None（fallback 到 cmd.exe /c 包装）
///
/// 算法：从最后一行往回扫，找含 `%*` 的真正调用行（如 `"%NODE_EXE%" "%NPX_CLI_JS%" %*`），
/// 跳过 FOR /F 块内部的 SET 行。如果找不到含 `%*` 的行，fallback 到最后一条含 .js 的非控制流行。
#[cfg(windows)]
pub fn parse_cmd_file(cmd_path: &PathBuf) -> Option<(PathBuf, Vec<String>)> {
    let content = std::fs::read_to_string(cmd_path).ok()?;
    let cmd_dir = cmd_path.parent()?;

    // node.exe 在 npx.cmd 同级目录
    let node_exe = cmd_dir.join("node.exe");
    if !node_exe.exists() { return None; }

    let lines: Vec<&str> = content.lines().collect();

    // 策略：优先找含 %* 的行（真正的调用行），其次找最后一条含 .js 的非控制流行
    let mut best_script: Option<String> = None;

    for line in lines.iter().rev() {
        let trimmed = line.trim_start();
        // 跳过空行、注释、控制流关键字（大小写不敏感）
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.is_empty()
            || trimmed.starts_with('@')
            || lower.starts_with("if ")
            || lower.starts_with("for ")
            || lower.starts_with("set ")
            || lower.starts_with("echo ")
            || lower.starts_with("endlocal")
            || lower.starts_with("local")
            || lower.starts_with("goto ")
            || lower.starts_with("call ")
        {
            continue;
        }

        // 找 .js 路径（含 node_modules 或 %dp0%）
        let mut script_path: Option<String> = None;
        for token in trimmed.split_whitespace() {
            let t = token.trim_matches('"').trim_matches('\'');
            if t.ends_with(".js") && (t.contains("node_modules") || t.contains("%dp0%")) {
                let resolved = t.replace("%dp0%", &cmd_dir.to_string_lossy())
                    .replace('\\', "/");
                script_path = Some(resolved);
            }
        }

        if let Some(sp) = script_path {
            // 含 %* 的行是真正的调用行，直接用
            if trimmed.contains("%*") {
                return Some((node_exe, vec![sp]));
            }
            // 否则暂存，继续往前找更好的
            if best_script.is_none() {
                best_script = Some(sp);
            }
        }
    }

    let script = best_script?;
    Some((node_exe, vec![script]))
}

/// uvx 缓存路径（绕过 uvx 直接 spawn 缓存的 .exe）
#[cfg(windows)]
pub fn resolve_uvx_cached_exe(package: &str) -> Option<PathBuf> {
    let cache_root = dirs::data_local_dir()?.join("uv").join("cache").join("archive-v0");
    if !cache_root.is_dir() { return None; }
    let Ok(entries) = std::fs::read_dir(&cache_root) else { return None; };
    let target = format!("{package}.exe");
    for e in entries.flatten() {
        let exe = e.path().join("Scripts").join(&target);
        if exe.is_file() { return Some(exe); }
    }
    None
}

/// 把命令 + 参数转成可直接 spawn 的形式
/// - uvx <pkg> → 绕过 uvx，用缓存的 .exe
/// - npx <pkg> → 直接找 npm 缓存里的 node.exe + npx-cli.js
/// - .exe      → 原样
pub fn resolve_spawn_command(cmd: &PathBuf, args: &[String]) -> (PathBuf, Vec<String>) {
    // uvx 绕过
    if let Some(name) = cmd.file_stem().and_then(|s| s.to_str()) {
        if name.eq_ignore_ascii_case("uvx") && !args.is_empty() {
            if let Some(cached) = resolve_uvx_cached_exe(&args[0]) {
                let mut new_args = Vec::with_capacity(args.len() - 1);
                new_args.extend(args[1..].iter().cloned());
                return (cached, new_args);
            }
        }
    }
    // npx / npm .cmd 文件解析
    if cmd.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).map_or(false, |e| e == "cmd" || e == "bat") {
        if let Some((exe, mut exe_args)) = parse_cmd_file(cmd) {
            exe_args.extend(args.iter().cloned());
            return (exe, exe_args);
        }
    }
    (cmd.clone(), args.to_vec())
}