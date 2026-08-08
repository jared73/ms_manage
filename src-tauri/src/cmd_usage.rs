use crate::state::AppState;
use crate::usage::session_common::SessionSyncResult;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// tauri command 错误转换：把任意 Display 错误转 String，省掉 30+ 处 `.map_err(|e| e.to_string())` 模板代码。
/// 用法：`conn.query_row(...).cmd_err()?`
pub trait CmdErr<T> {
    fn cmd_err(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> CmdErr<T> for Result<T, E> {
    fn cmd_err(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn get_usage_summary(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
) -> Result<crate::usage::stats::UsageSummary, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        crate::usage::stats::get_usage_summary(&db, start_date, end_date, app_type.as_deref())
    })
    .await
    .cmd_err()?
    .cmd_err()
}

#[tauri::command]
pub async fn get_provider_stats(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::ProviderStats>, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        crate::usage::stats::get_provider_stats(&db, start_date, end_date)
    })
    .await
    .cmd_err()?
    .cmd_err()
}

#[tauri::command]
pub async fn get_usage_trends(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: Option<String>,
    app_type: Option<String>,
    model: Option<String>,
) -> Result<Vec<crate::usage::stats::DailyStats>, String> {
    let db = state.usage_db.clone();
    let grouping = grouping.unwrap_or_else(|| "day".to_string());
    tokio::task::spawn_blocking(move || {
        crate::usage::stats::get_usage_trends(
            &db,
            start_date,
            end_date,
            &grouping,
            app_type.as_deref(),
            model.as_deref(),
        )
    })
    .await
    .cmd_err()?
    .cmd_err()
}

#[tauri::command]
pub async fn get_model_stats(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::ModelStats>, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        crate::usage::stats::get_model_stats(&db, start_date, end_date)
    })
    .await
    .cmd_err()?
    .cmd_err()
}

#[tauri::command]
pub async fn get_usage_data_sources(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::usage::stats::DataSourceStatus>, String> {
    let overrides = state
        .storage
        .load_settings()
        .map(|s| s.usage_overrides)
        .unwrap_or_default();
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || crate::usage::stats::get_usage_data_sources(&db, &overrides))
        .await
        .cmd_err()?
        .cmd_err()
}

/// 导出请求日志 CSV（按当前过滤条件），返回 CSV 文本。
#[tauri::command]
pub async fn export_request_logs_csv(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
    model: Option<String>,
) -> Result<String, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        crate::usage::stats::export_request_logs_csv(
            &db,
            start_date,
            end_date,
            app_type.as_deref(),
            model.as_deref(),
        )
    })
    .await
    .cmd_err()?
    .cmd_err()
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
    .cmd_err()?;
    result.cmd_err()
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
pub async fn get_model_pricing(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT model_id, display_name, input_cost_per_million, output_cost_per_million,
                        cache_read_cost_per_million, cache_creation_cost_per_million
                 FROM model_pricing ORDER BY model_id",
            )
            .cmd_err()?;
        let rows = stmt
            .query_map([], |r| {
                Ok(serde_json::json!({
                    "modelId": r.get::<_, String>(0)?,
                    "displayName": r.get::<_, String>(1)?,
                    "inputCostPerMillion": r.get::<_, String>(2)?,
                    "outputCostPerMillion": r.get::<_, String>(3)?,
                    "cacheReadCostPerMillion": r.get::<_, String>(4)?,
                    "cacheCreationCostPerMillion": r.get::<_, String>(5)?,
                }))
            })
            .cmd_err()?;
        rows.collect::<Result<Vec<_>, _>>().cmd_err()
    })
    .await
    .cmd_err()?
}

fn validate_price(name: &str, model: &str, v: &str) -> Result<(), String> {
    match v.trim().parse::<f64>() {
        Ok(n) if n >= 0.0 => Ok(()),
        _ => Err(format!("模型 {model} 的 {name} 定价非法: {v}")),
    }
}

#[tauri::command]
pub async fn delete_model_pricing(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn.lock();
        conn.execute("DELETE FROM model_pricing WHERE model_id = ?1", [&model_id])
            .cmd_err()?;
        Ok(())
    })
    .await
    .cmd_err()?
}

/// app 级成本倍率配置
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigInput {
    pub app_type: String,
    pub cost_multiplier: String,
}

#[tauri::command]
pub async fn get_app_configs(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare("SELECT app_type, cost_multiplier FROM app_config ORDER BY app_type")
            .cmd_err()?;
        let rows = stmt
            .query_map([], |r| {
                Ok(serde_json::json!({
                    "appType": r.get::<_, String>(0)?,
                    "costMultiplier": r.get::<_, String>(1)?,
                }))
            })
            .cmd_err()?;
        rows.collect::<Result<Vec<_>, _>>().cmd_err()
    })
    .await
    .cmd_err()?
}

#[tauri::command]
pub async fn update_app_config_batch(
    state: State<'_, Arc<AppState>>,
    items: Vec<AppConfigInput>,
) -> Result<(), String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.conn.lock();
        for item in &items {
            match item.cost_multiplier.trim().parse::<f64>() {
                Ok(n) if n > 0.0 => {}
                _ => {
                    return Err(format!(
                        "应用 {} 的成本倍率非法: {}",
                        item.app_type, item.cost_multiplier
                    ))
                }
            }
        }
        for item in items {
            conn.execute(
                "INSERT INTO app_config (app_type, cost_multiplier)
                 VALUES (?1, ?2)
                 ON CONFLICT(app_type) DO UPDATE SET cost_multiplier = excluded.cost_multiplier",
                rusqlite::params![item.app_type, item.cost_multiplier.trim()],
            )
            .cmd_err()?;
        }
        Ok(())
    })
    .await
    .cmd_err()?
}

#[tauri::command]
pub async fn update_model_pricing_batch(
    state: State<'_, Arc<AppState>>,
    items: Vec<ModelPricingInput>,
) -> Result<(), String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
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
            .cmd_err()?;
        }
        Ok(())
    })
    .await
    .cmd_err()?
}

/// 按当前模型定价重算全部历史请求的费用（改价后调用，同步更新 rollup）。
/// 数据量大时耗时较长，放到阻塞线程池执行，避免卡住 tauri 主线程。
/// 每处理 500 条往 `usage://recalc-progress` 事件 emit 当前进度（processed 条数）。
#[tauri::command]
pub async fn recalculate_costs(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<u32, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        // 与后台同步互斥，避免重算期间新数据落库导致结果不一致
        let _guard = crate::usage::sync_mutex().lock();
        let app_for_progress = app.clone();
        let recalculated = crate::usage::session_common::recalculate_all_costs(&db, |processed| {
            let _ = app_for_progress.emit("usage://recalc-progress", processed);
        })
        .cmd_err()?;
        // 重算后重建 rollup
        crate::usage::rollup::rebuild_rollups(&db).cmd_err()?;
        Ok(recalculated)
    })
    .await
    .cmd_err()?
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
pub async fn get_request_logs(
    state: State<'_, Arc<AppState>>,
    filters: RequestLogFilters,
) -> Result<PaginatedLogs, String> {
    let db = state.usage_db.clone();
    let page = filters.page.unwrap_or(1).max(1);
    let page_size = filters.page_size.unwrap_or(50).min(500);
    tokio::task::spawn_blocking(move || {
        let p = crate::usage::stats::get_request_logs(
            &db,
            filters.start_date,
            filters.end_date,
            filters.app_type.as_deref(),
            filters.model.as_deref(),
            page,
            page_size,
        )
        .cmd_err()?;
        Ok(PaginatedLogs { rows: p.rows, total: p.total, page: p.page, page_size: p.page_size })
    })
    .await
    .cmd_err()?
}

/// 返回 usage.db 启动时的损坏恢复状态（前端首次 query 时若非 None 即 toast 告知）。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatus {
    pub backup_path: String,
    pub recovered_at: i64,
}

#[tauri::command]
pub fn get_usage_recovery_status(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<RecoveryStatus>, String> {
    let db = state.usage_db.clone();
    let guard = db.recovered_backup.lock().cmd_err()?;
    Ok(guard.as_ref().map(|p| RecoveryStatus {
        backup_path: p.to_string_lossy().to_string(),
        recovered_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    }))
}

/// 重置同步水位线：删除 session_log_sync 全部行，下次同步会重新扫描全部数据源。
/// 已通过 INSERT OR IGNORE 写入的 request_logs 不会重复导入；水位线推后等于重跑全量扫描
/// （对部分文件可能 re-parse 一次但 INSERT 全部跳过，IO 微小）。
/// 与 sync_mutex 互斥，避免与正在跑的同步并发。
#[tauri::command]
pub async fn reset_sync_state(
    state: State<'_, Arc<AppState>>,
) -> Result<u32, String> {
    let db = state.usage_db.clone();
    tokio::task::spawn_blocking(move || {
        let _guard = crate::usage::sync_mutex().lock();
        let conn = db.conn.lock();
        let n = conn
            .execute("DELETE FROM session_log_sync", [])
            .map_err(|e| e.to_string())?;
        Ok(n as u32)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 把 CSV 文本写到用户选定的路径。前端流程：
/// 1. `save` dialog 拿到用户选的 `path`
/// 2. `exportRequestLogsCsv` 拿到 csv 文本
/// 3. invoke `writeCsvFile(path, csv)` 写文件
/// 用 Rust 直接写比前端 `tauri-plugin-fs` 的 `writeTextFile` 更稳 —— 后者需要 scope 限制
/// （默认 fs:default 只允许 app 目录），每次改 scope 都要重 build capabilities。
#[tauri::command]
pub async fn write_csv_file(path: String, content: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(format!("目标目录不存在: {}", parent.display()));
        }
    }
    std::fs::write(&p, content).map_err(|e| format!("写入失败: {e}"))
}
