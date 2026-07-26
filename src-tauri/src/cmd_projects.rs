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
    // 唯一性：name 大小写不敏感
    let list = state.storage.load_projects().map_err(|e| e.to_string())?;
    if list.iter().any(|x| x.id != p.id && x.name.to_lowercase() == p.name.to_lowercase()) {
        return Err(format!("项目名 '{}' 已存在", p.name));
    }
    // 原子 upsert（更新或插入），持有写锁避免并发覆盖
    state.storage.upsert_project(&p).map_err(|e| e.to_string())?;
    Ok(from_stored(&p))
}

#[tauri::command]
pub fn delete_project(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if id == UNASSIGNED_PROJECT_ID {
        return Err("无法删除「未分组」".into());
    }
    let list = state.storage.load_projects().map_err(|e| e.to_string())?;
    if !list.iter().any(|x| x.id == id) {
        return Err("项目不存在".into());
    }
    // 原子删除 project
    state.storage.remove_project(&id).map_err(|e| e.to_string())?;
    // 级联：把 server 的 project_id 重置为 None（移到「未分组」）
    for sid in state.storage.load_servers().map_err(|e| e.to_string())?.iter().filter(|s| s.project_id.as_deref() == Some(&id)).map(|s| s.id.clone()) {
        if let Err(e) = state.storage.mutate_server(&sid, |srv| {
            srv.project_id = None;
            true
        }) {
            crate::proxy::log_event("error", "projects", format!("级联移除 server {sid} 的项目归属失败：{e}"), None);
        }
    }
    Ok(())
}
