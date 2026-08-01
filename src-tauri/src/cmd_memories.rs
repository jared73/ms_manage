//! 全局记忆文件命令 —— 管理各 AI 工具的全局指令/规则文件。
//!
//! 数据模型：每个记忆条目是「主副本 Markdown 内容」+「目标绝对路径」。
//! 启用后主副本复制到目标路径，让工具读到。应用是主副本权威，
//! 外部改动标记 drift，由用户手动「拉取 / 推送」。

use crate::state::AppState;
use crate::storage::{memories, StoredMemory};
use chrono::Utc;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

/// 前端 DTO：StoredMemory 全字段 + 实时同步状态
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryDto {
    #[serde(flatten)]
    pub memory: StoredMemory,
    #[serde(flatten)]
    pub sync: memories::SyncState,
}

fn to_stored(v: serde_json::Value) -> Result<StoredMemory, String> {
    serde_json::from_value(v).map_err(|e| e.to_string())
}

fn to_dto(m: &StoredMemory) -> MemoryDto {
    let sync = memories::sync_state(m);
    MemoryDto { memory: m.clone(), sync }
}

#[tauri::command]
pub fn list_memories(state: State<'_, Arc<AppState>>) -> Result<Vec<MemoryDto>, String> {
    let list = state.storage.load_memories().map_err(|e| e.to_string())?;
    Ok(list.iter().map(to_dto).collect())
}

#[tauri::command]
pub fn get_memory(id: String, state: State<'_, Arc<AppState>>) -> Result<MemoryDto, String> {
    let list = state.storage.load_memories().map_err(|e| e.to_string())?;
    let m = list.iter().find(|m| m.id == id).ok_or_else(|| "not found".to_string())?;
    Ok(to_dto(m))
}

#[tauri::command]
pub fn upsert_memory(memory: serde_json::Value, state: State<'_, Arc<AppState>>) -> Result<MemoryDto, String> {
    let mut m: StoredMemory = to_stored(memory)?;
    if !m.target_path.is_empty() {
        crate::cmd_skills::validate_skill_path(&m.target_path)?;
    }
    if m.id.is_empty() { m.id = uuid::Uuid::new_v4().to_string(); }
    let now = Utc::now().to_rfc3339();
    if m.created_at.is_empty() { m.created_at = now.clone(); }
    m.updated_at = now;
    let enabled = m.enabled;
    state.storage.upsert_memory(&m).map_err(|e| e.to_string())?;
    // 保存后若启用，推送主副本到目标
    if enabled {
        if let Err(e) = memories::sync_to_target(&m) {
            crate::proxy::log_event("error", "memories", format!("记忆 {} 同步到目标失败：{e}", m.name), None);
        }
    }
    Ok(to_dto(&m))
}

#[tauri::command]
pub fn delete_memory(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.storage.remove_memory(&id).map_err(|e| e.to_string())
}

/// 启用/禁用。启用时做首次安全 reconcile（目标已有内容则先拉回主副本），
/// 随后推送，保证两边一致；禁用时仅改变状态，不删除目标文件。
#[tauri::command]
pub fn toggle_memory(id: String, enabled: bool, state: State<'_, Arc<AppState>>) -> Result<MemoryDto, String> {
    let list = state.storage.load_memories().map_err(|e| e.to_string())?;
    let mut m = list.into_iter().find(|m| m.id == id).ok_or_else(|| "not found".to_string())?;
    m.enabled = enabled;
    m.updated_at = Utc::now().to_rfc3339();
    if enabled {
        memories::enable_with_reconcile(&mut m).map_err(|e| e.to_string())?;
    }
    state.storage.upsert_memory(&m).map_err(|e| e.to_string())?;
    Ok(to_dto(&m))
}

/// 手动推送：主副本覆盖目标文件。返回是否发生写入。
#[tauri::command]
pub fn sync_memory_to_target(id: String, state: State<'_, Arc<AppState>>) -> Result<MemoryDto, String> {
    let list = state.storage.load_memories().map_err(|e| e.to_string())?;
    let mut m = list.into_iter().find(|m| m.id == id).ok_or_else(|| "not found".to_string())?;
    let changed = memories::sync_to_target(&m).map_err(|e| e.to_string())?;
    if changed {
        m.updated_at = Utc::now().to_rfc3339();
        state.storage.upsert_memory(&m).map_err(|e| e.to_string())?;
    }
    Ok(to_dto(&m))
}

/// 手动拉取：目标文件内容覆盖主副本。返回更新后的条目。
#[tauri::command]
pub fn pull_memory_from_target(id: String, state: State<'_, Arc<AppState>>) -> Result<MemoryDto, String> {
    let list = state.storage.load_memories().map_err(|e| e.to_string())?;
    let mut m = list.into_iter().find(|m| m.id == id).ok_or_else(|| "not found".to_string())?;
    let changed = memories::pull_from_target(&mut m).map_err(|e| e.to_string())?;
    if changed {
        state.storage.upsert_memory(&m).map_err(|e| e.to_string())?;
    }
    Ok(to_dto(&m))
}

/// 打开目标文件所在目录（方便用户核对/手动编辑）。
#[tauri::command]
pub fn open_memory_target_dir(id: String, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let list = state.storage.load_memories().map_err(|e| e.to_string())?;
    let m = list.iter().find(|m| m.id == id).ok_or_else(|| "not found".to_string())?;
    let target = std::path::PathBuf::from(&m.target_path);
    let dir = target.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if dir.is_empty() {
        return Err("目标路径缺少父目录".to_string());
    }
    app.opener().open_path(dir, None::<&str>).map_err(|e| e.to_string())
}
