use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{
    find_model_pricing_cached, get_incomplete_tolerance_seconds, input_includes_cache_read, ModelPricing,
    TokenUsage,
};
use std::cell::RefCell;
use std::rc::Rc;
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, with_transaction, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::str::FromStr;
use std::time::SystemTime;

/// 未完成消息的容忍窗口默认值（24 小时）；可被 app_config.incomplete_tolerance_seconds 覆盖。
const INCOMPLETE_TOLERANCE_DEFAULT_SECS: i64 = 24 * 3600;

/// 同步 OpenCode 使用数据（只读打开 ~/.local/share/opencode/opencode.db）
/// `since_ts`：上次同步的最大 last_synced_at（毫秒），用作 session / message 全表扫的下界；
/// 0 表示首次同步,扫全表。
pub fn sync_opencode_usage(db: &UsageDb, paths: &UsagePaths, since_ts: i64) -> Result<SessionSyncResult> {
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let db_path = &paths.opencode_db;
    let mut result = SessionSyncResult::default();

    if !db_path.exists() {
        return Ok(result);
    }

    // 同步周期内的定价缓存：同一 model 在所有 session 里被命中,避免每次都 SELECT。
    // opencode 跨 session 共享 db conn,但 with_transaction 闭包拿不到外层 &mut HashMap,
    // 用 Rc<RefCell> 让闭包内可 borrow_mut 共享缓存。
    let pricing_cache: Rc<RefCell<std::collections::HashMap<String, Option<ModelPricing>>>> =
        Rc::new(RefCell::new(std::collections::HashMap::new()));

    let db_path_str = db_path.to_string_lossy().to_string();

    // opencode 是 WAL 模式，需同时考虑 -wal 的 mtime
    let metadata = fs::metadata(db_path).context("无法读取 opencode.db 元数据")?;
    let mut file_modified = metadata_modified_nanos(&metadata);
    let wal_path = db_path.with_extension("db-wal");
    if let Ok(wal_meta) = fs::metadata(&wal_path) {
        file_modified = file_modified.max(metadata_modified_nanos(&wal_meta));
    }

    let (last_modified, _, _, _) = get_sync_state(db, &db_path_str)?;
    if file_modified <= last_modified {
        result.files_scanned = 1;
        return Ok(result);
    }

    let opencode_conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .context("无法打开 opencode.db")?;

    result.files_scanned = 1;
    let mut has_sync_errors = false;

    // 查询自 since_ts 以来有更新的会话及水位线 —— 减少大 session 表的全表扫成本。
    // 首次同步 (since_ts = 0) 走全表;后续按上次同步时间过滤只取变更过的 session。
    // opencode 自报 cost 场景（#39）：已完成消息按 request_id 唯一约束 INSERT OR IGNORE，
    // 重复扫描不会重复写库 —— 唯一成本是多解析一次 JSON，量级小可接受。
    let sessions_sql = if since_ts > 0 {
        "SELECT s.id,
                MAX(s.time_updated, COALESCE(m_max.mx, s.time_updated)) AS sync_watermark
         FROM session s
         LEFT JOIN (
             SELECT session_id, MAX(time_updated) AS mx
             FROM message WHERE time_updated > ?1
             GROUP BY session_id
         ) m_max ON m_max.session_id = s.id
         WHERE s.time_updated > ?1 OR m_max.mx IS NOT NULL
         GROUP BY s.id
         ORDER BY sync_watermark"
    } else {
        "SELECT s.id,
                MAX(s.time_updated, COALESCE(m_max.mx, s.time_updated)) AS sync_watermark
         FROM session s
         LEFT JOIN (
             SELECT session_id, MAX(time_updated) AS mx
             FROM message GROUP BY session_id
         ) m_max ON m_max.session_id = s.id
         GROUP BY s.id
         ORDER BY sync_watermark"
    };
    let mut stmt = opencode_conn
        .prepare(sessions_sql)
        .context("准备会话查询失败")?;
    let sessions: Vec<(String, i64)> = stmt
        .query_map([since_ts], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("查询会话失败")?
        .filter_map(|r| r.ok())
        .collect();

    for (session_id, time_updated) in &sessions {
        let sync_key = format!("{db_path_str}:{session_id}");
        let (sess_last_modified, _, _, sess_last_msg_ts) = get_sync_state(db, &sync_key)?;
        if *time_updated <= sess_last_modified {
            continue;
        }

        let mut session_has_incomplete = false;

        // 查询 assistant 消息(增量：跳过 <= sess_last_msg_ts 的消息,避免 #39 未完成消息阻塞水位线)
        let msg_sql = if sess_last_msg_ts > 0 {
            "SELECT id, data, time_updated FROM message
             WHERE session_id = ?1 AND time_updated > ?2 ORDER BY time_created"
        } else {
            "SELECT id, data, time_updated FROM message
             WHERE session_id = ?1 ORDER BY time_created"
        };
        let mut msg_stmt = opencode_conn
            .prepare(msg_sql)
            .context("准备消息查询失败")?;
        let rows: Box<dyn Iterator<Item = rusqlite::Result<(String, String, i64)>>> = if sess_last_msg_ts > 0 {
            Box::new(
                msg_stmt
                    .query_map(
                        rusqlite::params![session_id, sess_last_msg_ts],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
                    )
                    .context("查询消息失败")?,
            )
        } else {
            Box::new(
                msg_stmt
                    .query_map([session_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)))
                    .context("查询消息失败")?,
            )
        };

        // 同步 session 内 max(message.time_updated) —— 即使有未完成消息,
        // 已处理的水位也推进(下次 sync 跳过该水位之前的 messages)
        let mut max_msg_ts = sess_last_msg_ts;

        with_transaction(db, |conn| {
            for row in rows {
                let (message_id, data_json, msg_ts) = match row {
                    Ok(r) => r,
                    Err(e) => {
                        // 单条行读失败 → 记错误,跳过该条,**不阻塞水位线**
                        let m = format!("OpenCode 消息行读失败 {session_id}: {e:#}");
                        eprintln!("[OPENCODE-SYNC] {m}");
                        result.errors.push(m);
                        result.skipped += 1;
                        continue;
                    }
                };
                if msg_ts > max_msg_ts {
                    max_msg_ts = msg_ts;
                }
                let value: serde_json::Value = match serde_json::from_str(&data_json) {
                    Ok(v) => v,
                    Err(_) => {
                        let m = format!("OpenCode 消息解析失败 {session_id}:{message_id}");
                        result.errors.push(m);
                        result.skipped += 1;
                        continue;
                    }
                };

                if value.get("role").and_then(|r| r.as_str()) != Some("assistant") {
                    continue;
                }
                if value.get("tokens").is_none() {
                    continue;
                }
                // 跳过未完成消息；超过容忍窗口的视为已终止会话，不阻塞水位线
                if value.get("time").and_then(|t| t.get("completed")).is_none() {
                    let created_ms = value
                        .get("time")
                        .and_then(|t| t.get("created"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    // 容忍窗口可被 app_config 覆盖(否则用 24h 默认)
                    let tolerance = get_incomplete_tolerance_seconds(
                        conn,
                        "opencode",
                        INCOMPLETE_TOLERANCE_DEFAULT_SECS,
                    );
                    if created_ms > 0 && now - created_ms / 1000 > tolerance {
                        result.skipped += 1;
                        continue;
                    }
                    session_has_incomplete = true;
                    continue;
                }

                let Some(msg) = parse_message_data(&value) else { continue };

                let request_id = format!("opencode_session:{session_id}:{message_id}");
                match insert_opencode_message(conn, &request_id, &msg, &mut pricing_cache.borrow_mut()) {
                    Ok(true) => result.imported += 1,
                    Ok(false) => result.skipped += 1,
                    Err(e) => {
                        // 单条 INSERT 失败 → 记错误,跳过该条,**不阻塞水位线**
                        let m = format!("OpenCode 消息插入失败 {request_id}: {e:#}");
                        eprintln!("[OPENCODE-SYNC] {m}");
                        result.errors.push(m);
                        result.skipped += 1;
                    }
                }
            }
            Ok(())
        })?;

        // 仅「仍有未完成消息」时不推进水位线(JSON 损坏 / INSERT 失败不阻塞)
        if session_has_incomplete {
            continue;
        }
        if let Err(e) = update_sync_state(db, &sync_key, *time_updated, 0, 0, max_msg_ts) {
            let m = format!("OpenCode 会话同步状态更新失败 {session_id}: {e:#}");
            eprintln!("[OPENCODE-SYNC] {m}");
            result.errors.push(m);
            has_sync_errors = true;
        }
    }

    if !has_sync_errors {
        update_sync_state(db, &db_path_str, file_modified, 0, 0, 0)?;
    }

    if result.imported > 0 {
        eprintln!("[OPENCODE-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

struct OpenCodeMessageData {
    input_tokens: u32,
    output_tokens: u32,
    reasoning_tokens: u32,
    cache_read_tokens: u32,
    cache_write_tokens: u32,
    cost: f64,
    model_id: String,
    timestamp_ms: i64,
    /// HTTP 状态码（0 表示识别到上游 error 字段）
    status_code: i64,
    /// 错误描述（上游 error 字段或本地识别失败原因）
    error_message: Option<String>,
}

fn parse_message_data(value: &serde_json::Value) -> Option<OpenCodeMessageData> {
    let tokens = value.get("tokens")?;
    let input_tokens = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let output_tokens = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let reasoning_tokens = tokens.get("reasoning").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let cache_obj = tokens.get("cache");
    let cache_read_tokens = cache_obj.and_then(|c| c.get("read")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let cache_write_tokens = cache_obj.and_then(|c| c.get("write")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    if input_tokens == 0 && output_tokens == 0 && reasoning_tokens == 0
        && cache_read_tokens == 0 && cache_write_tokens == 0
    {
        return None;
    }

    let cost = value.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let model_id = value.get("modelID").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let timestamp_ms = value.get("time").and_then(|t| t.get("created")).and_then(|v| v.as_i64()).unwrap_or(0);

    // 上游错误识别：opencode 在 message data 顶层放 error 字段（含 message 子字段）
    let (status_code, error_message) = extract_opencode_error(value);

    Some(OpenCodeMessageData {
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost,
        model_id,
        timestamp_ms,
        status_code,
        error_message,
    })
}

/// 从 opencode message data 中提取上游错误。
/// 识别规则：顶层 `error` 是 string 或 object（含 `message` 子字段）；命中则 status=0。
/// 非命中返回 (200, None) 表示本次请求按成功记录。
fn extract_opencode_error(value: &serde_json::Value) -> (i64, Option<String>) {
    let Some(err) = value.get("error") else { return (200, None) };
    if err.is_null() {
        return (200, None);
    }
    let msg = if let Some(s) = err.as_str() {
        s.to_string()
    } else if let Some(obj) = err.as_object() {
        obj.get("message")
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| err.to_string())
    } else {
        err.to_string()
    };
    let trimmed = msg.chars().take(200).collect::<String>();
    (0, Some(trimmed))
}

fn insert_opencode_message(
    conn: &rusqlite::Connection,
    request_id: &str,
    msg: &OpenCodeMessageData,
    pricing_cache: &mut std::collections::HashMap<String, Option<ModelPricing>>,
) -> Result<bool> {
    let created_at = if msg.timestamp_ms > 0 {
        msg.timestamp_ms / 1000
    } else {
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    };

    // opencode: output 含 reasoning；input 是 fresh input
    let output_with_reasoning = msg.output_tokens + msg.reasoning_tokens;
    let app_type = "opencode";
    let model_id = &msg.model_id;
    let multiplier = crate::usage::pricing::get_app_multiplier(conn, app_type);

    // 优先用 opencode 自带的 cost,否则定价表估算
    // cost_source 区分:上游自报=session_reported(recalculate 不动),本地估算=pricing_estimated
    let (costs, total_cost, cost_source) = if msg.cost > 0.0 {
        let zero = rust_decimal::Decimal::ZERO;
        let total = rust_decimal::Decimal::from_str(&msg.cost.to_string())
            .unwrap_or(rust_decimal::Decimal::ZERO);
        (
            crate::usage::pricing::CostBreakdown {
                input_cost: zero,
                output_cost: zero,
                cache_read_cost: zero,
                cache_creation_cost: zero,
                total_cost: total,
            },
            msg.cost.to_string(),
            "session_reported",
        )
    } else {
        let usage = TokenUsage {
            input_tokens: msg.input_tokens,
            output_tokens: output_with_reasoning,
            cache_read_tokens: msg.cache_read_tokens,
            cache_creation_tokens: msg.cache_write_tokens,
            model: Some(model_id.clone()),
        };
        match find_model_pricing_cached(conn, model_id, pricing_cache) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(
                    &usage,
                    &pricing,
                    input_includes_cache_read(app_type),
                    multiplier,
                );
                let total = c.total_cost.to_string();
                (c, total, "pricing_estimated")
            }
            None => {
                let zero = crate::usage::pricing::CostBreakdown::default();
                (zero, "0".to_string(), "pricing_estimated")
            }
        }
    };

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
            "_opencode_session",
            app_type,
            model_id,
            model_id,
            msg.input_tokens,
            output_with_reasoning,
            msg.cache_read_tokens,
            msg.cache_write_tokens,
            crate::usage::sql_helpers::INPUT_TOKEN_SEMANTICS_FRESH,
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            total_cost,
            0i64,
            msg.status_code,
            msg.error_message.as_deref(),
            Some(request_id),
            1i64,
            multiplier.to_string(),
            cost_source,
            created_at,
            "session",
        ],
    )
    .context("插入 OpenCode 消息失败")?;
    Ok(inserted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_data_full() {
        let json = serde_json::json!({
            "role": "assistant",
            "cost": 0.0023113,
            "tokens": {
                "total": 56554,
                "input": 3272,
                "output": 383,
                "reasoning": 419,
                "cache": { "write": 0, "read": 52480 }
            },
            "modelID": "deepseek-v4-pro",
            "providerID": "deepseek",
            "time": { "created": 1779755333700i64, "completed": 1779755350639i64 }
        });
        let data = parse_message_data(&json).unwrap();
        assert_eq!(data.input_tokens, 3272);
        assert_eq!(data.output_tokens, 383);
        assert_eq!(data.reasoning_tokens, 419);
        assert_eq!(data.cache_read_tokens, 52480);
        assert!((data.cost - 0.0023113).abs() < 1e-10);
        assert_eq!(data.model_id, "deepseek-v4-pro");
    }

    #[test]
    fn test_parse_message_data_missing_cache() {
        let json = serde_json::json!({
            "role": "assistant",
            "cost": 0.0,
            "tokens": { "input": 1000, "output": 200 },
            "modelID": "mimo-v2.5-pro",
            "time": { "created": 1779755333700i64 }
        });
        let data = parse_message_data(&json).unwrap();
        assert_eq!(data.input_tokens, 1000);
        assert_eq!(data.output_tokens, 200);
        assert_eq!(data.reasoning_tokens, 0);
        assert_eq!(data.cache_read_tokens, 0);
    }

    #[test]
    fn test_parse_message_data_skips_zero_tokens() {
        let json = serde_json::json!({
            "role": "assistant",
            "tokens": { "input": 0, "output": 0, "reasoning": 0, "cache": { "read": 0, "write": 0 } },
            "modelID": "test"
        });
        assert!(parse_message_data(&json).is_none());
    }
}
