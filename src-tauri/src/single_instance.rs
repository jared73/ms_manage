//! 单实例守卫：启动 GUI 时把已经跑着的旧实例干掉。
//!
//! 思路很简单：
//!   - 在 data_dir 放一个 PID 文件，里头是上次启动的进程号。
//!   - 每次 GUI 启动先读这个文件：
//!     1. 文件不存在 → 我们是第一个
//!     2. 文件存在 + PID 解析失败 → 旧文件脏了，清掉重来
//!     3. 文件存在 + PID 就是自己 → 重入了，啥也不做
//!     4. 文件存在 + PID 是别人 → 用平台命令 taskkill / kill -9 干掉它
//!   - 然后把自己的 PID 写进去。
//!
//! 只对 GUI 入口生效，proxy / serve / connect / diagnose 这些 CLI 子命令
//! 不受这个保护 —— 它们本来就应该允许多实例。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn pid_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ms-manage.pid")
}

/// 杀掉旧实例（如果有）。返回 true 表示确实杀了一个进程。
pub fn kill_previous_instance(data_dir: &Path) -> bool {
    let path = pid_file_path(data_dir);
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return false, // 没文件 = 没旧实例
    };
    let pid: u32 = match content.trim().parse() {
        Ok(n) => n,
        Err(_) => {
            // 文件脏了（崩溃/被改），清掉重来
            let _ = fs::remove_file(&path);
            return false;
        }
    };
    let my_pid = std::process::id();
    if pid == my_pid {
        // 同进程重入（理论上不该发生），啥也不做
        return false;
    }

    let killed = try_kill(pid);
    if killed {
        // 给旧进程一点点时间彻底退出（释放端口、文件锁等）
        std::thread::sleep(std::time::Duration::from_millis(300));
        // 保险起见再杀一次 —— 万一旧进程还在收尾
        let _ = try_kill(pid);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // 不管杀没杀掉都清掉文件，避免下次启动读到一个死 PID
    let _ = fs::remove_file(&path);
    killed
}

/// 把当前进程的 PID 写进去，给下次启动看。
pub fn write_pid_file(data_dir: &Path) {
    let path = pid_file_path(data_dir);
    // 父目录由 AppState::initialize() 保证了，这里只管写文件
    let _ = fs::write(&path, std::process::id().to_string());
}

#[cfg(target_os = "windows")]
fn try_kill(pid: u32) -> bool {
    // taskkill /F /PID <pid> —— 强制结束进程
    // 退出码 0 = 杀成功；128 = 找不到进程
    Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn try_kill(pid: u32) -> bool {
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}