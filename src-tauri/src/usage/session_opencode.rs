use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, with_transaction, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::str::FromStr;
use std::time::SystemTime;

/// 未完成消息的容忍窗口：超过该时长仍未完成的会话视为已终止，允许推进水位线
const INCOMPLETE_TOLERANCE_SECS: i64 = 24 * 3600;

/// 同步 OpenCode 使用数据（只读打开 ~/.local/share/opencode/opencode.db）
pub fn sync_opencode_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let db_path = &paths.opencode_db;
    let mut result = SessionSyncResult::default();

    if !db_path.exists() {
        return Ok(result);
    }

    let db_path_str = db_path.to_string_lossy().to_string();

    // opencode 是 WAL 模式，需同时考虑 -wal 的 mtime
    let metadata = fs::metadata(db_path).context("无法读取 opencode.db 元数据")?;
    let mut file_modified = metadata_modified_nanos(&metadata);
    let wal_path = db_path.with_extension("db-wal");
    if let Ok(wal_meta) = fs::metadata(&wal_path) {
        file_modified = file_modified.max(metadata_modified_nanos(&wal_meta));
    }

    let (last_modified, _) = get_sync_state(db, &db_path_str)?;
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

    // 查询所有会话及水位线
    let mut stmt = opencode_conn
        .prepare(
            "SELECT s.id,
                    MAX(s.time_updated, COALESCE(MAX(m.time_updated), s.time_updated)) AS sync_watermark
             FROM session s
             LEFT JOIN message m ON m.session_id = s.id
             GROUP BY s.id
             ORDER BY sync_watermark",
        )
        .context("准备会话查询失败")?;
    let sessions: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("查询会话失败")?
        .filter_map(|r| r.ok())
        .collect();

    for (session_id, time_updated) in &sessions {
        let sync_key = format!("{db_path_str}:{session_id}");
        let (sess_last_modified, _) = get_sync_state(db, &sync_key)?;
        if *time_updated <= sess_last_modified {
            continue;
        }

        let mut session_had_error = false;
        let mut session_has_incomplete = false;

        // 查询 assistant 消息
        let mut msg_stmt = opencode_conn
            .prepare("SELECT id, data FROM message WHERE session_id = ?1 ORDER BY time_created")
            .context("准备消息查询失败")?;
        let rows = msg_stmt
            .query_map([session_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .context("查询消息失败")?;

        with_transaction(db, |conn| {
            for row in rows {
                let (message_id, data_json) = match row {
                    Ok(r) => r,
                    Err(_) => {
                        session_had_error = true;
                        continue;
                    }
                };
                let value: serde_json::Value = match serde_json::from_str(&data_json) {
                    Ok(v) => v,
                    Err(_) => {
                        let m = format!("OpenCode 消息解析失败 {session_id}:{message_id}");
                        result.errors.push(m);
                        result.skipped += 1;
                        session_had_error = true;
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
                    if created_ms > 0 && now - created_ms / 1000 > INCOMPLETE_TOLERANCE_SECS {
                        result.skipped += 1;
                        continue;
                    }
                    session_has_incomplete = true;
                    continue;
                }

                let Some(msg) = parse_message_data(&value) else { continue };

                let request_id = format!("opencode_session:{session_id}:{message_id}");
                match insert_opencode_message(conn, &request_id, &msg) {
                    Ok(true) => result.imported += 1,
                    Ok(false) => result.skipped += 1,
                    Err(e) => {
                        let m = format!("OpenCode 消息插入失败 {request_id}: {e:#}");
                        eprintln!("[OPENCODE-SYNC] {m}");
                        result.errors.push(m);
                        result.skipped += 1;
                        session_had_error = true;
                    }
                }
            }
            Ok(())
        })?;

        if session_had_error {
            has_sync_errors = true;
            continue;
        }
        if session_has_incomplete {
            continue;
        }
        if let Err(e) = update_sync_state(db, &sync_key, *time_updated, 0) {
            let m = format!("OpenCode 会话同步状态更新失败 {session_id}: {e:#}");
            eprintln!("[OPENCODE-SYNC] {m}");
            result.errors.push(m);
            has_sync_errors = true;
        }
    }

    if !has_sync_errors {
        update_sync_state(db, &db_path_str, file_modified, 0)?;
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

    Some(OpenCodeMessageData {
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost,
        model_id,
        timestamp_ms,
    })
}

fn insert_opencode_message(
    conn: &rusqlite::Connection,
    request_id: &str,
    msg: &OpenCodeMessageData,
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

    // 优先用 opencode 自带的 cost，否则定价表估算
    let (costs, total_cost) = if msg.cost > 0.0 {
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
        )
    } else {
        let usage = TokenUsage {
            input_tokens: msg.input_tokens,
            output_tokens: output_with_reasoning,
            cache_read_tokens: msg.cache_read_tokens,
            cache_creation_tokens: msg.cache_write_tokens,
            model: Some(model_id.clone()),
        };
        match find_model_pricing(conn, model_id) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(
                    &usage,
                    &pricing,
                    input_includes_cache_read(app_type),
                    multiplier,
                );
                let total = c.total_cost.to_string();
                (c, total)
            }
            None => {
                let zero = crate::usage::pricing::CostBreakdown::default();
                (zero, "0".to_string())
            }
        }
    };

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
            200i64,
            Some(request_id),
            1i64,
            multiplier.to_string(),
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
