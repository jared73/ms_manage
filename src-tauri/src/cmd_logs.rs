use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_logs(
    opts: Option<serde_json::Value>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let limit = opts.as_ref()
        .and_then(|o| o.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;
    let level_filter = opts.as_ref()
        .and_then(|o| o.get("level"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let source_filter = opts.as_ref()
        .and_then(|o| o.get("source"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let logs = state.storage.logs.read();
    let mut filtered: Vec<_> = logs.iter()
        .filter(|l| level_filter.as_ref().map(|lv| l.level == *lv).unwrap_or(true))
        .filter(|l| source_filter.as_ref().map(|sv| l.source.starts_with(sv)).unwrap_or(true))
        .cloned()
        .collect();
    filtered.reverse(); // 最新的在前
    filtered.truncate(limit);
    Ok(filtered.iter().map(|l| serde_json::to_value(l).unwrap_or(serde_json::Value::Null)).collect())
}

#[tauri::command]
pub fn clear_logs(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.storage.logs.write().clear();
    Ok(())
}
