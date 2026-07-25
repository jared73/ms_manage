use crate::state::AppState;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn app_info(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "data_dir": state.data_dir.to_string_lossy(),
        "log_dir": state.log_dir.to_string_lossy(),
        "app_dir": std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
    })
}

#[tauri::command]
pub fn open_data_dir(app: AppHandle, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    app.opener()
        .open_path(state.data_dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    // 仅放行 http/https —— 防御 webview 注入 `file://` / `cmd://` 等本地程序协议
    // 让用户受害者点开就 spawn 任意进程。
    let trimmed = url.trim();
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(format!("仅允许 http(s) scheme, 收到:{trimmed}"));
    }
    app.opener()
        .open_url(trimmed, None::<&str>)
        .map_err(|e| e.to_string())
}
