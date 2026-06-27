use crate::state::AppState;
use crate::storage::{StoredProject, UNASSIGNED_PROJECT_ID};
use chrono::Utc;
use std::sync::Arc;
use tauri::State;

fn to_stored(p: serde_json::Value) -> Result<StoredProject, String> {
    serde_json::from_value(p).map_err(|e| e.to_string())
}

fn from_stored(p: &StoredProject) -> serde_json::Value {
    serde_json::to_value(p).unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
pub fn list_projects(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let mut list = state.storage.load_projects().map_err(|e| e.to_string())?;
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    // 在前面插入"未分组"虚拟项目（id = __unassigned__），前端用来展示
    let mut out = vec![serde_json::json!({
        "id": UNASSIGNED_PROJECT_ID,
        "name": "未分组",
        "description": "所有未指定项目的 MCP server",
        "created_at": "",
        "updated_at": "",
    })];
    out.extend(list.iter().map(from_stored));
    Ok(out)
}

#[tauri::command]
pub fn upsert_project(project: serde_json::Value, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let mut p: StoredProject = to_stored(project)?;
    if p.id.is_empty() || p.id == UNASSIGNED_PROJECT_ID {
        p.id = uuid::Uuid::new_v4().to_string();
    }
    let now = Utc::now().to_rfc3339();
    if p.created_at.is_empty() { p.created_at = now.clone(); }
    p.updated_at = now;
    let mut list = state.storage.load_projects().map_err(|e| e.to_string())?;
    // 唯一性：name 大小写不敏感（与 mcp-router 一致）
    if list.iter().any(|x| x.id != p.id && x.name.to_lowercase() == p.name.to_lowercase()) {
        return Err(format!("项目名 '{}' 已存在", p.name));
    }
    if let Some(pos) = list.iter().position(|x| x.id == p.id) {
        list[pos] = p.clone();
    } else {
        list.push(p.clone());
    }
    state.storage.save_projects(&list).map_err(|e| e.to_string())?;
    Ok(from_stored(&p))
}

#[tauri::command]
pub fn delete_project(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if id == UNASSIGNED_PROJECT_ID {
        return Err("无法删除「未分组」".into());
    }
    let mut list = state.storage.load_projects().map_err(|e| e.to_string())?;
    if !list.iter().any(|x| x.id == id) {
        return Err("项目不存在".into());
    }
    list.retain(|x| x.id != id);
    state.storage.save_projects(&list).map_err(|e| e.to_string())?;
    // 级联：把 server 的 project_id 重置为 None（移到「未分组」）
    let mut servers = state.storage.load_servers().map_err(|e| e.to_string())?;
    for s in servers.iter_mut() {
        if s.project_id.as_deref() == Some(&id) {
            s.project_id = None;
            s.updated_at = Utc::now().to_rfc3339();
        }
    }
    state.storage.save_servers(&servers).map_err(|e| e.to_string())?;
    Ok(())
}
