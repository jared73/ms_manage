use crate::state::AppState;
use std::sync::Arc;
use tauri::State;

/// level 数值化:
/// - debug=0, info=1, warn=2, error=3, 其它 → -1
/// - 这样调用方传入"info"时显示 info(1) + warn(2) + error(3),与前端 LEVEL_ORDER 对齐。
fn level_rank(s: &str) -> i32 {
    match s.to_ascii_lowercase().as_str() {
        "debug" => 0,
        "info" => 1,
        "warn" | "warning" => 2,
        "error" | "err" => 3,
        _ => -1,
    }
}

#[tauri::command]
pub fn list_logs(
    opts: Option<serde_json::Value>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let limit = opts.as_ref()
        .and_then(|o| o.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;
    // level_min:后端过滤掉 rank < min 的条目,前端不必再过滤一遍。
    // 老 level 字段保留兼容(精确匹配),level_min 优先级更高。
    let level_min = opts.as_ref()
        .and_then(|o| o.get("level_min"))
        .and_then(|v| v.as_str())
        .map(level_rank)
        .filter(|r| *r >= 0);
    let level_filter = opts.as_ref()
        .and_then(|o| o.get("level"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let source_filter = opts.as_ref()
        .and_then(|o| o.get("source"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let logs = state.storage.logs.read();
    // 倒序遍历（最新在前），跳过不匹配的，只取 limit 条 —— 避免 clone 全量日志再 truncate。
    let filtered: Vec<_> = logs.iter()
        .rev()
        .filter(|l| match level_min {
            Some(min) => level_rank(&l.level) >= min,
            None => level_filter.as_ref().map(|lv| l.level == *lv).unwrap_or(true),
        })
        .filter(|l| source_filter.as_ref().map(|sv| l.source.starts_with(sv)).unwrap_or(true))
        .take(limit)
        .map(|l| serde_json::to_value(l).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok(filtered)
}

#[tauri::command]
pub fn clear_logs(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.storage.logs.write().clear();
    Ok(())
}
