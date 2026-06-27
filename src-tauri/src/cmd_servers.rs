use crate::state::AppState;
use crate::storage::LogRecord;
use crate::storage::StoredServer;
use chrono::Utc;
use std::sync::Arc;
use tauri::State;

fn to_stored(s: serde_json::Value) -> Result<StoredServer, String> {
    serde_json::from_value(s).map_err(|e| e.to_string())
}

fn from_stored(s: &StoredServer) -> serde_json::Value {
    serde_json::to_value(s).unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
pub fn list_servers(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    Ok(list.iter().map(from_stored).collect())
}

#[tauri::command]
pub fn list_server_states(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let states = state.storage.live_states.read();
    let logs = state.storage.logs.read();
    // 按 server.name 索引最近一条 error log
    let mut last_err: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for l in logs.iter().rev() {
        if l.level == "error" {
            if let Some(rest) = l.source.strip_prefix("server:") {
                last_err.entry(rest.to_string()).or_insert_with(|| l.message.clone());
            }
        }
    }
    Ok(list.iter().map(|s| {
        let st = states.get(&s.id).cloned().unwrap_or_else(|| "stopped".to_string());
        serde_json::json!({
            "server": from_stored(s),
            "status": st,
            "last_error": last_err.get(&s.name).cloned(),
        })
    }).collect())
}

#[tauri::command]
pub fn get_server_state(id: String, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    let states = state.storage.live_states.read();
    let logs = state.storage.logs.read();
    let st = states.get(&s.id).cloned().unwrap_or_else(|| "stopped".to_string());
    let last_err = logs.iter().rev()
        .find(|l| l.level == "error" && l.source == format!("server:{}", s.name))
        .map(|l| l.message.clone());
    Ok(serde_json::json!({ "server": from_stored(&s), "status": st, "last_error": last_err }))
}

#[tauri::command]
pub fn upsert_server(server: serde_json::Value, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let mut s: StoredServer = to_stored(server)?;
    if s.id.is_empty() { s.id = uuid::Uuid::new_v4().to_string(); }
    let now = Utc::now().to_rfc3339();
    if s.created_at.is_empty() { s.created_at = now.clone(); }
    s.updated_at = now;
    let mut list = state.storage.load_servers().map_err(|e| e.to_string())?;
    if let Some(pos) = list.iter().position(|x| x.id == s.id) {
        list[pos] = s.clone();
    } else {
        list.push(s.clone());
    }
    state.storage.save_servers(&list).map_err(|e| e.to_string())?;
    Ok(from_stored(&s))
}

#[tauri::command]
pub async fn delete_server(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut list = state.storage.load_servers().map_err(|e| e.to_string())?;
    list.retain(|s| s.id != id);
    state.storage.save_servers(&list).map_err(|e| e.to_string())?;
    crate::proxy::stop(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn start_server(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    if let Err(e) = crate::proxy::start_one(&s).await {
        // 把详细错误写到 logs（GUI 日志页可见）+ live_states 已设为 Error
        let log = LogRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: "error".to_string(),
            source: format!("server:{}", s.name),
            message: format!("启动失败：{}", e),
            detail: None,
        };
        state.storage.logs.write().push(log);
        return Err(e.to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn stop_server(id: String, _state: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::proxy::stop(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn test_server(id: String, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    crate::proxy::test(&s).await.map_err(|e| e.to_string())
}

/// 仅取工具列表（详情面板用，spawn→init→tools/list→kill）
#[tauri::command]
pub async fn fetch_server_tools(id: String, state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    crate::proxy::fetch_tools(&s).await
}

/// 切换单个工具的启用状态（写进 server.disabled_tools，下次 tools/list 时过滤掉）
#[tauri::command]
pub async fn set_tool_enabled(id: String, tool_name: String, enabled: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.iter_mut().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    s.disabled_tools.retain(|t| t != &tool_name);
    if !enabled { s.disabled_tools.push(tool_name); }
    s.updated_at = Utc::now().to_rfc3339();
    state.storage.save_servers(&list).map_err(|e| e.to_string())?;
    Ok(())
}
