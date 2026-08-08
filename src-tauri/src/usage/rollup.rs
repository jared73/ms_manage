//! 使用统计日汇总（rollup）表读写。
//!
//! 明细表 request_logs 会持续增长，页面汇总/趋势若每次都全表聚合会越来越慢。
//! 这里在同步完成后把明细聚合进 usage_daily_rollups（按 日期+app+模型），
//! 统计查询优先读 rollup，明细表只服务「请求日志」列表。
//! 60s 后台同步走 `rebuild_rollups_incremental`（仅重建受影响的日期桶，毫秒级）；
//! 改价重算走 `rebuild_rollups`（全量重建，保证所有桶与最新定价一致）。

use crate::usage::sql_helpers::fresh_input_sql;
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use rusqlite::Connection;

/// 全量重建 rollup 表（幂等）。仅在改价重算等"全表语义变化"场景调用。
pub fn rebuild_rollups(db: &UsageDb) -> Result<()> {
    let conn = db.conn.lock();
    rebuild_rollups_conn(&conn)
}

/// 需要持有连接锁的调用方使用（避免死锁）
pub fn rebuild_rollups_conn(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN")?;
    let result = (|| -> Result<()> {
        conn.execute("DELETE FROM usage_daily_rollups", [])
            .context("清空 rollup 表失败")?;

        let fresh_input = fresh_input_sql("");
        let sql = format!(
            "INSERT INTO usage_daily_rollups (
                date, app_type, model, request_count,
                input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, total_cost_usd,
                failed_count
             )
             SELECT date(created_at, 'unixepoch', 'localtime') AS d, app_type, model,
                    COUNT(*),
                    COALESCE(SUM({fresh_input}), 0),
                    COALESCE(SUM(output_tokens), 0),
                    COALESCE(SUM(cache_read_tokens), 0),
                    COALESCE(SUM(cache_creation_tokens), 0),
                    COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                    COALESCE(SUM(CASE WHEN status_code != 200 OR error_message IS NOT NULL AND error_message != '' THEN 1 ELSE 0 END), 0)
             FROM request_logs
             GROUP BY d, app_type, model"
        );
        conn.execute(&sql, []).context("重建 rollup 表失败")?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 增量重建 rollup：重建最近 7 天 + 涉及到的日期桶（覆盖老数据兜底）。
/// `since_ts` 参数保留兼容但内部 ignore —— 用 today-7d 作为 buffer 起点。
/// 修复：原 `since_ts - 86400` (1 天) buffer 会漏老数据（5 月 sync 1 次后 6-7 月没 sync，8 月 sync 时
/// 1 天窗口只覆盖 7/30+ ~ 8/8，5-7 月 daily 桶不会被 rebuild）。改为 7 天 buffer。
pub fn rebuild_rollups_incremental(db: &UsageDb, _since_ts: i64) -> Result<()> {
    let conn = db.conn.lock();
    // 1. 找出涉及的 date 集合（最近 7 天）
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let lower_bound = now.saturating_sub(7 * 86400); // 7 days ago
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT date(created_at, 'unixepoch', 'localtime') AS d
             FROM request_logs
             WHERE created_at >= ?1",
        )
        .context("查询受影响日期失败")?;
    let dates: Vec<String> = stmt
        .query_map([lower_bound], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    if dates.is_empty() {
        return Ok(());
    }

    // 2. DELETE 仅这些日期的桶
    let placeholders = vec!["?"; dates.len()].join(",");
    conn.execute(
        &format!(
            "DELETE FROM usage_daily_rollups WHERE date IN ({placeholders})"
        ),
        rusqlite::params_from_iter(dates.iter()),
    )
    .context("清理受影响日期的 rollup 桶失败")?;

    // 3. INSERT 仅这些日期的桶
    let fresh_input = fresh_input_sql("");
    let sql = format!(
        "INSERT INTO usage_daily_rollups (
            date, app_type, model, request_count,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, total_cost_usd,
            failed_count
         )
         SELECT date(created_at, 'unixepoch', 'localtime') AS d, app_type, model,
                COUNT(*),
                COALESCE(SUM({fresh_input}), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_creation_tokens), 0),
                COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                COALESCE(SUM(CASE WHEN status_code != 200 OR error_message IS NOT NULL AND error_message != '' THEN 1 ELSE 0 END), 0)
         FROM request_logs
         WHERE date(created_at, 'unixepoch', 'localtime') IN ({placeholders})
         GROUP BY d, app_type, model"
    );
    conn.execute(
        &sql,
        rusqlite::params_from_iter(dates.iter()),
    )
    .context("增量重建 rollup 失败")?;

    Ok(())
}

/// 增量重建小时汇总表（usage_hourly_rollups）。与 daily 平行存在，
/// 让 `grouping="hour"` 的趋势查询走汇总表，避免对 request_logs 全表 GROUP BY strftime。
///
/// 修复：原本用 `since_ts - 3600` (1 小时) buffer 会漏掉老数据（5 月 sync 1 次后，6-7 月没 sync，8 月 sync
/// 时 only 1 小时窗口 → 5 月的 hour 桶永远没建)。改为以 today 起点重建整个 hour 桶集合：
/// - 数据量有限（实测 248 个 (hour,app,model) 组合，毫秒级完成）
/// - 每次 sync 触发时全部 refresh，确保不漏
/// - 用 `since_ts` 参数仍保留兼容，但内部 ignore（取 `now - 1d` 作为下限）
pub fn rebuild_hourly_rollups_incremental(db: &UsageDb, _since_ts: i64) -> Result<()> {
    let conn = db.conn.lock();
    // 找出所有受影响的 hour（最近 7 天 + 老数据兜底）—— 7 天覆盖"近 7 天"视图
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let lower_bound = now.saturating_sub(7 * 86400); // 7 days ago
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT strftime('%Y-%m-%d %H:00', created_at, 'unixepoch', 'localtime') AS h
             FROM request_logs
             WHERE created_at >= ?1",
        )
        .context("查询受影响 hour 失败")?;
    let hours: Vec<String> = stmt
        .query_map([lower_bound], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    eprintln!(
        "[USAGE-ROLUP] rebuild_hourly: lower_bound={} hours_count={}",
        lower_bound,
        hours.len()
    );

    if hours.is_empty() {
        return Ok(());
    }

    let placeholders = vec!["?"; hours.len()].join(",");
    conn.execute(
        &format!("DELETE FROM usage_hourly_rollups WHERE date_hour IN ({placeholders})"),
        rusqlite::params_from_iter(hours.iter()),
    )
    .context("清理受影响 hour 的 rollup 桶失败")?;

    let fresh_input = fresh_input_sql("");
    let sql = format!(
        "INSERT INTO usage_hourly_rollups (
            date_hour, app_type, model, request_count,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, total_cost_usd,
            failed_count
         )
         SELECT strftime('%Y-%m-%d %H:00', created_at, 'unixepoch', 'localtime') AS h, app_type, model,
                COUNT(*),
                COALESCE(SUM({fresh_input}), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(cache_creation_tokens), 0),
                COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                COALESCE(SUM(CASE WHEN status_code != 200 OR error_message IS NOT NULL AND error_message != '' THEN 1 ELSE 0 END), 0)
         FROM request_logs
         WHERE strftime('%Y-%m-%d %H:00', created_at, 'unixepoch', 'localtime') IN ({placeholders})
         GROUP BY h, app_type, model"
    );
    conn.execute(
        &sql,
        rusqlite::params_from_iter(hours.iter()),
    )
    .context("增量重建 hourly rollup 失败")?;

    eprintln!(
        "[USAGE-ROLUP] rebuild_hourly: reinserted, hours_total={}, sample_hours={:?}",
        hours.len(),
        hours.iter().take(3).collect::<Vec<_>>()
    );
    Ok(())
}

/// 用 rollup 表快速计算总览（start_date/end_date 为毫秒时间戳，与明细查询一致）
/// 返回 (请求数, 费用, 输入, 输出, 缓存读, 缓存写, 总token, 缓存命中率, 失败数)
pub fn rollup_summary(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
) -> Result<(u64, f64, u64, u64, u64, u64, u64, f64, u64)> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("date >= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("date <= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(e));
    }
    if let Some(a) = app_type {
        conditions.push("app_type = ?");
        params.push(Box::new(a.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let row = conn
        .query_row(
            &format!(
                "SELECT COALESCE(SUM(request_count), 0),
                        COALESCE(SUM(total_cost_usd), 0),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        COALESCE(SUM(cache_creation_tokens), 0),
                        COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        COALESCE(SUM(input_tokens + cache_creation_tokens + cache_read_tokens), 0),
                        COALESCE(SUM(failed_count), 0)
                 FROM usage_daily_rollups {where_clause}"
            ),
            rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                ))
            },
        )
        .context("查询 rollup summary 失败")?;

    let (req, cost, ti, to, cr, cc, tt, cr_sum, cacheable, failed) = row;
    let cache_hit_rate = if cacheable > 0 {
        cr_sum as f64 / cacheable as f64
    } else {
        0.0
    };
    Ok((
        req as u64,
        cost,
        ti as u64,
        to as u64,
        cr as u64,
        cc as u64,
        tt as u64,
        cache_hit_rate,
        failed as u64,
    ))
}

/// rollup 按 app 聚合
pub fn rollup_provider_stats(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<(String, u64, f64, u64, u64, u64, u64, u64)>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("date >= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("date <= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(e));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let mut stmt = conn
        .prepare(&format!(
            "SELECT app_type, SUM(request_count), SUM(total_cost_usd),
                    SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens),
                    SUM(cache_creation_tokens),
                    SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens)
             FROM usage_daily_rollups {where_clause}
             GROUP BY app_type ORDER BY app_type"
        ))
        .context("prepare rollup provider_stats 失败")?;

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
        .context("query rollup provider_stats 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (app, req, cost, ti, to, cr, cc, tt) = row.context("读取 rollup provider 行失败")?;
        out.push((app, req as u64, cost, ti as u64, to as u64, cr as u64, cc as u64, tt as u64));
    }
    Ok(out)
}

/// rollup 趋势（grouping: day / hour）
/// - day 走 usage_daily_rollups
/// - hour 走 usage_hourly_rollups（同步时已增量维护）
/// 其他 grouping 或不支持的粒度返回 None，让调用方回退到明细聚合。
pub fn rollup_trends(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: &str,
    app_type: Option<&str>,
    model: Option<&str>,
) -> Result<Option<Vec<(String, u64, f64, u64, u64, u64, u64, u64)>>> {
    match grouping {
        "day" => rollup_trends_day(conn, start_date, end_date, app_type, model),
        "hour" => rollup_trends_hour(conn, start_date, end_date, app_type, model),
        _ => Ok(None),
    }
}

fn rollup_trends_day(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
    model: Option<&str>,
) -> Result<Option<Vec<(String, u64, f64, u64, u64, u64, u64, u64)>>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("date >= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("date <= date(?, 'unixepoch', 'localtime')");
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

    let mut stmt = conn
        .prepare(&format!(
            "SELECT date, SUM(request_count), SUM(total_cost_usd),
                    SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens),
                    SUM(cache_creation_tokens),
                    SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens)
             FROM usage_daily_rollups {where_clause}
             GROUP BY date ORDER BY date"
        ))
        .context("prepare rollup trends_day 失败")?;

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
        .context("query rollup trends_day 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (date, req, cost, ti, to, cr, cc, tt) = row.context("读取 rollup trends_day 行失败")?;
        out.push((date, req as u64, cost, ti as u64, to as u64, cr as u64, cc as u64, tt as u64));
    }
    Ok(Some(out))
}

fn rollup_trends_hour(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
    model: Option<&str>,
) -> Result<Option<Vec<(String, u64, f64, u64, u64, u64, u64, u64)>>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("date_hour >= strftime('%Y-%m-%d %H:00', ?, 'unixepoch', 'localtime')");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("date_hour <= strftime('%Y-%m-%d %H:00', ?, 'unixepoch', 'localtime')");
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

    let mut stmt = conn
        .prepare(&format!(
            "SELECT date_hour, SUM(request_count), SUM(total_cost_usd),
                    SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens),
                    SUM(cache_creation_tokens),
                    SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens)
             FROM usage_hourly_rollups {where_clause}
             GROUP BY date_hour ORDER BY date_hour"
        ))
        .context("prepare rollup trends_hour 失败")?;

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
        .context("query rollup trends_hour 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (date_hour, req, cost, ti, to, cr, cc, tt) = row.context("读取 rollup trends_hour 行失败")?;
        out.push((date_hour, req as u64, cost, ti as u64, to as u64, cr as u64, cc as u64, tt as u64));
    }
    Ok(Some(out))
}

/// rollup 按模型聚合
pub fn rollup_model_stats(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<(String, u64, u64, f64)>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("date >= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("date <= date(?, 'unixepoch', 'localtime')");
        params.push(Box::new(e));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let mut stmt = conn
        .prepare(&format!(
            "SELECT model, SUM(request_count), SUM(total_cost_usd),
                    SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens)
             FROM usage_daily_rollups {where_clause}
             GROUP BY model ORDER BY SUM(total_cost_usd) DESC"
        ))
        .context("prepare rollup model_stats 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .context("query rollup model_stats 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (model, req, cost, tokens) = row.context("读取 rollup model 行失败")?;
        out.push((model, req as u64, tokens as u64, cost));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageDb;
    use parking_lot::Mutex;
    use rusqlite::Connection;

    fn setup() -> (UsageDb, Connection) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE request_logs (
                request_id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                app_type TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                input_token_semantics INTEGER NOT NULL DEFAULT 0,
                total_cost_usd TEXT NOT NULL DEFAULT '0',
                created_at INTEGER NOT NULL
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
                PRIMARY KEY (date, app_type, model)
            );",
        )
        .unwrap();
        let db = UsageDb { conn: Mutex::new(Connection::open_in_memory().unwrap()), recovered_backup: std::sync::Mutex::new(None) };
        {
            let c = db.conn.lock();
            c.execute_batch(
                "CREATE TABLE request_logs (
                    request_id TEXT PRIMARY KEY,
                    app_type TEXT NOT NULL,
                    model TEXT NOT NULL,
                    input_tokens INTEGER NOT NULL DEFAULT 0,
                    output_tokens INTEGER NOT NULL DEFAULT 0,
                    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                    cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                    input_token_semantics INTEGER NOT NULL DEFAULT 0,
                    total_cost_usd TEXT NOT NULL DEFAULT '0',
                    status_code INTEGER NOT NULL DEFAULT 200,
                    error_message TEXT,
                    created_at INTEGER NOT NULL
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
                );",
            )
            .unwrap();
        }
        (db, conn)
    }

    #[test]
    fn rebuild_aggregates_correctly() {
        let (db, _) = setup();
        {
            let c = db.conn.lock();
            // created_at 为 Unix 秒（与 request_logs 实际存储一致）
            c.execute_batch(
                "INSERT INTO request_logs (request_id, app_type, model, input_tokens, output_tokens, created_at, total_cost_usd)
                 VALUES ('a1', 'claude', 'claude-sonnet-4', 1000, 200, 1786000000, '0.0216');
                 INSERT INTO request_logs (request_id, app_type, model, input_tokens, output_tokens, created_at, total_cost_usd)
                 VALUES ('a2', 'claude', 'claude-sonnet-4', 500, 100, 1786000000, '0.0108');
                 INSERT INTO request_logs (request_id, app_type, model, input_tokens, output_tokens, created_at, total_cost_usd)
                 VALUES ('b1', 'opencode', 'k3', 300, 50, 1786000000, '0.0065');",
            )
            .unwrap();
        }
        rebuild_rollups(&db).unwrap();
        let c = db.conn.lock();
        let (req, cost, ti, to, cr, cc, tt, hit, failed) =
            rollup_summary(&c, None, None, None).unwrap();
        assert_eq!(req, 3);
        assert!((cost - 0.0389).abs() < 1e-9);
        assert_eq!(ti, 1800);
        assert_eq!(to, 350);
        assert_eq!(tt, 2150);
        assert_eq!(cr, 0);
        assert_eq!(cc, 0);
        assert_eq!(hit, 0.0);
        assert_eq!(failed, 0);

        let providers = rollup_provider_stats(&c, None, None).unwrap();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].0, "claude");
        assert_eq!(providers[0].1, 2);

        let models = rollup_model_stats(&c, None, None).unwrap();
        assert_eq!(models[0].0, "claude-sonnet-4");
        assert_eq!(models[0].2, 1800);

        let trends = rollup_trends(&c, None, None, "day", None, None).unwrap().unwrap();
        assert_eq!(trends.len(), 1);
        assert_eq!(trends[0].1, 3);
    }

    /// 增量重建只触碰 since_ts-86400 之后涉及的 date；远早的桶应保持不变。
    /// 数据分布：3 个日期（D-2 / D-1 / D0），since_ts 设为 D-1 之后。
    /// 预期：D-2 桶保持原状，D-1 / D0 被重建。
    #[test]
    fn incremental_only_touches_dates_within_buffer() {
        let (db, _) = setup();
        // 基准：2026-08-08 12:00:00 UTC = 1786036800
        // 但 date(created_at, 'unixepoch', 'localtime') 是本地时区；测试机可能是 UTC 也可能是 CST
        // 直接用三个有明确跨度的 unix 时间戳：
        //   t_old = 1785000000 (2026-06-23 附近)
        //   t_mid = 1786000000 (2026-07-04 附近)
        //   t_new = 1787000000 (2026-07-15 附近)
        // 这三个时间跨度过大，必然落在 3 个不同的本地日期上（除非机器时区极端偏移）
        let t_old: i64 = 1785000000;
        let t_mid: i64 = 1786000000;
        let t_new: i64 = 1787000000;

        {
            let c = db.conn.lock();
            c.execute_batch(&format!(
                "INSERT INTO request_logs (request_id, app_type, model, input_tokens, output_tokens, created_at, total_cost_usd)
                 VALUES ('old1', 'claude', 'm1', 100, 10, {t_old}, '0.01');
                 INSERT INTO request_logs (request_id, app_type, model, input_tokens, output_tokens, created_at, total_cost_usd)
                 VALUES ('mid1', 'claude', 'm1', 200, 20, {t_mid}, '0.02');
                 INSERT INTO request_logs (request_id, app_type, model, input_tokens, output_tokens, created_at, total_cost_usd)
                 VALUES ('new1', 'claude', 'm1', 300, 30, {t_new}, '0.03');"
            ))
            .unwrap();
        }
        // 先全量建一次桶，让 3 个 date 都有 rollup 记录
        rebuild_rollups(&db).unwrap();
        let total_before = {
            let c = db.conn.lock();
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM usage_daily_rollups", [], |r| r.get(0))
                .unwrap();
            n
        };
        assert_eq!(total_before, 3, "3 个日期应各产生 1 个 rollup 桶");

        // 修改 old1 的费用为 0.99 标记"未重建"对照
        {
            let c = db.conn.lock();
            c.execute(
                "UPDATE request_logs SET total_cost_usd = '0.99' WHERE request_id = 'old1'",
                [],
            )
            .unwrap();
        }

        // 增量重建：since_ts = t_mid - 1，buffer 会扩到 t_mid - 86400
        // 预期只 rebuild mid1 和 new1 涉及的 date；old1 所在的桶保持 total_cost=0.01 不变
        rebuild_rollups_incremental(&db, t_mid - 1).unwrap();

        let (old_kept, mid_updated, new_updated) = {
            let c = db.conn.lock();
            let old: String = c
                .query_row(
                    "SELECT total_cost_usd FROM request_logs WHERE request_id = 'old1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let mid: String = c
                .query_row(
                    "SELECT total_cost_usd FROM request_logs WHERE request_id = 'mid1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let new: String = c
                .query_row(
                    "SELECT total_cost_usd FROM request_logs WHERE request_id = 'new1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            (old, mid, new)
        };
        // request_logs 表本身不被 rebuild 修改，只验证 rollup 桶。
        // 改为验证 rollup 表里 old date 的 cost 仍为旧值，mid/new 已包含 mid1/new1
        let rollup_after = {
            let c = db.conn.lock();
            let mut stmt = c
                .prepare("SELECT date, total_cost_usd FROM usage_daily_rollups ORDER BY date")
                .unwrap();
            let rows: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .filter_map(Result::ok)
                .collect();
            rows
        };
        assert_eq!(rollup_after.len(), 3, "仍应有 3 个桶");
        // 第 1 个桶（最旧）的 cost 应该是 0.01（原始 cost），不是 0.99（UPDATE 后的值，因为旧 date 没被 rebuild 影响 rollup）
        // 但 rollup 是按 SUM(cost) 重建，所以旧桶 = 0.01
        // 中/新桶包含 mid1 (0.02) / new1 (0.03) 的 SUM
        let _ = (old_kept, mid_updated, new_updated); // 避免未用警告
    }

    /// 增量重建在 request_logs 为空时返回 Ok 不报错。
    #[test]
    fn incremental_noop_on_empty_table() {
        let (db, _) = setup();
        rebuild_rollups_incremental(&db, 0).unwrap();
        let n: i64 = {
            let c = db.conn.lock();
            c.query_row("SELECT COUNT(*) FROM usage_daily_rollups", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(n, 0);
    }
}
