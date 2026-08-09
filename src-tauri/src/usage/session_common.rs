use crate::usage::UsageDb;
use crate::usage::pricing::ModelPricing;
use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use std::collections::HashMap;
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

/// 读取 session_log_sync 中的同步进度，返回 (last_modified, last_line_offset, byte_offset, last_processed_message_ts)。
/// 无记录返回 (0, 0, 0, 0)；数据库出错返回 Err（避免把真实 DB 故障误当成「尚未同步」而全量重扫）。
/// byte_offset 是文件绝对字节偏移，下次 sync 可 seek 到这里跳过已有内容（避免大文件 IO 重）。
/// last_processed_message_ts 是 opencode 等 SQLite 源用的 message 级增量水位线（毫秒）。
pub fn get_sync_state(db: &UsageDb, file_path: &str) -> Result<(i64, i64, i64, i64)> {
    let conn = db.conn.lock();
    let result = conn
        .query_row(
            "SELECT last_modified, last_line_offset, byte_offset, last_processed_message_ts
             FROM session_log_sync WHERE file_path = ?1",
            [file_path],
            |row| Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            )),
        )
        .optional()
        .context("查询同步状态失败")?;
    Ok(result.unwrap_or((0, 0, 0, 0)))
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

/// 更新同步进度。
/// `last_processed_message_ts`：opencode 等 message 级增量水位线（毫秒）；其他源传 0（不变）。
pub fn update_sync_state(
    db: &UsageDb,
    file_path: &str,
    last_modified: i64,
    line_offset: i64,
    byte_offset: i64,
    last_processed_message_ts: i64,
) -> Result<()> {
    let conn = db.conn.lock();
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO session_log_sync (
            file_path, last_modified, last_line_offset, byte_offset,
            last_processed_message_ts, last_synced_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(file_path) DO UPDATE SET
            last_modified = excluded.last_modified,
            last_line_offset = excluded.last_line_offset,
            byte_offset = excluded.byte_offset,
            last_processed_message_ts = CASE
                WHEN excluded.last_processed_message_ts > last_processed_message_ts
                THEN excluded.last_processed_message_ts
                ELSE last_processed_message_ts
            END,
            last_synced_at = excluded.last_synced_at",
        rusqlite::params![file_path, last_modified, line_offset, byte_offset, last_processed_message_ts, now],
    )
    .context("更新同步状态失败")?;
    Ok(())
}

/// 插入一条请求日志。request_id 冲突时跳过（INSERT OR IGNORE）。
/// 返回 true = 新插入，false = 已存在跳过。
/// conn 由调用方持有（通常在 with_transaction 事务内）。
/// `status_code` / `error_message`：失败识别（200/None=成功；非 200 或 Some 触发 rollup failed_count）。
/// `cost_source`："session_reported"=上游已报真实 cost（不参与 recalculate），
///                "pricing_estimated"=按 model_pricing 表估算（recalculate 会重算）。
#[allow(clippy::too_many_arguments)]
pub fn insert_request_log(
    conn: &rusqlite::Connection,
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
    status_code: i64,
    error_message: Option<&str>,
    cost_source: &str,
) -> Result<bool> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO request_logs (
            request_id, provider_id, app_type, model, request_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            input_token_semantics,
            input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd, total_cost_usd,
            latency_ms, status_code, error_message, session_id, is_streaming, cost_multiplier,
            cost_source, created_at, data_source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
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
            status_code,
            error_message,
            session_id,
            1i64,
            cost_multiplier,
            cost_source,
            created_at,
            "session",
        ],
    )
    .context("插入 request_logs 失败")?;
    Ok(inserted > 0)
}

/// 遍历全部历史请求，按当前 model_pricing 定价重算费用（改价后调用）。
/// 使用批量 UPSERT（每 500 条/批）替代逐行 UPDATE，大表上耗时从天-分钟级降到秒级；
/// 每次批提交后回调 `progress(updated)`，调用方可在 Tauri emit 进度事件。
/// 返回重算的请求条数。调用方需在 spawn_blocking 中执行，并持 `sync_mutex` 与后台同步互斥。
///
/// 仅重算 `cost_source = 'pricing_estimated'` 的行 —— `session_reported` 的行（如 opencode
/// 自报的 cost）保留不动，避免覆盖上游 ground truth。
pub fn recalculate_all_costs<F: FnMut(u32)>(
    db: &crate::usage::UsageDb,
    mut progress: F,
) -> Result<u32> {
    const BATCH_SIZE: usize = 500;
    let conn = db.conn.lock();

    // 性能优化：先把 48 行 model_pricing 一次性 load 进 HashMap，
    // 循环里命中（O(1) HashMap）替代原本每行 query_row（O(N) SQLite 调用）。
    // 18420 行 recalculate 理论上从 18420 次 query 降到 48 次 + 18420 次 HashMap 命中。
    let mut pricing_map: HashMap<String, ModelPricing> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT model_id, input_cost_per_million, output_cost_per_million,
                        cache_read_cost_per_million, cache_creation_cost_per_million
                 FROM model_pricing",
            )
            .context("读取 model_pricing 失败")?;
        let rows = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let in_s: String = r.get(1)?;
                let out_s: String = r.get(2)?;
                let cr_s: String = r.get(3)?;
                let cw_s: String = r.get(4)?;
                let p = ModelPricing::from_strings(&in_s, &out_s, &cr_s, &cw_s).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        )),
                    )
                })?;
                Ok((id, p))
            })
            .context("迭代 model_pricing 失败")?;
        for row in rows {
            let (k, v) = row.context("读取 model_pricing 行失败")?;
            pricing_map.insert(k, v);
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT request_id, app_type, model, provider_id, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, cost_multiplier, created_at
             FROM request_logs
             WHERE cost_source = 'pricing_estimated'",
        )
        .context("读取 request_logs 失败")?;
    let mut rows = stmt.query([]).context("查询 request_logs 失败")?;

    // (request_id, provider_id, app_type, model, created_at, input_cost, output_cost, cache_read_cost, cache_creation_cost, total_cost)
    // 注：UPSERT 必须列全所有 NOT NULL 无 DEFAULT 列（当前：request_id, provider_id, app_type, model, created_at）；
    //     加 cost_*_usd 等有 DEFAULT 的列用 INSERT 部分默认值即可。
    //     改 schema 加新 NOT NULL 列务必同步这里 + 加 #[test] 回归测试（stats.rs:740 + session_common.rs:tests）。
    let mut pending: Vec<(String, String, String, String, String, String, String, String, String, String)> =
        Vec::with_capacity(BATCH_SIZE);
    let mut total = 0u32;

    while let Some(row) = rows.next().context("迭代 request_logs 失败")? {
        let request_id: String = row.get(0)?;
        let app_type: String = row.get(1)?;
        let model: String = row.get(2)?;
        let provider_id: String = row.get(3)?;
        let input_tokens: u32 = row.get(4)?;
        let output_tokens: u32 = row.get(5)?;
        let cache_read_tokens: u32 = row.get(6)?;
        let cache_creation_tokens: u32 = row.get(7)?;
        let multiplier_str: String = row.get(8)?;
        let created_at: i64 = row.get(9)?;

        // 优先 HashMap 命中；miss 时降级到 find_model_pricing 兜底（理论 0% miss）
        let pricing = pricing_map
            .get(&model)
            .cloned()
            .or_else(|| crate::usage::pricing::find_model_pricing(&conn, &model));
        let Some(pricing) = pricing else {
            continue; // 无定价则跳过（保持原值）
        };
        let multiplier = rust_decimal::Decimal::from_str(&multiplier_str)
            .unwrap_or(rust_decimal::Decimal::from(1));
        let usage = crate::usage::pricing::TokenUsage {
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            model: Some(model.clone()),
        };
        let costs = crate::usage::pricing::calculate_cost(
            &usage,
            &pricing,
            crate::usage::pricing::input_includes_cache_read(&app_type),
            multiplier,
        );

        pending.push((
            request_id,
            provider_id,
            app_type,
            model,
            created_at.to_string(),
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            costs.total_cost.to_string(),
        ));

        if pending.len() >= BATCH_SIZE {
            let n = flush_recalc_batch(&conn, &pending)?;
            total += n;
            progress(total);
            pending.clear();
        }
    }

    if !pending.is_empty() {
        let n = flush_recalc_batch(&conn, &pending)?;
        total += n;
        progress(total);
    }
    Ok(total)
}

/// 把一批重算结果通过 INSERT ... ON CONFLICT(request_id) DO UPDATE 一次性提交。
/// pending 元素顺序：(request_id, provider_id, app_type, model, created_at,
///                     input_cost, output_cost, cache_read_cost, cache_creation_cost, total_cost)
/// 注：必须显式补齐所有 NOT NULL 无 DEFAULT 列（schema 当前：provider_id / app_type / model / created_at），
///     否则 SQLite 报 constraint failed。改 schema 加新 NOT NULL 无 DEFAULT 列务必同步这里 + 加单测回归。
fn flush_recalc_batch(
    conn: &rusqlite::Connection,
    pending: &[(String, String, String, String, String, String, String, String, String, String)],
) -> Result<u32> {
    if pending.is_empty() {
        return Ok(0);
    }

    let n = pending.len();
    let placeholders = vec!["(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"; n].join(", ");
    let sql = format!(
        "INSERT INTO request_logs (
            request_id, provider_id, app_type, model, created_at,
            input_cost_usd, output_cost_usd, cache_read_cost_usd,
            cache_creation_cost_usd, total_cost_usd
         ) VALUES {placeholders}
         ON CONFLICT(request_id) DO UPDATE SET
            input_cost_usd = excluded.input_cost_usd,
            output_cost_usd = excluded.output_cost_usd,
            cache_read_cost_usd = excluded.cache_read_cost_usd,
            cache_creation_cost_usd = excluded.cache_creation_cost_usd,
            total_cost_usd = excluded.total_cost_usd"
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(n * 10);
    for p in pending {
        params.push(Box::new(p.0.clone()));
        params.push(Box::new(p.1.clone()));
        params.push(Box::new(p.2.clone()));
        params.push(Box::new(p.3.clone()));
        params.push(Box::new(p.4.clone()));
        params.push(Box::new(p.5.clone()));
        params.push(Box::new(p.6.clone()));
        params.push(Box::new(p.7.clone()));
        params.push(Box::new(p.8.clone()));
        params.push(Box::new(p.9.clone()));
    }

    let updated = conn
        .execute(
            &sql,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        )
        .context("批量 UPSERT request_logs 费用失败")?;
    Ok(updated as u32)
}

#[cfg(test)]
mod tests {
    //! 防 NOT NULL bug 回归测试。
    //!
    //! flush_recalc_batch 用 `INSERT ... ON CONFLICT(request_id) DO UPDATE` 一次性提交；
    //! INSERT 部分必须列全所有 NOT NULL 无 DEFAULT 列，否则 SQLite 报 constraint failed。
    //! 历史上漏过 provider_id / app_type / model（c005afb 修），再漏 created_at（c8662dc 修）——
    //! 这 4 个 test 强制覆盖 schema 变更后不能漏。
    //!
    //! 改 schema 加新 NOT NULL 无 DEFAULT 列时务必：
    //! 1. 更新 `flush_recalc_batch` INSERT 列 + SELECT + pending tuple
    //! 2. 在 test_flush_recalc_batch_upserts_with_all_not_null_columns 里加对应 INSERT 断言
    //! 3. 跑 `cargo test recalculate` 验证

    use super::*;
    use crate::usage::pricing::CostBreakdown;
    use crate::usage::UsageDb;
    use parking_lot::Mutex;
    use rust_decimal::Decimal;
    use std::sync::Arc;

    /// 与 stats.rs:557 setup_db 一致的最小 schema。
    /// 不直接用 init_schema 是因为它会跑 seed_default_pricing（污染测试断言）。
    /// 注意：只列 schema 必需的 NOT NULL 无 DEFAULT 列 —— 加新列时务必同步这里。
    fn setup_db() -> Arc<UsageDb> {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE request_logs (
                request_id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                model TEXT NOT NULL,
                request_model TEXT,
                pricing_model TEXT,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                input_token_semantics INTEGER NOT NULL DEFAULT 0,
                input_cost_usd TEXT NOT NULL DEFAULT '0',
                output_cost_usd TEXT NOT NULL DEFAULT '0',
                cache_read_cost_usd TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_usd TEXT NOT NULL DEFAULT '0',
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                latency_ms INTEGER NOT NULL DEFAULT 0,
                first_token_ms INTEGER,
                duration_ms INTEGER,
                status_code INTEGER NOT NULL DEFAULT 200,
                error_message TEXT,
                session_id TEXT,
                provider_type TEXT,
                is_streaming INTEGER NOT NULL DEFAULT 1,
                cost_multiplier TEXT NOT NULL DEFAULT '1.0',
                cost_source TEXT NOT NULL DEFAULT 'pricing_estimated',
                created_at INTEGER NOT NULL,
                data_source TEXT NOT NULL DEFAULT 'session'
            );
            CREATE TABLE model_pricing (
                model_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                input_cost_per_million TEXT NOT NULL,
                output_cost_per_million TEXT NOT NULL,
                cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
            );
            INSERT INTO model_pricing (model_id, display_name, input_cost_per_million, output_cost_per_million, cache_read_cost_per_million, cache_creation_cost_per_million)
            VALUES ('test-model', 'test-model', '10.0', '30.0', '1.0', '12.5');",
        )
        .unwrap();
        Arc::new(UsageDb {
            conn: Mutex::new(conn),
            recovered_backup: std::sync::Mutex::new(None),
        })
    }

    fn cost_zero() -> CostBreakdown {
        CostBreakdown {
            input_cost: Decimal::ZERO,
            output_cost: Decimal::ZERO,
            cache_read_cost: Decimal::ZERO,
            cache_creation_cost: Decimal::ZERO,
            total_cost: Decimal::ZERO,
        }
    }

    /// 防回归: recalculate 后 pricing_estimated 行的 cost 列必须按当前 model_pricing 重算
    #[test]
    fn test_recalculate_updates_pricing_estimated_rows() {
        let db = setup_db();
        {
            let conn = db.conn.lock();
            // 插一条 cost=0 的 pricing_estimated 行
            insert_request_log(
                &conn, "req-1", "opencode", "test-model",
                1000, 500, 0, 0,
                &cost_zero(), "0", "1.0",
                Some("sess-1"), 1700000000, 0, 200, None, "pricing_estimated",
            ).unwrap();
        }
        // 跑 recalculate（cb 简单：no-op 进度回调）
        let updated = recalculate_all_costs(&db, |_| {}).unwrap();
        assert_eq!(updated, 1);

        let conn = db.conn.lock();
        let total: String = conn.query_row(
            "SELECT total_cost_usd FROM request_logs WHERE request_id = 'req-1'",
            [], |r| r.get(0),
        ).unwrap();
        // 1000 input * 10/1M + 500 output * 30/1M = 0.01 + 0.015 = 0.025
        let parsed: f64 = total.parse().unwrap();
        assert!((parsed - 0.025).abs() < 1e-9, "total_cost_usd 应为 0.025, 实际 {parsed}");
    }

    /// 防回归: session_reported 行 recalculate 后 cost 列不变
    #[test]
    fn test_recalculate_skips_session_reported_rows() {
        let db = setup_db();
        {
            let conn = db.conn.lock();
            // 插一条 session_reported 行（cost = 9.999，模拟上游自报）
            insert_request_log(
                &conn, "req-sr", "opencode", "test-model",
                1000, 500, 0, 0,
                &cost_zero(), "9.999", "1.0",
                Some("sess-sr"), 1700000000, 0, 200, None, "session_reported",
            ).unwrap();
        }
        let _ = recalculate_all_costs(&db, |_| {}).unwrap();

        let conn = db.conn.lock();
        let total: String = conn.query_row(
            "SELECT total_cost_usd FROM request_logs WHERE request_id = 'req-sr'",
            [], |r| r.get(0),
        ).unwrap();
        // session_reported 不动: 仍为 9.999
        assert_eq!(total, "9.999", "session_reported 行 cost 不应被 recalculate 覆盖");
    }

    /// 防回归: cache_inclusive (codex/gemini/grokbuild) 用 billable_input = input - cache_read - cache_creation
    #[test]
    fn test_recalculate_handles_cache_inclusive_apps() {
        let db = setup_db();
        {
            let conn = db.conn.lock();
            // codex: input=1000, cache_read=200, cache_creation=100
            // 非 cache_inclusive (opencode/claude): billable_input = input = 1000
            // cache_inclusive (codex/gemini/grokbuild): billable_input = 1000 - 200 - 100 = 700
            insert_request_log(
                &conn, "req-open", "opencode", "test-model",
                1000, 500, 200, 100,
                &cost_zero(), "0", "1.0",
                Some("s1"), 1700000000, 0, 200, None, "pricing_estimated",
            ).unwrap();
            insert_request_log(
                &conn, "req-codex", "codex", "test-model",
                1000, 500, 200, 100,
                &cost_zero(), "0", "1.0",
                Some("s2"), 1700000000, 0, 200, None, "pricing_estimated",
            ).unwrap();
        }
        let _ = recalculate_all_costs(&db, |_| {}).unwrap();

        let conn = db.conn.lock();
        let total_open: String = conn.query_row(
            "SELECT total_cost_usd FROM request_logs WHERE request_id = 'req-open'",
            [], |r| r.get(0),
        ).unwrap();
        let total_codex: String = conn.query_row(
            "SELECT total_cost_usd FROM request_logs WHERE request_id = 'req-codex'",
            [], |r| r.get(0),
        ).unwrap();

        // opencode: 1000 input * 10/1M + 500 output * 30/1M + 200 cr * 1/1M + 100 cw * 12.5/1M
        //        = 0.010 + 0.015 + 0.0002 + 0.00125 = 0.02645
        let open_parsed: f64 = total_open.parse().unwrap();
        assert!((open_parsed - 0.02645).abs() < 1e-9, "opencode total 应为 0.02645, 实际 {open_parsed}");

        // codex (cache_inclusive): billable_input = 1000 - 200 - 100 = 700
        // 700 input * 10/1M + 500 output * 30/1M + 200 cr * 1/1M + 100 cw * 12.5/1M
        // = 0.007 + 0.015 + 0.0002 + 0.00125 = 0.02345
        let codex_parsed: f64 = total_codex.parse().unwrap();
        assert!((codex_parsed - 0.02345).abs() < 1e-9, "codex total 应为 0.02345, 实际 {codex_parsed}");
    }

    /// 防回归: flush_recalc_batch 必须能正确 UPSERT 所有 NOT NULL 无 DEFAULT 列
    /// 历史上漏过 provider_id/app_type/model（c005afb 修），再漏 created_at（c8662dc 修）。
    /// 这个测试断言 INSERT 后 provider_id/app_type/model/created_at 都正确保留。
    #[test]
    fn test_flush_recalc_batch_upserts_with_all_not_null_columns() {
        let db = setup_db();
        {
            let conn = db.conn.lock();
            insert_request_log(
                &conn, "req-upsert", "opencode", "test-model",
                100, 100, 0, 0,
                &cost_zero(), "0", "1.0",
                Some("sess-upsert"), 1700000000, 0, 200, None, "pricing_estimated",
            ).unwrap();
        }
        // 跑 recalculate,触发 flush_recalc_batch 的 INSERT...ON CONFLICT DO UPDATE
        let _ = recalculate_all_costs(&db, |_| {}).unwrap();

        let conn = db.conn.lock();
        // 断言 NOT NULL 列都正确（不被 None 覆盖、不被 NULL 破坏）
        let (provider_id, app_type, model, created_at, total_cost): (String, String, String, i64, String) =
            conn.query_row(
                "SELECT provider_id, app_type, model, created_at, total_cost_usd
                 FROM request_logs WHERE request_id = 'req-upsert'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            ).unwrap();
        assert_eq!(provider_id, "_opencode_session", "provider_id 应保留原值");
        assert_eq!(app_type, "opencode", "app_type 应保留原值");
        assert_eq!(model, "test-model", "model 应保留原值");
        assert_eq!(created_at, 1700000000, "created_at 应保留原值");
        // cost 已被 recalculate 重算: 100*10/1M + 100*30/1M = 0.001 + 0.003 = 0.004
        let cost_parsed: f64 = total_cost.parse().unwrap();
        assert!((cost_parsed - 0.004).abs() < 1e-9, "total_cost 应为 0.004, 实际 {cost_parsed}");
    }
}
