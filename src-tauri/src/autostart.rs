//! 开机自启辅助。
//!
//! 主逻辑用官方插件 `tauri-plugin-autostart`（Windows 底层写注册表 Run 键，
//! 见 auto-launch crate 的 windows.rs），本模块只做两件补充：
//!   1. `fix_registry_target` —— 把 Run 键里记录的 exe 路径纠正为 release 构建。
//!      原因：debug 构建的 WebView 固定加载 `tauri.conf.json` 里的 `devUrl`
//!      （http://127.0.0.1:1520，Vite dev 服务器），自启时 Vite 没跑，
//!      整个窗口就会 ERR_CONNECTION_REFUSED。release 构建内嵌前端，不依赖任何 dev 服务器。
//!   2. `remove_legacy_shortcut` —— 清理旧版本写在 Startup 文件夹的 .lnk，
//!      避免「注册表 + 快捷方式」双重启动。

use std::path::PathBuf;

const RUN_REGKEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
/// 插件用 `package_info().name`（tauri.conf.json 的 productName）作为注册表值名。
const RUN_VALUE_NAME: &str = "MS Manage";
const LEGACY_LNK_NAME: &str = "MS Manage.lnk";
const STARTUP_ARGS: &str = "--minimized";

/// 自启动应该指向的 exe：
/// 优先同目录旁的 release 构建（内嵌前端、不依赖 Vite），否则退回当前 exe。
fn preferred_exe_path() -> PathBuf {
    let current = std::env::current_exe().unwrap_or_default();
    let file_name = match current.file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => return current,
    };
    if let Some(parent) = current.parent() {
        let release = parent.join("..").join("release").join(&file_name);
        if release.exists() {
            return release;
        }
    }
    current
}

/// 把 Run 键里记录的 exe 纠正为 preferred_exe_path()。
/// 插件 `enable()` 用的是 `current_exe()`；如果用户在 debug 构建里开了自启，
/// 它会把 debug exe 写进注册表，自启时就会去连 Vite 报 ERR_CONNECTION_REFUSED —— 这里兜底改回来。
pub fn fix_registry_target() {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
        use winreg::RegKey;
        let exe_path = preferred_exe_path();
        let Some(exe) = exe_path.to_str() else { return };
        let value = format!("{exe} {STARTUP_ARGS}");
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(run) = hkcu.open_subkey_with_flags(RUN_REGKEY, KEY_SET_VALUE) else { return };
        let _ = run.set_value(RUN_VALUE_NAME, &value);
    }
}

/// 删除旧版本创建在 Startup 文件夹的快捷方式（如果存在），防止双重启动。
pub fn remove_legacy_shortcut() {
    let Some(lnk) = legacy_lnk_path() else { return };
    let _ = std::fs::remove_file(lnk);
}

fn legacy_lnk_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| {
        d.join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup")
            .join(LEGACY_LNK_NAME)
    })
}
