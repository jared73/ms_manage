use crate::usage::UsageDb;
use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use std::str::FromStr;
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

/// 读取 session_log_sync 中的同步进度，返回 (last_modified, last_line_offset)。
/// 无记录返回 (0, 0)；数据库出错返回 Err（避免把真实 DB 故障误当成「尚未同步」而全量重扫）。
pub fn get_sync_state(db: &UsageDb, file_path: &str) -> Result<(i64, i64)> {
    let conn = db.conn.lock();
    let result = conn
        .query_row(
            "SELECT last_modified, last_line_offset FROM session_log_sync WHERE file_path = ?1",
            [file_path],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .context("查询同步状态失败")?;
    Ok(result.unwrap_or((0, 0)))
}

/// 在事务中执行闭包；成功提交，失败回滚。
/// 闭包内不得再调用任何会锁 `db.conn` 的函数（如 get_sync_state / update_sync_state），
/// 否则 parking_lot::Mutex 不可重入会死锁。
pub fn with_transaction<T>(
    db: &UsageDb,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, anyhow::Error>,
) -> Result<T> {
    let conn = db.conn.lock();
    conn.execute_batch("BEGIN")?;
    match f(&conn) {
        Ok(v) => {
            conn.execute_batch("COMMIT")?;
            Ok(v)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
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
/// conn 由调用方持有（通常在 with_transaction 事务内）。
pub fn insert_request_log(    conn: &rusqlite::Connection,
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

/// 遍历全部历史请求，按当前 model_pricing 定价重算费用（改价后调用）。
/// 返回重算的请求条数。整体包在单事务中，避免逐条 fsync；数据量大时调用方需在 spawn_blocking 中执行。
pub fn recalculate_all_costs(db: &crate::usage::UsageDb) -> Result<u32> {
    let conn = db.conn.lock();
    conn.execute_batch("BEGIN")?;
    let result = (|| -> Result<u32> {
        let batch = {
            let mut stmt = conn
                .prepare(
                    "SELECT request_id, app_type, model, input_tokens, output_tokens,
                            cache_read_tokens, cache_creation_tokens, cost_multiplier
                     FROM request_logs",
                )
                .context("读取 request_logs 失败")?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, u32>(3)?,
                        r.get::<_, u32>(4)?,
                        r.get::<_, u32>(5)?,
                        r.get::<_, u32>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                })
                .context("查询 request_logs 失败")?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row.context("读取 request_logs 行失败")?);
            }
            out
        };

        let mut updated = 0u32;
        for (request_id, app_type, model, input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, multiplier) in batch {
            let usage = crate::usage::pricing::TokenUsage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                model: Some(model.clone()),
            };
            let Some(pricing) = crate::usage::pricing::find_model_pricing(&conn, &model) else {
                continue; // 无定价则跳过（保持原值）
            };
            let multiplier = rust_decimal::Decimal::from_str(&multiplier)
                .unwrap_or(rust_decimal::Decimal::from(1));
            let costs = crate::usage::pricing::calculate_cost(
                &usage,
                &pricing,
                crate::usage::pricing::input_includes_cache_read(&app_type),
                multiplier,
            );
            conn.execute(
                "UPDATE request_logs SET
                    input_cost_usd = ?1, output_cost_usd = ?2,
                    cache_read_cost_usd = ?3, cache_creation_cost_usd = ?4,
                    total_cost_usd = ?5, pricing_model = ?6
                 WHERE request_id = ?7",
                rusqlite::params![
                    costs.input_cost.to_string(),
                    costs.output_cost.to_string(),
                    costs.cache_read_cost.to_string(),
                    costs.cache_creation_cost.to_string(),
                    costs.total_cost.to_string(),
                    model,
                    request_id,
                ],
            )
            .context("更新 request_logs 费用失败")?;
            updated += 1;
        }
        Ok(updated)
    })();
    match result {
        Ok(n) => {
            conn.execute_batch("COMMIT")?;
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}
