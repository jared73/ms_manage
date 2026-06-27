//! Windows 进程创建辅助
//!
//! 关键：所有启动 stdio 子进程的 Command 都必须设 `CREATE_NO_WINDOW` (0x08000000)
//! 否则 npx / node / python 会在任务栏和桌面闪一个黑色 cmd 窗口。
//!
//! Windows 还有个经典坑：Rust `Command::new("npx")` 只找 `npx.exe`，
//! 找不到 Node.js 安装的 `npx.cmd` / `npx.ps1`。
//! 用 `resolve_cmd()` 在 spawn 前通过 `where` 命令解析到绝对路径再 spawn。

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
pub fn apply_no_window(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn apply_no_window(_cmd: &mut tokio::process::Command) {}

/// 在 spawn 前解析 `npx` / `node` 这种不带扩展名的命令到绝对路径。
///
/// Windows 上 Node.js 安装通常建 `npx.cmd` 而不是 `npx.exe`，
/// Rust CreateProcess 不会自动补 PATHEXT，导致 spawn 失败。
/// 而且 PATH 里常有 Git for Windows 的 2KB 占位文件，名字也叫 `npx`，
/// `where npx` 优先返回它，spawn 后报「不是有效的 Win32 应用程序」。
///
/// 所以：自己遍历 PATH，按优先级 (`.exe` > `.cmd` > `.bat` > 无扩展名) 找，
/// 跳过小于 4 KB 的可疑文件（Git 占位通常只有 2 KB）。
pub fn resolve_cmd(name: &str) -> std::path::PathBuf {
    use std::path::PathBuf;
    let p = PathBuf::from(name);
    if p.is_absolute() { return p; }
    if p.extension().is_some() { return p; }

    #[cfg(windows)]
    {
        // 优先级：.exe > .cmd > .bat > .ps1 > 无扩展名
        let suffixes = ["exe", "cmd", "bat", "ps1", ""];
        let path_var = match std::env::var_os("PATH") {
            Some(v) => v, None => return p,
        };
        for dir in std::env::split_paths(&path_var) {
            for ext in &suffixes {
                let candidate = if ext.is_empty() {
                    dir.join(name)
                } else {
                    dir.join(format!("{name}.{ext}"))
                };
                if let Ok(meta) = candidate.metadata() {
                    if meta.is_file() {
                        // 跳过可疑的占位文件（< 4 KB 且没有 .exe / .cmd 扩展名）
                        if meta.len() < 4096 && (*ext == "" || *ext == "ps1") {
                            continue;
                        }
                        return candidate;
                    }
                }
            }
        }
    }
    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("which").arg(name).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).to_string();
                if let Some(first) = s.lines().next() {
                    let first = first.trim();
                    if !first.is_empty() { return PathBuf::from(first); }
                }
            }
        }
    }
    p
}
