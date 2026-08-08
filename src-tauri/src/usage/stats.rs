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
) -> Result<Vec<DailyStats>> {
    // day 粒度走 rollup；hour 粒度 rollup 不支持，回退明细。
    // 注意：rollup 分支的 conn 锁必须在这个块结束前释放，否则 hour 分支
    // 再进 get_usage_trends_detail 会重入 lock 自死锁（parking_lot 不可重入）。
    {
        let conn = db.conn.lock();
        if let Some(rows) =
            crate::usage::rollup::rollup_trends(&conn, start_date, end_date, grouping, app_type)?
        {
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
    get_usage_trends_detail(&db, start_date, end_date, grouping, app_type)
}

/// 明细趋势（hour 粒度使用）
fn get_usage_trends_detail(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    grouping: &str,
    app_type: Option<&str>,
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
    let paths = crate::usage::paths::UsagePaths::detect().apply_overrides(overrides);
    let conn = db.conn.lock();
    let mut out = Vec::new();
    for (app, label, p) in [
        ("opencode", "OpenCode", &paths.opencode_db),
        ("claude", "Claude Code", &paths.claude_projects),
        ("codex", "Codex", &paths.codex_sessions),
        ("gemini", "Gemini", &paths.gemini_dir),
        ("grokbuild", "Grok Build", &paths.grok_dir),
    ] {
        let path_str = p.display().to_string();
        let last_synced_at: Option<i64> = conn
            .query_row(
                "SELECT MAX(last_synced_at) FROM session_log_sync WHERE file_path = ?1",
                [&path_str],
                |r| r.get(0),
            )
            .unwrap_or(None);
        let imported_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_logs WHERE app_type = ?1",
                [app],
                |r| r.get(0),
            )
            .unwrap_or(0);
        out.push(DataSourceStatus {
            app_type: app.to_string(),
            label: label.to_string(),
            path: path_str,
            exists: p.exists(),
            last_synced_at,
            imported_count: imported_count as u64,
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
                    status_code, latency_ms
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
            ))
        })
        .context("query export csv 失败")?;

    let mut out = String::from("时间,应用,模型,输入Token,输出Token,缓存读,缓存写,费用,状态,耗时ms\n");
    for row in rows {
        let (created_at, app, model, ti, to, cr, cc, cost, status, latency) =
            row.context("读取 export 行失败")?;
        let time = chrono::DateTime::from_timestamp(created_at, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        out.push_str(&format!(
            "{time},{},{},{},{},{},{},{:.4},{},{}\n",
            csv_escape(&app),
            csv_escape(&model),
            ti,
            to,
            cr,
            cc,
            cost.parse::<f64>().unwrap_or(0.0),
            status,
            latency
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
    let total: u64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM request_logs {where_clause}"),
            rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)),
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as u64;

    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT request_id, app_type, model, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, total_cost_usd, created_at, session_id,
                    latency_ms, status_code, error_message
             FROM request_logs {where_clause}
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
        ))
        .context("prepare request_logs 失败")?;

    let mut all_params: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    all_params.push(&page_size);
    all_params.push(&offset);

    let rows = stmt
        .query_map(rusqlite::params_from_iter(all_params), |r| {
            Ok(RequestLogRow {
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
            })
        })
        .context("query request_logs 失败")?
        .filter_map(Result::ok)
        .collect();

    Ok(PaginatedLogs { rows, total, page, page_size })
}
