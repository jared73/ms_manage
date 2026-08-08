//! 使用统计日汇总（rollup）表读写。
//!
//! 明细表 request_logs 会持续增长，页面汇总/趋势若每次都全表聚合会越来越慢。
//! 这里在同步完成后把明细聚合进 usage_daily_rollups（按 日期+app+模型），
//! 统计查询优先读 rollup，明细表只服务「请求日志」列表。
//! 数据量不大时直接全量重建（DELETE + INSERT），保证与明细一致，不做增量。

use crate::usage::sql_helpers::fresh_input_sql;
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use rusqlite::Connection;

/// 全量重建 rollup 表（幂等）。在同步完成、改价重算后调用。
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

/// rollup 趋势（grouping: day / hour；hour 粒度 rollup 表只有天，回退到明细聚合）
pub fn rollup_trends(
    conn: &Connection,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: &str,
    app_type: Option<&str>,
) -> Result<Option<Vec<(String, u64, f64, u64, u64, u64, u64, u64)>>> {
    if grouping != "day" {
        return Ok(None); // hour 粒度走明细
    }
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

    let mut stmt = conn
        .prepare(&format!(
            "SELECT date, SUM(request_count), SUM(total_cost_usd),
                    SUM(input_tokens), SUM(output_tokens), SUM(cache_read_tokens),
                    SUM(cache_creation_tokens),
                    SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens)
             FROM usage_daily_rollups {where_clause}
             GROUP BY date ORDER BY date"
        ))
        .context("prepare rollup trends 失败")?;

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
        .context("query rollup trends 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (date, req, cost, ti, to, cr, cc, tt) = row.context("读取 rollup trends 行失败")?;
        out.push((date, req as u64, cost, ti as u64, to as u64, cr as u64, cc as u64, tt as u64));
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
        let db = UsageDb { conn: Mutex::new(Connection::open_in_memory().unwrap()) };
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

        let trends = rollup_trends(&c, None, None, "day", None).unwrap().unwrap();
        assert_eq!(trends.len(), 1);
        assert_eq!(trends[0].1, 3);
    }
}
