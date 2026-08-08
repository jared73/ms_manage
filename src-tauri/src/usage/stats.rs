use crate::usage::sql_helpers::fresh_input_sql;
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub request_count: u64,
    pub total_cost: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_tokens: u64,
    /// 缓存命中率 0.0-1.0（cache_read / 可缓存输入）
    pub cache_hit_rate: f64,
    /// 失败请求数（status != 200 或有错误消息）
    pub failed_count: u64,
    /// 失败率 0.0-1.0
    pub failure_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStats {
    pub app_type: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyStats {
    pub date: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub model: String,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogRow {
    pub request_id: String,
    pub app_type: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub total_cost_usd: String,
    pub created_at: i64,
    pub session_id: Option<String>,
    pub latency_ms: i64,
    pub status_code: i64,
    pub error_message: Option<String>,
}

/// 从 rollup 聚合（数据量大时避免全表扫明细）
pub fn get_usage_summary(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
) -> Result<UsageSummary> {
    let conn = db.conn.lock();
    let (req, cost, ti, to, cr, cc, tt, cache_hit_rate, failed) =
        crate::usage::rollup::rollup_summary(&conn, start_date, end_date, app_type)?;
    let failure_rate = if req > 0 { failed as f64 / req as f64 } else { 0.0 };
    Ok(UsageSummary {
        request_count: req,
        total_cost: format!("{:.4}", cost),
        total_input_tokens: ti,
        total_output_tokens: to,
        total_cache_read_tokens: cr,
        total_cache_creation_tokens: cc,
        total_tokens: tt,
        cache_hit_rate,
        failed_count: failed,
        failure_rate,
    })
}

pub fn get_provider_stats(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<ProviderStats>> {
    let conn = db.conn.lock();
    let rows = crate::usage::rollup::rollup_provider_stats(&conn, start_date, end_date)?;
    Ok(rows
        .into_iter()
        .map(|(app, req, cost, ti, to, cr, cc, tt)| ProviderStats {
            app_type: app,
            request_count: req,
            total_cost: format!("{:.4}", cost),
            total_tokens: tt,
            total_input_tokens: ti,
            total_output_tokens: to,
            total_cache_read_tokens: cr,
            total_cache_creation_tokens: cc,
        })
        .collect())
}

pub fn get_usage_trends(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: &str,
    app_type: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<DailyStats>> {
    // day 粒度走 rollup；hour 粒度 rollup 不支持，回退明细。
    // 注意：rollup 分支的 conn 锁必须在这个块结束前释放，否则 hour 分支
    // 再进 get_usage_trends_detail 会重入 lock 自死锁（parking_lot 不可重入）。
    {
        let conn = db.conn.lock();
        if let Some(rows) = crate::usage::rollup::rollup_trends(
            &conn, start_date, end_date, grouping, app_type, model,
        )? {
            return Ok(rows
                .into_iter()
                .map(|(date, req, cost, ti, to, cr, cc, tt)| DailyStats {
                    date,
                    request_count: req,
                    total_cost: format!("{:.4}", cost),
                    total_tokens: tt,
                    total_input_tokens: ti,
                    total_output_tokens: to,
                    total_cache_read_tokens: cr,
                    total_cache_creation_tokens: cc,
                })
                .collect());
        }
    }
    get_usage_trends_detail(&db, start_date, end_date, grouping, app_type, model)
}

/// 明细趋势（hour 粒度使用）
fn get_usage_trends_detail(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: &str,
    app_type: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<DailyStats>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    if let Some(a) = app_type {
        conditions.push("app_type = ?");
        params.push(Box::new(a.to_string()));
    }
    if let Some(m) = model {
        conditions.push("model = ?");
        params.push(Box::new(m.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let date_expr = match grouping {
        "hour" => "strftime('%Y-%m-%d %H:00', created_at, 'unixepoch', 'localtime')",
        _ => "date(created_at, 'unixepoch', 'localtime')",
    };

    let conn = db.conn.lock();
    let fresh_input = fresh_input_sql("");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {date_expr} AS bucket,
                    COUNT(*), COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                    COALESCE(SUM({fresh_input} + output_tokens + cache_creation_tokens + cache_read_tokens), 0),
                    COALESCE(SUM(input_tokens), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COALESCE(SUM(cache_creation_tokens), 0)
             FROM request_logs {where_clause}
             GROUP BY bucket ORDER BY bucket"
        ))
        .context("prepare trends 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })
        .context("query trends 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (date, req, cost, tokens, ti, to, cr, cc) = row.context("读取 trends 行失败")?;
        out.push(DailyStats {
            date,
            request_count: req as u64,
            total_cost: format!("{:.4}", cost),
            total_tokens: tokens as u64,
            total_input_tokens: ti as u64,
            total_output_tokens: to as u64,
            total_cache_read_tokens: cr as u64,
            total_cache_creation_tokens: cc as u64,
        });
    }
    Ok(out)
}

pub fn get_model_stats(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<ModelStats>> {
    let conn = db.conn.lock();
    let rows = crate::usage::rollup::rollup_model_stats(&conn, start_date, end_date)?;
    Ok(rows
        .into_iter()
        .map(|(model, req, tokens, cost)| ModelStats {
            model,
            request_count: req,
            total_tokens: tokens,
            total_cost: format!("{:.4}", cost),
        })
        .collect())
}

/// 数据源状态（路径 + 是否存在 + 最近同步时间 + 导入条数）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceStatus {
    pub app_type: String,
    pub label: String,
    pub path: String,
    pub exists: bool,
    pub last_synced_at: Option<i64>,
    pub imported_count: u64,
}

pub fn get_usage_data_sources(
    db: &UsageDb,
    overrides: &std::collections::HashMap<String, String>,
) -> Result<Vec<DataSourceStatus>> {
    use std::collections::HashMap;

    let paths = crate::usage::paths::UsagePaths::detect().apply_overrides(overrides);
    let app_paths: [(&str, &str, &std::path::Path); 5] = [
        ("opencode", "OpenCode", &paths.opencode_db),
        ("claude", "Claude Code", &paths.claude_projects),
        ("codex", "Codex", &paths.codex_sessions),
        ("gemini", "Gemini", &paths.gemini_dir),
        ("grokbuild", "Grok Build", &paths.grok_dir),
    ];
    let path_strs: Vec<String> = app_paths
        .iter()
        .map(|(_, _, p)| p.display().to_string())
        .collect();

    let conn = db.conn.lock();

    // 一次 GROUP BY 拿各 app_type 的已导入条数（替代 5 次 SELECT COUNT）
    let mut counts: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT app_type, COUNT(*) FROM request_logs GROUP BY app_type")
            .context("prepare data_sources counts 失败")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .context("query data_sources counts 失败")?;
        for row in rows {
            let (app, cnt) = row.context("读取 data_sources counts 行失败")?;
            counts.insert(app, cnt);
        }
    }

    // 一次全量读 session_log_sync（通常 < 几百行），在内存里按 path 匹配；
    // 替代 5 次 SELECT MAX(last_synced_at) WHERE file_path = ? 的 N+1
    let mut last_synced_by_path: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT file_path, last_synced_at FROM session_log_sync")
            .context("prepare data_sources session_log_sync 失败")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .context("query data_sources session_log_sync 失败")?;
        for row in rows {
            let (path, synced) = row.context("读取 session_log_sync 行失败")?;
            // opencode 写入时 key 是 "{db_path}:{session_id}"，MAX 会按 db_path 前缀覆盖到同一桶；
            // 此处按"以 path_str 为前缀"匹配取最新
            for ps in &path_strs {
                if path == *ps || path.starts_with(&format!("{ps}:")) {
                    let entry = last_synced_by_path.entry(ps.clone()).or_insert(0);
                    if synced > *entry {
                        *entry = synced;
                    }
                }
            }
        }
    }

    let mut out = Vec::with_capacity(app_paths.len());
    for (app, label, p) in app_paths.iter() {
        let path_str = p.display().to_string();
        out.push(DataSourceStatus {
            app_type: app.to_string(),
            label: label.to_string(),
            path: path_str.clone(),
            exists: p.exists(),
            last_synced_at: last_synced_by_path.get(&path_str).copied(),
            imported_count: counts.get(*app).copied().unwrap_or(0) as u64,
        });
    }
    Ok(out)
}

/// 导出请求日志 CSV（按过滤条件），列：时间,应用,模型,输入,输出,缓存读,缓存写,费用,状态,耗时
pub fn export_request_logs_csv(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
    model: Option<&str>,
) -> Result<String> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    if let Some(a) = app_type {
        conditions.push("app_type = ?");
        params.push(Box::new(a.to_string()));
    }
    if let Some(m) = model {
        conditions.push("model = ?");
        params.push(Box::new(m.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT created_at, app_type, model, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, total_cost_usd,
                    status_code, latency_ms, error_message
             FROM request_logs {where_clause}
             ORDER BY created_at ASC"
        ))
        .context("prepare export csv 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u32>(3)?,
                r.get::<_, u32>(4)?,
                r.get::<_, u32>(5)?,
                r.get::<_, u32>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, Option<String>>(10)?,
            ))
        })
        .context("query export csv 失败")?;

    let mut out = String::from("时间,应用,模型,输入Token,输出Token,缓存读,缓存写,费用,状态,耗时ms,错误信息\n");
    for row in rows {
        let (created_at, app, model, ti, to, cr, cc, cost, status, latency, error_msg) =
            row.context("读取 export 行失败")?;
        let time = chrono::DateTime::from_timestamp(created_at, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        out.push_str(&format!(
            "{time},{},{},{},{},{},{},{:.4},{},{},{}\n",
            csv_escape(&app),
            csv_escape(&model),
            ti,
            to,
            cr,
            cc,
            cost.parse::<f64>().unwrap_or(0.0),
            status,
            latency,
            csv_escape(&error_msg.unwrap_or_default()),
        ));
    }
    Ok(out)
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedLogs {
    pub rows: Vec<RequestLogRow>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

pub fn get_request_logs(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
    model: Option<&str>,
    page: u64,
    page_size: u64,
) -> Result<PaginatedLogs> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    if let Some(a) = app_type {
        conditions.push("app_type = ?");
        params.push(Box::new(a.to_string()));
    }
    if let Some(m) = model {
        conditions.push("model = ?");
        params.push(Box::new(m.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();

    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT request_id, app_type, model, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, total_cost_usd, created_at, session_id,
                    latency_ms, status_code, error_message,
                    COUNT(*) OVER() AS total_count
             FROM request_logs {where_clause}
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
        ))
        .context("prepare request_logs 失败")?;

    let mut all_params: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    all_params.push(&page_size);
    all_params.push(&offset);

    // 一次查询同时返回 rows + total（窗口函数 COUNT(*) OVER()），省一次 round-trip
    let mut rows: Vec<RequestLogRow> = Vec::new();
    let mut total: u64 = 0;
    let mut rows_iter = stmt
        .query_map(rusqlite::params_from_iter(all_params), |r| {
            Ok((
                RequestLogRow {
                    request_id: r.get(0).unwrap_or_default(),
                    app_type: r.get(1).unwrap_or_default(),
                    model: r.get(2).unwrap_or_default(),
                    input_tokens: r.get(3).unwrap_or(0),
                    output_tokens: r.get(4).unwrap_or(0),
                    cache_read_tokens: r.get(5).unwrap_or(0),
                    cache_creation_tokens: r.get(6).unwrap_or(0),
                    total_cost_usd: r.get(7).unwrap_or_else(|_| "0".to_string()),
                    created_at: r.get(8).unwrap_or(0),
                    session_id: r.get(9).ok(),
                    latency_ms: r.get(10).unwrap_or(0),
                    status_code: r.get(11).unwrap_or(200),
                    error_message: r.get(12).ok(),
                },
                r.get::<_, i64>(13).unwrap_or(0) as u64, // total_count（每行重复返回）
            ))
        })
        .context("query request_logs 失败")?;
    for row in rows_iter.by_ref() {
        let (r, t) = row.context("读取 request_logs 行失败")?;
        if total == 0 {
            total = t;
        }
        rows.push(r);
    }

    Ok(PaginatedLogs { rows, total, page, page_size })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::rollup::rebuild_rollups;
    use crate::usage::session_common::{insert_request_log, recalculate_all_costs};
    use crate::usage::UsageDb;
    use parking_lot::Mutex;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use std::sync::Arc;

    /// 构造与 db.rs::init_schema 一致的最小 schema + 用 session_common::insert_request_log 写入测试数据。
    /// 不直接用 init_schema 是因为测试 in-memory DB 不需要 PRAGMA 优化,
    /// 而且 init_schema 内部会跑 seed_default_pricing(污染测试断言)。
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
            CREATE TABLE usage_daily_rollups (
                date TEXT NOT NULL,
                app_type TEXT NOT NULL,
                model TEXT NOT NULL,
                request_count INTEGER NOT NULL DEFAULT 0,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                failed_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (date, app_type, model)
            );
            CREATE TABLE usage_hourly_rollups (
                date_hour TEXT NOT NULL,
                app_type TEXT NOT NULL,
                model TEXT NOT NULL,
                request_count INTEGER NOT NULL DEFAULT 0,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                failed_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (date_hour, app_type, model)
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
            VALUES ('test-model', 'test-model', '10.0', '30.0', '1.0', '12.5');
            CREATE TABLE app_config (
                app_type TEXT PRIMARY KEY,
                cost_multiplier TEXT NOT NULL DEFAULT '1.0',
                settle_window_seconds INTEGER NOT NULL DEFAULT 0,
                incomplete_tolerance_seconds INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE session_log_sync (
                file_path TEXT PRIMARY KEY,
                last_modified INTEGER NOT NULL,
                last_line_offset INTEGER NOT NULL DEFAULT 0,
                byte_offset INTEGER NOT NULL DEFAULT 0,
                last_processed_message_ts INTEGER NOT NULL DEFAULT 0,
                last_synced_at INTEGER NOT NULL
            );",
        )
        .unwrap();
        Arc::new(UsageDb {
            conn: Mutex::new(conn),
            recovered_backup: std::sync::Mutex::new(None),
        })
    }

    fn make_cost_zero() -> crate::usage::pricing::CostBreakdown {
        crate::usage::pricing::CostBreakdown {
            input_cost: Decimal::ZERO,
            output_cost: Decimal::ZERO,
            cache_read_cost: Decimal::ZERO,
            cache_creation_cost: Decimal::ZERO,
            total_cost: Decimal::ZERO,
        }
    }

    /// #46 #1: cache_hit_rate = cache_read_tokens / (input_tokens + cache_read_tokens + cache_creation_tokens)
    #[test]
    fn test_cache_hit_rate_calculation() {
        let db = setup_db();
        let conn = db.conn.lock();
        let cost = make_cost_zero();
        // input=900, cache_read=100, cache_creation=0 → hit_rate = 100/1000 = 10%
        let inserted = insert_request_log(
            &conn,
            "req-1",
            "claude",
            "test-model",
            900,
            100,
            100,
            0,
            &cost,
            "0",
            "1.0",
            Some("sess-1"),
            1700000000,
            0,
            200,
            None,
            "pricing_estimated",
        )
        .unwrap();
        assert!(inserted);

        drop(conn);
        rebuild_rollups(&db).unwrap();
        let s = get_usage_summary(&db, None, None, None).unwrap();
        assert_eq!(s.request_count, 1);
        assert_eq!(s.total_input_tokens, 900);
        assert_eq!(s.total_cache_read_tokens, 100);
        assert_eq!(s.total_cache_creation_tokens, 0);
        assert!(
            (s.cache_hit_rate - 0.10).abs() < 1e-9,
            "cache_hit_rate 应为 0.10, 实际 {}",
            s.cache_hit_rate
        );
    }

    /// #46 #2: failure_rate 与 failed_count 正确聚合
    #[test]
    fn test_failure_rate_aggregation() {
        let db = setup_db();
        let conn = db.conn.lock();
        let cost = make_cost_zero();
        // 3 个成功 + 1 个失败 → failure_rate = 0.25
        for (i, status) in [200i64, 200, 200, 0].iter().enumerate() {
            insert_request_log(
                &conn,
                &format!("req-{}", i),
                "opencode",
                "test-model",
                10,
                10,
                0,
                0,
                &cost,
                "0",
                "1.0",
                None,
                1700000000 + i as i64,
                0,
                *status,
                if *status != 200 { Some("fail") } else { None },
                "pricing_estimated",
            )
            .unwrap();
        }
        drop(conn);
        rebuild_rollups(&db).unwrap();
        let s = get_usage_summary(&db, None, None, None).unwrap();
        assert_eq!(s.request_count, 4);
        assert_eq!(s.failed_count, 1);
        assert!((s.failure_rate - 0.25).abs() < 1e-9);
    }

    /// #46 #3: recalculate_all_costs idempotency —— 跑两次结果不变
    #[test]
    fn test_recalculate_is_idempotent() {
        let db = setup_db();
        let conn = db.conn.lock();
        let cost = crate::usage::pricing::CostBreakdown {
            input_cost: Decimal::from_str("0.001").unwrap(),
            output_cost: Decimal::from_str("0.003").unwrap(),
            cache_read_cost: Decimal::ZERO,
            cache_creation_cost: Decimal::ZERO,
            total_cost: Decimal::from_str("0.004").unwrap(),
        };
        insert_request_log(
            &conn,
            "req-r1",
            "claude",
            "test-model",
            100,
            100,
            0,
            0,
            &cost,
            "0.004",
            "1.0",
            Some("sess"),
            1700000000,
            0,
            200,
            None,
            "pricing_estimated",
        )
        .unwrap();
        drop(conn);

        // 跑 recalculate 一次,确认 cost 仍按 model_pricing 的 test-model (input=10, output=30) 算
        let _ = recalculate_all_costs(&db, |_| {});
        let conn = db.conn.lock();
        let after: String = conn
            .query_row(
                "SELECT total_cost_usd FROM request_logs WHERE request_id='req-r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // 100 input * 10/1M + 100 output * 30/1M = 0.001 + 0.003 = 0.004
        assert!((after.parse::<f64>().unwrap() - 0.004).abs() < 1e-9);
        drop(conn);

        // 再跑一次,值不应变化
        let _ = recalculate_all_costs(&db, |_| {});
        let conn = db.conn.lock();
        let after2: String = conn
            .query_row(
                "SELECT total_cost_usd FROM request_logs WHERE request_id='req-r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, after2, "recalculate 应幂等");
    }

    /// #59: cost_source='session_reported' 不被 recalculate 覆盖(保护 opencode 自报 cost)
    #[test]
    fn test_recalculate_skips_session_reported_costs() {
        let db = setup_db();
        let conn = db.conn.lock();
        let original_cost = crate::usage::pricing::CostBreakdown {
            input_cost: Decimal::ZERO,
            output_cost: Decimal::ZERO,
            cache_read_cost: Decimal::ZERO,
            cache_creation_cost: Decimal::ZERO,
            total_cost: Decimal::from_str("9.999").unwrap(),
        };
        insert_request_log(
            &conn,
            "opencode-self-reported",
            "opencode",
            "test-model",
            100,
            100,
            0,
            0,
            &original_cost,
            "9.999",
            "1.0",
            Some("sess-1"),
            1700000000,
            0,
            200,
            None,
            "session_reported",
        )
        .unwrap();
        drop(conn);

        let _ = recalculate_all_costs(&db, |_| {});
        let conn = db.conn.lock();
        let cost: String = conn
            .query_row(
                "SELECT total_cost_usd FROM request_logs WHERE request_id='opencode-self-reported'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cost, "9.999", "session_reported 的 cost 不应被 recalculate 覆盖");
    }

    /// #46 #4: rebuild_rollups 按 (date, app_type, model) 正确聚合
    #[test]
    fn test_rollup_aggregates_correctly() {
        let db = setup_db();
        let conn = db.conn.lock();
        let cost = make_cost_zero();
        // 同一天同一 model 2 行
        for i in 0..2 {
            insert_request_log(
                &conn,
                &format!("req-{}", i),
                "claude",
                "test-model",
                50,
                30,
                0,
                0,
                &cost,
                "0.001",
                "1.0",
                None,
                1700000000 + i,
                0,
                200,
                None,
                "pricing_estimated",
            )
            .unwrap();
        }
        drop(conn);

        rebuild_rollups(&db).unwrap();
        let conn = db.conn.lock();
        let (req_count, cost_sum): (i64, f64) = conn
            .query_row(
                "SELECT request_count, CAST(total_cost_usd AS REAL)
                 FROM usage_daily_rollups WHERE app_type='claude' AND model='test-model'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(req_count, 2);
        assert!((cost_sum - 0.002).abs() < 1e-9);
    }

    /// #33 byte_offset: get_sync_state 返回 byte_offset
    #[test]
    fn test_get_sync_state_returns_byte_offset() {
        let db = setup_db();
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO session_log_sync (file_path, last_modified, last_line_offset, byte_offset, last_processed_message_ts, last_synced_at)
             VALUES ('/tmp/test.jsonl', 100, 5, 1024, 0, 100)",
            [],
        )
        .unwrap();
        drop(conn);
        let (last_modified, line_offset, byte_offset, msg_ts) =
            crate::usage::session_common::get_sync_state(&db, "/tmp/test.jsonl").unwrap();
        assert_eq!(last_modified, 100);
        assert_eq!(line_offset, 5);
        assert_eq!(byte_offset, 1024);
        assert_eq!(msg_ts, 0);
    }

    /// #50 CSV 含 error_message 列
    #[test]
    fn test_export_csv_includes_error_column() {
        let db = setup_db();
        let conn = db.conn.lock();
        let cost = make_cost_zero();
        insert_request_log(
            &conn,
            "err-1",
            "claude",
            "test-model",
            100,
            100,
            0,
            0,
            &cost,
            "0.004",
            "1.0",
            None,
            1700000000,
            0,
            500,
            Some("upstream error: rate limit"),
            "pricing_estimated",
        )
        .unwrap();
        drop(conn);

        let csv = export_request_logs_csv(&db, None, None, None, None).unwrap();
        // header 含错误信息列
        assert!(csv.contains("错误信息"));
        // 数据行含错误消息
        assert!(csv.contains("upstream error: rate limit"));
    }
}
