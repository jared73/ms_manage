use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::time::SystemTime;

/// 一次同步的结果汇总
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSyncResult {
    pub imported: u32,
    pub skipped: u32,
    pub files_scanned: u32,
    pub suspected_duplicates: u32,
    pub deferred_files: u32,
    pub errors: Vec<String>,
}

impl SessionSyncResult {
    pub fn merge(&mut self, other: SessionSyncResult) {
        self.imported += other.imported;
        self.skipped += other.skipped;
        self.files_scanned += other.files_scanned;
        self.suspected_duplicates += other.suspected_duplicates;
        self.deferred_files += other.deferred_files;
        self.errors.extend(other.errors);
    }
}

/// 读取 session_log_sync 中的同步进度，返回 (last_modified, last_line_offset)
pub fn get_sync_state(db: &UsageDb, file_path: &str) -> Result<(i64, i64)> {
    let conn = db.conn.lock();
    let result = conn.query_row(
        "SELECT last_modified, last_line_offset FROM session_log_sync WHERE file_path = ?1",
        [file_path],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    );
    Ok(result.unwrap_or((0, 0)))
}

/// 文件 mtime 的纳秒时间戳
pub fn metadata_modified_nanos(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// 更新同步进度
pub fn update_sync_state(db: &UsageDb, file_path: &str, last_modified: i64, line_offset: i64) -> Result<()> {
    let conn = db.conn.lock();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO session_log_sync (file_path, last_modified, last_line_offset, last_synced_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(file_path) DO UPDATE SET
            last_modified = excluded.last_modified,
            last_line_offset = excluded.last_line_offset,
            last_synced_at = excluded.last_synced_at",
        rusqlite::params![file_path, last_modified, line_offset, now],
    )
    .context("更新同步状态失败")?;
    Ok(())
}

/// 插入一条请求日志。request_id 冲突时跳过（INSERT OR IGNORE）。
/// 返回 true = 新插入，false = 已存在跳过。
pub fn insert_request_log(
    db: &UsageDb,
    request_id: &str,
    app_type: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    costs: &crate::usage::pricing::CostBreakdown,
    total_cost_usd: &str,
    cost_multiplier: &str,
    session_id: Option<&str>,
    created_at: i64,
    input_token_semantics: i64,
) -> Result<bool> {
    let conn = db.conn.lock();
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO request_logs (
            request_id, provider_id, app_type, model, request_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            input_token_semantics,
            input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd, total_cost_usd,
            latency_ms, status_code, session_id, is_streaming, cost_multiplier, created_at, data_source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        rusqlite::params![
            request_id,
            format!("_{app_type}_session"),
            app_type,
            model,
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            input_token_semantics,
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            total_cost_usd,
            0i64,
            200i64,
            session_id,
            1i64,
            cost_multiplier,
            created_at,
            "session",
        ],
    )
    .context("插入 request_logs 失败")?;
    Ok(inserted > 0)
}
