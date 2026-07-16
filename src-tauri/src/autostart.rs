//! Windows 开机自启 —— 直接写注册表 CurrentUser\Run
//!
//! 不依赖 tauri-plugin-autostart，避免某些环境（组策略/注册表保护）下返回
//! "Access denied"（os error 5）。写入 HKEY_CURRENT_USER 通常不需要管理员权限。

use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, MAX_PATH};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_OPTION_NON_VOLATILE, REG_SZ, KEY_WRITE, KEY_READ,
    RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW, RegCloseKey,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "MS Manage";

fn to_wstr(s: &str) -> Vec<u16> {
    OsString::from(s).encode_wide().chain(Some(0)).collect()
}

fn get_exe_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

type HKey = *mut std::ffi::c_void;

/// 返回开机自启是否已启用（注册表里存在 MS Manage 项）
pub fn is_enabled() -> bool {
    let hkcu = to_wstr(RUN_KEY);
    let app = to_wstr(APP_NAME);
    let mut hkey: HKey = std::ptr::null_mut();
    unsafe {
        let ret = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            hkcu.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_READ,
            std::ptr::null(),
            &mut hkey,
            std::ptr::null_mut(),
        );
        if ret != ERROR_SUCCESS {
            return false;
        }
        let mut len = 0u32;
        let status = RegQueryValueExW(
            hkey,
            app.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut len,
        );
        RegCloseKey(hkey);
        status == ERROR_SUCCESS
    }
}

/// 开启开机自启：写入注册表 Run 项，带 `--minimized` 参数
pub fn enable() -> Result<(), String> {
    let exe = get_exe_path().ok_or_else(|| "无法获取当前程序路径".to_string())?;
    let exe_str = exe.to_string_lossy().to_string();
    // 命令行格式："C:\path\to\ms_manage.exe" --minimized
    let cmd = format!("\"{}\" --minimized", exe_str.replace('"', "\\\""));

    let hkcu = to_wstr(RUN_KEY);
    let app = to_wstr(APP_NAME);
    let value = to_wstr(&cmd);

    unsafe {
        let mut hkey: HKey = std::ptr::null_mut();
        let ret = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            hkcu.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut hkey,
            std::ptr::null_mut(),
        );
        if ret != ERROR_SUCCESS {
            return Err(format!("打开注册表失败 (0x{:08X})", ret));
        }

        let data_len = (value.len() * 2) as u32;
        let ret = RegSetValueExW(
            hkey,
            app.as_ptr(),
            0,
            REG_SZ,
            value.as_ptr() as *const u8,
            data_len,
        );
        RegCloseKey(hkey);
        if ret != ERROR_SUCCESS {
            return Err(format!("写入注册表失败 (0x{:08X})", ret));
        }
    }
    Ok(())
}

/// 关闭开机自启：删除注册表 Run 项
pub fn disable() -> Result<(), String> {
    let hkcu = to_wstr(RUN_KEY);
    let app = to_wstr(APP_NAME);

    unsafe {
        let mut hkey: HKey = std::ptr::null_mut();
        let ret = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            hkcu.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut hkey,
            std::ptr::null_mut(),
        );
        if ret != ERROR_SUCCESS {
            return Err(format!("打开注册表失败 (0x{:08X})", ret));
        }
        let ret = RegDeleteValueW(hkey, app.as_ptr());
        RegCloseKey(hkey);
        if ret != ERROR_SUCCESS && ret != 2 {
            // 2 = ERROR_FILE_NOT_FOUND，项本来就不存在，不算错
            return Err(format!("删除注册表失败 (0x{:08X})", ret));
        }
    }
    Ok(())
}
