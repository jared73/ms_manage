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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummaryByApp {
    pub app_type: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyStats {
    pub date: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
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
}

/// 从明细表聚合（数据量小直接聚合，不依赖 rollup 表）
pub fn get_usage_summary(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
) -> Result<UsageSummary> {
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

    let conn = db.conn.lock();
    let row = conn
        .query_row(
            &format!(
                "SELECT COUNT(*),
                        COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        COALESCE(SUM(cache_creation_tokens), 0)
                 FROM request_logs {where_clause}"
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
                ))
            },
        )
        .context("查询 usage summary 失败")?;

    let (req, cost, ti, to, cr, cc) = row;
    Ok(UsageSummary {
        request_count: req as u64,
        total_cost: format!("{:.6}", cost),
        total_input_tokens: ti as u64,
        total_output_tokens: to as u64,
        total_cache_read_tokens: cr as u64,
        total_cache_creation_tokens: cc as u64,
    })
}

pub fn get_usage_summary_by_app(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<UsageSummaryByApp>> {
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
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT app_type, COUNT(*), COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                    COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0)
             FROM request_logs {where_clause}
             GROUP BY app_type ORDER BY app_type"
        ))
        .context("prepare by_app 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .context("query by_app 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (app, req, cost, tokens) = row.context("读取 by_app 行失败")?;
        out.push(UsageSummaryByApp {
            app_type: app,
            request_count: req as u64,
            total_cost: format!("{:.6}", cost),
            total_tokens: tokens as u64,
        });
    }
    Ok(out)
}

pub fn get_usage_trends(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
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
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT date(created_at, 'unixepoch', 'localtime') as day,
                    COUNT(*), COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                    COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0)
             FROM request_logs {where_clause}
             GROUP BY day ORDER BY day"
        ))
        .context("prepare trends 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .context("query trends 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (date, req, cost, tokens) = row.context("读取 trends 行失败")?;
        out.push(DailyStats {
            date,
            request_count: req as u64,
            total_cost: format!("{:.6}", cost),
            total_tokens: tokens as u64,
        });
    }
    Ok(out)
}

pub fn get_model_stats(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<ModelStats>> {
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
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT model, COUNT(*),
                    COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0),
                    COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0)
             FROM request_logs {where_clause}
             GROUP BY model ORDER BY SUM(CAST(total_cost_usd AS REAL)) DESC"
        ))
        .context("prepare model_stats 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })
        .context("query model_stats 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (model, req, tokens, cost) = row.context("读取 model_stats 行失败")?;
        out.push(ModelStats {
            model,
            request_count: req as u64,
            total_tokens: tokens as u64,
            total_cost: format!("{:.6}", cost),
        });
    }
    Ok(out)
}

/// 数据源状态（路径 + 是否存在）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceStatus {
    pub app_type: String,
    pub label: String,
    pub path: String,
    pub exists: bool,
}

pub fn get_usage_data_sources() -> Vec<DataSourceStatus> {
    let paths = crate::usage::paths::UsagePaths::detect();
    [
        ("opencode", "OpenCode", &paths.opencode_db),
        ("claude", "Claude Code", &paths.claude_projects),
        ("codex", "Codex", &paths.codex_sessions),
        ("gemini", "Gemini", &paths.gemini_dir),
        ("grokbuild", "Grok Build", &paths.grok_dir),
    ]
    .into_iter()
    .map(|(app, label, p)| DataSourceStatus {
        app_type: app.to_string(),
        label: label.to_string(),
        path: p.display().to_string(),
        exists: p.exists(),
    })
    .collect()
}
