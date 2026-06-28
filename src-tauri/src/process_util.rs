//! Windows 进程创建辅助

/// 禁止分配控制台窗口，仅此一个 flag。
/// DETACHED_PROCESS / CREATE_NEW_PROCESS_GROUP 不加 —— 它们会破坏 .cmd 的继承链。
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
pub fn apply_no_window(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn apply_no_window(_cmd: &mut tokio::process::Command) {}

use std::path::PathBuf;

/// PATH 解析：`npx` → `C:\...\npx.cmd`
pub fn resolve_cmd(name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() { return p; }
    if p.extension().is_some() { return p; }
    #[cfg(windows)]
    {
        let suffixes = ["exe", "cmd", "bat", "ps1", ""];
        let path_var = match std::env::var_os("PATH") {
            Some(v) => v, None => return p,
        };
        for dir in std::env::split_paths(&path_var) {
            for ext in &suffixes {
                let candidate = if ext.is_empty() { dir.join(name) } else { dir.join(format!("{name}.{ext}")) };
                if let Ok(meta) = candidate.metadata() {
                    if meta.is_file() {
                        if meta.len() < 4096 && (*ext == "" || *ext == "ps1") { continue; }
                        return candidate;
                    }
                }
            }
        }
    }
    p
}

/// uvx 缓存绕过
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

/// 把 uvx 类型的命令绕过到缓存的真 exe
#[cfg(windows)]
pub fn resolve_spawn_command(cmd: &PathBuf, args: &[String]) -> (PathBuf, Vec<String>) {
    if let Some(name) = cmd.file_stem().and_then(|s| s.to_str()) {
        if name.eq_ignore_ascii_case("uvx") && !args.is_empty() {
            if let Some(cached) = resolve_uvx_cached_exe(&args[0]) {
                let mut new_args = Vec::with_capacity(args.len() - 1);
                new_args.extend(args[1..].iter().cloned());
                return (cached, new_args);
            }
        }
    }
    (cmd.clone(), args.to_vec())
}

/// .cmd/.bat/.ps1 文件需要特殊处理
#[cfg(windows)]
pub fn is_batch_script(path: &PathBuf) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(ext.to_ascii_lowercase().as_str(), "cmd" | "bat" | "ps1")
    } else {
        false
    }
}
#[cfg(not(windows))]
pub fn is_batch_script(_path: &PathBuf) -> bool { false }