//! Windows 开机自启 —— 在 Startup 文件夹创建/删除 .lnk 快捷方式
//!
//! 不依赖注册表（某些环境写 HKEY_CURRENT_USER\Run 会被组策略/安全软件拒绝）。
//! 通过 PowerShell 的 WScript.Shell 创建快捷方式，避免直接依赖 COM 接口定义。
//! Windows 启动文件夹路径：
//!   %APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup

use std::path::PathBuf;

const APP_NAME: &str = "MS Manage";

fn get_exe_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn startup_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("Startup"))
}

fn lnk_path() -> Option<PathBuf> {
    startup_dir().map(|d| d.join(format!("{}.lnk", APP_NAME)))
}

/// 返回开机自启是否已启用（Startup 文件夹里存在 MS Manage.lnk）
pub fn is_enabled() -> bool {
    lnk_path().map(|p| p.exists()).unwrap_or(false)
}

/// 开启开机自启：在 Startup 文件夹创建指向当前 exe 的快捷方式，带 --minimized
pub fn enable() -> Result<(), String> {
    let exe = get_exe_path().ok_or_else(|| "无法获取当前程序路径".to_string())?;
    let lnk = lnk_path().ok_or_else(|| "无法获取启动文件夹路径".to_string())?;

    if let Some(parent) = lnk.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建启动文件夹失败: {e}"))?;
    }

    let exe_str = exe.to_string_lossy().to_string();
    let lnk_str = lnk.to_string_lossy().to_string();

    // 用 PowerShell 创建快捷方式，避免 rust 里绑定 COM 接口
    let ps = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $s = $ws.CreateShortcut('{}'); \
         $s.TargetPath = '{}'; \
         $s.Arguments = '--minimized'; \
         $s.Description = 'MS Manage 开机自启'; \
         $s.Save()",
        escape_ps_single(&lnk_str),
        escape_ps_single(&exe_str),
    );

    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps])
        .output()
        .map_err(|e| format!("启动 PowerShell 失败: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建快捷方式失败: {stderr}"));
    }

    if !lnk.exists() {
        return Err("快捷方式未生成".to_string());
    }

    Ok(())
}

/// 关闭开机自启：删除 Startup 文件夹里的 MS Manage.lnk
pub fn disable() -> Result<(), String> {
    let lnk = lnk_path().ok_or_else(|| "无法获取启动文件夹路径".to_string())?;
    if lnk.exists() {
        std::fs::remove_file(&lnk).map_err(|e| format!("删除快捷方式失败: {e}"))?;
    }
    Ok(())
}

fn escape_ps_single(s: &str) -> String {
    // PowerShell 单引号字符串里，两个单引号表示一个单引号
    s.replace("'", "''")
}
