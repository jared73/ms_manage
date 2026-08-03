use crate::state::AppState;
use crate::usage::session_common::SessionSyncResult;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_usage_summary(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
) -> Result<crate::usage::stats::UsageSummary, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_usage_summary(&db, start_date, end_date, app_type.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_usage_summary_by_app(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::UsageSummaryByApp>, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_usage_summary_by_app(&db, start_date, end_date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_usage_trends(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: Option<String>,
) -> Result<Vec<crate::usage::stats::DailyStats>, String> {
    let db = state.usage_db.clone();
    let grouping = grouping.as_deref().unwrap_or("day");
    crate::usage::stats::get_usage_trends(&db, start_date, end_date, grouping).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_model_stats(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::ModelStats>, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_model_stats(&db, start_date, end_date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_usage_data_sources(
    state: State<'_, Arc<AppState>>,
) -> Vec<crate::usage::stats::DataSourceStatus> {
    let overrides = state
        .storage
        .load_settings()
        .map(|s| s.usage_overrides)
        .unwrap_or_default();
    crate::usage::stats::get_usage_data_sources(&overrides)
}

/// 立即手动同步所有数据源
#[tauri::command]
pub async fn sync_session_usage(state: State<'_, Arc<AppState>>) -> Result<SessionSyncResult, String> {
    let db = state.usage_db.clone();
    let overrides = state
        .storage
        .load_settings()
        .map(|s| s.usage_overrides)
        .unwrap_or_default();
    let result = tokio::task::spawn_blocking(move || {
        let _guard = crate::usage::sync_mutex().lock();
        let paths = crate::usage::paths::UsagePaths::detect().apply_overrides(&overrides);
        crate::usage::sync_all(&db, &paths)
    })
    .await
    .map_err(|e| e.to_string())?;
    result.map_err(|e| e.to_string())
}

/// 模型定价管理
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricingInput {
    pub model_id: String,
    pub display_name: Option<String>,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    pub cache_creation_cost_per_million: String,
}

#[tauri::command]
pub fn get_model_pricing(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.usage_db.clone();
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT model_id, input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million
             FROM model_pricing ORDER BY model_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "modelId": r.get::<_, String>(0)?,
                "inputCostPerMillion": r.get::<_, String>(1)?,
                "outputCostPerMillion": r.get::<_, String>(2)?,
                "cacheReadCostPerMillion": r.get::<_, String>(3)?,
                "cacheCreationCostPerMillion": r.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn validate_price(name: &str, model: &str, v: &str) -> Result<(), String> {
    match v.trim().parse::<f64>() {
        Ok(n) if n >= 0.0 => Ok(()),
        _ => Err(format!("模型 {model} 的 {name} 定价非法: {v}")),
    }
}

#[tauri::command]
pub fn update_model_pricing_batch(
    state: State<'_, Arc<AppState>>,
    items: Vec<ModelPricingInput>,
) -> Result<(), String> {
    let db = state.usage_db.clone();
    let conn = db.conn.lock();
    for item in &items {
        validate_price("input_cost_per_million", &item.model_id, &item.input_cost_per_million)?;
        validate_price("output_cost_per_million", &item.model_id, &item.output_cost_per_million)?;
        validate_price("cache_read_cost_per_million", &item.model_id, &item.cache_read_cost_per_million)?;
        validate_price("cache_creation_cost_per_million", &item.model_id, &item.cache_creation_cost_per_million)?;
    }
    for item in items {
        let display_name = item
            .display_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&item.model_id);
        conn.execute(
            "INSERT INTO model_pricing (model_id, display_name,
                input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million)
             VALUES (?1, ?6, ?2, ?3, ?4, ?5)
             ON CONFLICT(model_id) DO UPDATE SET
                display_name = excluded.display_name,
                input_cost_per_million = excluded.input_cost_per_million,
                output_cost_per_million = excluded.output_cost_per_million,
                cache_read_cost_per_million = excluded.cache_read_cost_per_million,
                cache_creation_cost_per_million = excluded.cache_creation_cost_per_million",
            rusqlite::params![
                item.model_id,
                item.input_cost_per_million,
                item.output_cost_per_million,
                item.cache_read_cost_per_million,
                item.cache_creation_cost_per_million,
                display_name,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogFilters {
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub app_type: Option<String>,
    pub model: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedLogs {
    pub rows: Vec<crate::usage::stats::RequestLogRow>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[tauri::command]
pub fn get_request_logs(
    state: State<'_, Arc<AppState>>,
    filters: RequestLogFilters,
) -> Result<PaginatedLogs, String> {
    let db = state.usage_db.clone();
    let p = crate::usage::stats::get_request_logs(
        &db,
        filters.start_date,
        filters.end_date,
        filters.app_type.as_deref(),
        filters.model.as_deref(),
        filters.page.unwrap_or(1).max(1),
        filters.page_size.unwrap_or(50).min(500),
    )
    .map_err(|e| e.to_string())?;
    Ok(PaginatedLogs { rows: p.rows, total: p.total, page: p.page, page_size: p.page_size })
}
