//! 模型配置 + skill 解析 IPC 命令
//!
//! 设计:大多数命令走 sync(查配置、查 preset 列表),真正调 LLM 的命令(async)
//! 是 fetch_models / analyze_skill / analyze_all_skills —— 网络请求必须 async。

use crate::model;
use crate::model::analyze;
use crate::state::AppState;
use crate::storage::StoredSettings;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// 返回前端展示的所有 preset
#[tauri::command]
pub fn list_model_presets() -> Vec<model::ModelPreset> {
    model::presets::all_presets()
}

#[tauri::command]
pub fn get_model_config(state: State<'_, Arc<AppState>>) -> Result<Option<model::ModelConfig>, String> {
    let s = state.storage.load_settings().map_err(|e| e.to_string())?;
    Ok(s.model_config)
}

#[tauri::command]
pub fn save_model_config(
    config: model::ModelConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut s = state.storage.load_settings().map_err(|e| e.to_string())?;
    // 把当前 apiKey 一并存到 provider_keys map,实现"切来切去不丢 key"
    if !config.api_key.trim().is_empty() {
        s.provider_keys.insert(config.provider.clone(), config.api_key.clone());
    }
    s.model_config = Some(config);
    state.storage.save_settings(&s).map_err(|e| e.to_string())
}

/// 取所有 provider 的 key(供前端切换供应商时恢复用)
#[tauri::command]
pub fn get_provider_keys(state: State<'_, Arc<AppState>>) -> Result<std::collections::HashMap<String, String>, String> {
    let s = state.storage.load_settings().map_err(|e| e.to_string())?;
    Ok(s.provider_keys)
}

/// 异步拉模型列表 —— 调用 OpenAI 兼容 GET /v1/models(候选 URL 策略见 model::openai)
#[tauri::command]
pub async fn fetch_model_list(config: model::ModelConfig) -> Result<Vec<model::openai::FetchedModel>, String> {
    if config.api_key.trim().is_empty() {
        return Err("请先填写 API Key".to_string());
    }
    model::openai::list_models(&config.base_url, &config.api_key)
        .await
        .map_err(|e| e.to_string())
}

/// 异步解析单个 skill
#[tauri::command]
pub async fn analyze_skill(
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<model::SkillAnalysis, String> {
    let settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let cfg = settings.model_config.ok_or_else(|| "请先在「模型」页配置 LLM".to_string())?;
    let skills = state.storage.load_skills().map_err(|e| e.to_string())?;
    let skill = skills.into_iter()
        .find(|s| s.id == skill_id)
        .ok_or_else(|| format!("找不到 skill: {skill_id}"))?;
    let md_path: PathBuf = std::path::PathBuf::from(&skill.dir_path).join("SKILL.md");
    let content = std::fs::read_to_string(&md_path).map_err(|e| format!("读 SKILL.md 失败: {e}"))?;
    analyze::analyze_one(&state.data_dir, &cfg, &skill_id, &content)
        .await
        .map_err(|e| e.to_string())
}

/// 全局更新所有 skill 的解析 —— 异步 + 进度事件
#[tauri::command]
pub async fn analyze_all_skills(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<usize, String> {
    let cfg = {
        let s = state.storage.load_settings().map_err(|e| e.to_string())?;
        s.model_config.ok_or_else(|| "请先在「模型」页配置 LLM".to_string())?
    };
    analyze::analyze_all(app, state, cfg).await
}

/// 取缓存的解析结果
#[tauri::command]
pub fn get_skill_analysis(
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<model::SkillAnalysis>, String> {
    model::storage::get(&state.data_dir, &skill_id).map_err(|e| e.to_string())
}

/// 列出所有已有解析 —— 启动时一次性加载,避免每张卡片都 IPC 一次
#[tauri::command]
pub fn list_skill_analyses(
    state: State<'_, Arc<AppState>>,
) -> Result<std::collections::HashMap<String, model::SkillAnalysis>, String> {
    let f = model::storage::load(&state.data_dir).map_err(|e| e.to_string())?;
    Ok(f.analyses)
}

// 抑制 unused 警告(StoredSettings 在这里仅作占位提示)
#[allow(dead_code)]
fn _ensure_settings_used(_: &StoredSettings) {}