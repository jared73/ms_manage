use crate::state::AppState;
use crate::storage::StoredSettings;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

/// Wire-side AppSettings:送给前端的形态。
///
/// 设计:
/// - 永远只发掩码版 token(`mcpr_***1234`),永远不要发完整 token 到 IPC
/// - 前端不能 round-trip 真实 token;`update_settings` 用 `SafeAppSettings`(无 token 字段)
/// - 想复制真实 token?走专用 `reveal_token` 命令(走 `AppSettings.mcp_token_present` 决定 UI 行为)
#[derive(Debug, Clone, Serialize)]
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
    /// 完整 token 的掩码版(如 `mcpr_***1234`)。None 表示还没生成。
    #[serde(default)]
    pub mcp_token: Option<String>,
    /// 是否已生成 token(用于 UI 决定"重置"按钮 vs "生成"按钮)。
    /// 前端不能用 `mcp_token == null` 判断,因为没生成时也是 None。
    #[serde(default)]
    pub mcp_token_present: bool,
    #[serde(default)]
    pub github_token: Option<String>,
    #[serde(default)]
    pub skill_targets: Vec<crate::storage::SkillTarget>,
}

/// 前端 round-trip 回来的形态。不接受 token 字段——
/// 任何客户端代码都不能用 `update_settings` 来回写 token。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SafeAppSettings {
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
    pub skill_targets: Vec<crate::storage::SkillTarget>,
}

/// 把任意 token 字符串掩码成 `prefix***last4`,保留可识别前缀 + 后 4 位。
/// 太短的(< 8 字符)直接 `***`。
pub(crate) fn mask_token(token: &str) -> String {
    if token.len() < 8 {
        return "***".to_string();
    }
    let prefix: String = token.chars().take(5).collect();
    let suffix: String = token
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{prefix}***{suffix}")
}

impl From<StoredSettings> for AppSettings {
    fn from(s: StoredSettings) -> Self {
        let token_mask = s.mcp_token.as_deref().map(mask_token);
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
            mcp_token: token_mask,
            mcp_token_present: s.mcp_token.is_some(),
            // github_token 也是敏感字段,同样掩码
            github_token: s.github_token.as_deref().map(mask_token),
            skill_targets: s.skill_targets,
        }
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    state.storage.load_settings().map(Into::into).map_err(|e| e.to_string())
}

/// 接受 SafeAppSettings(没有 token 字段),所以前端不能 round-trip 真实 token。
/// 改 token 必须走 `regenerate_token`。
#[tauri::command]
pub fn update_settings(
    settings: SafeAppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut stored = state.storage.load_settings().map_err(|e| e.to_string())?;
    // 只更新 SafeAppSettings 包含的字段;token 字段保持不变
    stored.language = settings.language;
    stored.theme = settings.theme;
    stored.auto_launch = settings.auto_launch;
    stored.proxy_mode = settings.proxy_mode;
    stored.proxy_http_port = settings.proxy_http_port;
    stored.registry_url = settings.registry_url;
    stored.logs_max_entries = settings.logs_max_entries;
    stored.auto_start_proxy = settings.auto_start_proxy;
    stored.npm_registry = settings.npm_registry;
    stored.auto_inject_npm_registry = settings.auto_inject_npm_registry;
    stored.skill_targets = settings.skill_targets;
    state.storage.save_settings(&stored).map_err(|e| e.to_string())?;
    let token = stored.mcp_token.clone().unwrap_or_default();
    // 原子绑 (token, generation):锁内一次性 bump gen + 写 token,
    // handler 读时一次性 clone 整个 tuple,不会看到「gen 已 +1 但 token 还是旧的」。
    {
        let mut guard = state.cached_token.lock().unwrap_or_else(|e| e.into_inner());
        let next_gen = guard.1.wrapping_add(1);
        *guard = (token, next_gen);
    }
    Ok(())
}

#[tauri::command]
pub fn is_autostart_enabled(_app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::autostart::is_enabled())
}

#[tauri::command]
pub fn enable_autostart(_app: tauri::AppHandle) -> Result<(), String> {
    crate::autostart::enable()
}

#[tauri::command]
pub fn disable_autostart(_app: tauri::AppHandle) -> Result<(), String> {
    crate::autostart::disable()
}

#[tauri::command]
pub fn get_auto_launch(_app: tauri::AppHandle) -> Result<bool, String> {
    is_autostart_enabled(_app)
}

#[tauri::command]
pub fn set_auto_launch(enabled: bool, _app: tauri::AppHandle) -> Result<(), String> {
    if enabled { enable_autostart(_app) } else { disable_autostart(_app) }
}

/// 重新生成 MCPR_TOKEN(用户想要换 token 的时候用)。
/// 返回**完整** token —— 用户必须显式调用,前端通过 `reveal_token` 不允许自动触发。
/// 注意:换了之后所有正在用旧 token 连的客户端会立刻 401,
/// 需要把新 token 重新写到它们的 mcpServers 配置里。
#[tauri::command]
pub fn regenerate_token(state: State<'_, Arc<AppState>>) -> Result<RegenerateTokenResult, String> {
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let new_token = crate::storage::generate_token();
    settings.mcp_token = Some(new_token.clone());
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    // 原子绑 (token, generation),见 update_settings 注释。
    {
        let mut guard = state.cached_token.lock().unwrap_or_else(|e| e.into_inner());
        let next_gen = guard.1.wrapping_add(1);
        *guard = (new_token.clone(), next_gen);
    }
    Ok(RegenerateTokenResult { token: new_token })
}

#[derive(Debug, Clone, Serialize)]
pub struct RegenerateTokenResult {
    pub token: String,
}

/// 用户**显式**点击"复制 token"按钮时调用,返回完整 token。
/// 这条路径必须显式,不能前端默认/自动触发。
#[tauri::command]
pub fn reveal_token(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    settings
        .mcp_token
        .ok_or_else(|| "尚未生成 token".to_string())
}

/// 用当前 token 重写所有支持的客户端配置文件。
/// `use_npx` 为 true 时写 npx 配置,false 时写本地 exe 配置。
/// 返回成功写入的客户端名称列表。
#[tauri::command]
pub fn rewrite_client_configs(use_npx: bool) -> Result<Vec<String>, String> {
    Ok(crate::cli::write_all_client_configs(use_npx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_short_token() {
        assert_eq!(mask_token("abc"), "***");
        assert_eq!(mask_token("1234567"), "***");
    }

    #[test]
    fn mask_normal_token_keeps_prefix_and_last4() {
        let m = mask_token("mcpr_abcdefgh1234567");
        assert!(m.starts_with("mcpr_"), "got {m}");
        assert!(m.ends_with("4567"), "got {m}");
        assert!(m.contains("***"));
    }

    #[test]
    fn stored_to_wire_masks_tokens() {
        let stored = StoredSettings {
            language: "zh".into(),
            theme: "purple".into(),
            auto_launch: false,
            proxy_mode: "stdio".into(),
            proxy_http_port: 39231,
            registry_url: None,
            logs_max_entries: 1000,
            auto_start_proxy: Some(true),
            npm_registry: "https://registry.npmjs.org".into(),
            auto_inject_npm_registry: false,
            mcp_token: Some("mcpr_abcdefgh1234567".into()),
            github_token: Some("ghp_1234567890abcdefghij".into()),
            skill_targets: vec![],
        };
        let wire: AppSettings = stored.into();
        // 永远不应出现完整 token
        let serialized = serde_json::to_string(&wire).unwrap();
        assert!(!serialized.contains("abcdefgh1234567"));
        assert!(!serialized.contains("ghp_1234567890abcdefghij"));
        // 但应出现掩码版 + present flag
        assert!(serialized.contains("\"mcp_token_present\":true"));
        assert!(wire.mcp_token.as_deref().unwrap().contains("***"));
    }

    #[test]
    fn stored_to_wire_no_token_is_not_present() {
        let stored = StoredSettings {
            mcp_token: None,
            ..make_min_stored()
        };
        let wire: AppSettings = stored.into();
        assert!(wire.mcp_token.is_none());
        assert!(!wire.mcp_token_present);
    }

    fn make_min_stored() -> StoredSettings {
        StoredSettings {
            language: "zh".into(),
            theme: "purple".into(),
            auto_launch: false,
            proxy_mode: "stdio".into(),
            proxy_http_port: 39231,
            registry_url: None,
            logs_max_entries: 1000,
            auto_start_proxy: None,
            npm_registry: String::new(),
            auto_inject_npm_registry: false,
            mcp_token: None,
            github_token: None,
            skill_targets: vec![],
        }
    }
}