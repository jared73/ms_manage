use crate::state::AppState;
use crate::storage::StoredSettings;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub language: String,
    pub theme: String,
    pub auto_launch: bool,
    pub proxy_mode: String,
    pub proxy_http_port: u16,
    pub registry_url: Option<String>,
    pub logs_max_entries: usize,
    #[serde(default)]
    pub auto_start_proxy: Option<bool>,
    #[serde(default)]
    pub npm_registry: String,
    #[serde(default)]
    pub auto_inject_npm_registry: bool,
    #[serde(default)]
    pub mcp_token: Option<String>,
    #[serde(default)]
    pub skill_targets: Vec<crate::storage::SkillTarget>,
}

impl From<StoredSettings> for AppSettings {
    fn from(s: StoredSettings) -> Self {
        Self {
            language: s.language,
            theme: s.theme,
            auto_launch: s.auto_launch,
            proxy_mode: s.proxy_mode,
            proxy_http_port: s.proxy_http_port,
            registry_url: s.registry_url,
            logs_max_entries: s.logs_max_entries,
            auto_start_proxy: s.auto_start_proxy,
            npm_registry: s.npm_registry,
            auto_inject_npm_registry: s.auto_inject_npm_registry,
            mcp_token: s.mcp_token,
            skill_targets: s.skill_targets,
        }
    }
}

impl From<AppSettings> for StoredSettings {
    fn from(s: AppSettings) -> Self {
        Self {
            language: s.language,
            theme: s.theme,
            auto_launch: s.auto_launch,
            proxy_mode: s.proxy_mode,
            proxy_http_port: s.proxy_http_port,
            registry_url: s.registry_url,
            logs_max_entries: s.logs_max_entries,
            auto_start_proxy: s.auto_start_proxy,
            npm_registry: s.npm_registry,
            auto_inject_npm_registry: s.auto_inject_npm_registry,
            mcp_token: s.mcp_token,
            skill_targets: s.skill_targets,
        }
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    state.storage.load_settings().map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_settings(
    settings: AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let stored: StoredSettings = settings.into();
    state.storage.save_settings(&stored).map_err(|e| e.to_string())
}

/// 开机自启动开关
#[tauri::command]
pub fn get_auto_launch(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_auto_launch(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| e.to_string())
    } else {
        mgr.disable().map_err(|e| e.to_string())
    }
}

/// 重新生成 MCPR_TOKEN（用户想要换 token 的时候用）。
/// 注意：换了之后所有正在用旧 token 连的客户端会立刻 401，
/// 需要把新 token 重新写到它们的 mcpServers 配置里。
#[tauri::command]
pub fn regenerate_token(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let new_token = crate::storage::generate_token();
    settings.mcp_token = Some(new_token.clone());
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(new_token)
}
