use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing_cached, TokenUsage, input_includes_cache_read, ModelPricing};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// 同步 Claude Code 会话日志（~/.claude/projects/**/*.jsonl）
pub fn sync_claude_session_logs(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let projects_dir = &paths.claude_projects;
    let mut result = SessionSyncResult::default();
    if !projects_dir.exists() {
        return Ok(result);
    }

    let jsonl_files = collect_jsonl_files(projects_dir);
    for file_path in &jsonl_files {
        result.files_scanned += 1;
        match sync_single_file(db, file_path) {
            Ok((imported, skipped)) => {
                result.imported += imported;
                result.skipped += skipped;
            }
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                eprintln!("[CLAUDE-SYNC] 文件解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        eprintln!("[CLAUDE-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

/// 收集 .jsonl：项目目录/*.jsonl + subagents/*.jsonl + subagents/workflows/wf_*/*.jsonl
fn collect_jsonl_files(projects_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(projects_dir) else { return files };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(sub_entries) = fs::read_dir(&path) {
            for sub_entry in sub_entries.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    files.push(sub_path);
                } else if sub_path.is_dir() {
                    let subagents_dir = sub_path.join("subagents");
                    if subagents_dir.is_dir() {
                        push_jsonl_children(&subagents_dir, &mut files);
                        let workflows_dir = subagents_dir.join("workflows");
                        if workflows_dir.is_dir() {
                            if let Ok(wf_entries) = fs::read_dir(&workflows_dir) {
                                for wf_entry in wf_entries.flatten() {
                                    if wf_entry.path().is_dir() {
                                        push_jsonl_children(&wf_entry.path(), &mut files);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

fn push_jsonl_children(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
}

struct ParsedAssistantUsage {
    message_id: String,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    stop_reason: Option<String>,
    timestamp: Option<String>,
    session_id: Option<String>,
    /// HTTP 状态码（0 表示识别到上游 error 字段）
    status_code: i64,
    /// 错误描述（上游 error.message 或 isApiErrorMessage 文本）
    error_message: Option<String>,
}

/// 解析一行 JSONL 为 assistant 消息使用数据；非 assistant/无 usage 返回 None
fn parse_assistant_line(value: &serde_json::Value, session_id: Option<String>) -> Option<ParsedAssistantUsage> {
    if value.get("type").and_then(|t| t.as_str()) != Some("assistant") {
        return None;
    }
    let message = value.get("message")?;
    let msg_id = message.get("id").and_then(|v| v.as_str())?;
    let usage = message.get("usage")?;
    let (status_code, error_message) = extract_claude_error(message);
    Some(ParsedAssistantUsage {
        message_id: msg_id.to_string(),
        model: message.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        cache_read_tokens: usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        cache_creation_tokens: usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        stop_reason: message.get("stop_reason").and_then(|v| v.as_str()).map(str::to_string),
        timestamp: value.get("timestamp").and_then(|v| v.as_str()).map(str::to_string),
        session_id,
        status_code,
        error_message,
    })
}

/// 从 claude assistant message 节点识别上游错误。
/// 识别规则：
/// 1. `message.error` 非空对象/字符串 → status=0, error="<内容>"
/// 2. `message.isApiErrorMessage == true` → status=0, error="api error message"
fn extract_claude_error(message: &serde_json::Value) -> (i64, Option<String>) {
    if let Some(err) = message.get("error") {
        if !err.is_null() {
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
            return (0, Some(trimmed));
        }
    }
    if message
        .get("isApiErrorMessage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return (0, Some("api error message".to_string()));
    }
    (200, None)
}

/// 判断新条目是否应替换已有条目（stop_reason 优先，其次 output_tokens 更大）
fn should_replace(existing: &ParsedAssistantUsage, new: &ParsedAssistantUsage) -> bool {
    if new.stop_reason.is_some() && existing.stop_reason.is_none() {
        true
    } else if new.stop_reason.is_some() == existing.stop_reason.is_some() {
        new.output_tokens > existing.output_tokens
    } else {
        false
    }
}

fn sync_single_file(db: &UsageDb, file_path: &Path) -> Result<(u32, u32)> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, last_offset, last_byte_offset, _) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    // 按字节跳读：seek 到 last_byte_offset,再从那里按行读。
    // 避免大 JSONL 文件每次同步都从头 BufReader::lines() 走到 last_offset 才能跳过。
    let mut file = fs::File::open(file_path).context("无法打开文件")?;
    file.seek(SeekFrom::Start(last_byte_offset as u64))
        .context("seek 到 byte_offset 失败")?;
    let mut reader = BufReader::new(file);

    let mut messages: HashMap<String, ParsedAssistantUsage> = HashMap::new();
    let mut current_session_id: Option<String> = None;
    let mut bad_lines: u32 = 0;

    let mut buf = String::new();
    let mut new_byte_offset = last_byte_offset;
    loop {
        buf.clear();
        let n = reader.read_line(&mut buf).context("读取文件行失败")?;
        if n == 0 {
            break; // EOF
        }
        new_byte_offset += n as i64;
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                bad_lines += 1;
                continue;
            }
        };
        if current_session_id.is_none() {
            if let Some(sid) = value.get("sessionId").and_then(|v| v.as_str()) {
                current_session_id = Some(sid.to_string());
            }
        }
        if value.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let Some(parsed) = parse_assistant_line(&value, current_session_id.clone()) else {
            bad_lines += 1;
            continue;
        };
        let msg_id = parsed.message_id.clone();

        // is_none_or 1.82 才稳定，这里用 map_or 保持 1.77 MSRV
        if messages.get(&msg_id).map_or(true, |existing| should_replace(existing, &parsed)) {
            messages.insert(msg_id, parsed);
        }
    }

    // 单条 INSERT 失败不阻塞水位线推进 — 错误进 errors 数组,后续 sync 自然跳过(request_id 唯一约束)
    let mut imported = 0u32;
    let mut skipped = bad_lines;
    let mut pricing_cache: std::collections::HashMap<String, Option<ModelPricing>> = std::collections::HashMap::new();
    let conn = db.conn.lock();
    for msg in messages.values() {
        let has_billable = msg.input_tokens > 0 || msg.output_tokens > 0
            || msg.cache_read_tokens > 0 || msg.cache_creation_tokens > 0;
        if !has_billable {
            continue;
        }
        let request_id = format!("claude_session:{}", msg.message_id);

        match insert_claude_entry(&conn, &request_id, msg, &mut pricing_cache) {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                eprintln!("[CLAUDE-SYNC] 插入失败 ({}): {e:#}", msg.message_id);
                skipped += 1;
                // 不 return Err —— 让水位线仍推进,避免下次同步重头
            }
        }
    }
    drop(conn);

    update_sync_state(db, &file_path_str, file_modified, last_offset, new_byte_offset, 0)?;
    Ok((imported, skipped))
}

fn insert_claude_entry(
    conn: &rusqlite::Connection,
    request_id: &str,
    msg: &ParsedAssistantUsage,
    pricing_cache: &mut std::collections::HashMap<String, Option<ModelPricing>>,
) -> Result<bool> {
    let created_at = msg
        .timestamp
        .as_deref()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });

    let usage = TokenUsage {
        input_tokens: msg.input_tokens,
        output_tokens: msg.output_tokens,
        cache_read_tokens: msg.cache_read_tokens,
        cache_creation_tokens: msg.cache_creation_tokens,
        model: Some(msg.model.clone()),
    };
    let multiplier = crate::usage::pricing::get_app_multiplier(conn, "claude");
    let (costs, total) = match find_model_pricing_cached(conn, &msg.model, &mut *pricing_cache) {
        Some(pricing) => {
            let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("claude"), multiplier);
            let t = c.total_cost.to_string();
            (c, t)
        }
        None => (Default::default(), "0".to_string()),
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
            "_claude_session",
            "claude",
            msg.model,
            msg.model,
            msg.input_tokens,
            msg.output_tokens,
            msg.cache_read_tokens,
            msg.cache_creation_tokens,
            crate::usage::sql_helpers::INPUT_TOKEN_SEMANTICS_FRESH,
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            total,
            0i64,
            msg.status_code,
            msg.error_message.as_deref(),
            msg.session_id.as_deref(),
            1i64,
            multiplier.to_string(),
            "pricing_estimated",
            created_at,
            "session",
        ],
    )
    .context("插入 Claude 消息失败")?;
    Ok(inserted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_line() {
        let line = r#"{"type":"assistant","sessionId":"sess-1","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg-1","model":"claude-sonnet-4","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5},"stop_reason":"end_turn"}}"#;
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let usage = value.get("message").unwrap().get("usage").unwrap();
        assert_eq!(usage.get("input_tokens").unwrap().as_u64().unwrap(), 100);
        assert_eq!(usage.get("cache_read_input_tokens").unwrap().as_u64().unwrap(), 10);
        assert_eq!(value.get("sessionId").unwrap().as_str().unwrap(), "sess-1");
    }

    #[test]
    fn test_parse_assistant_line_extracts_fields() {
        let line = r#"{"type":"assistant","sessionId":"sess-1","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg-1","model":"claude-sonnet-4","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5},"stop_reason":"end_turn"}}"#;
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let parsed = parse_assistant_line(&value, Some("sess-1".to_string())).unwrap();
        assert_eq!(parsed.message_id, "msg-1");
        assert_eq!(parsed.model, "claude-sonnet-4");
        assert_eq!(parsed.input_tokens, 100);
        assert_eq!(parsed.output_tokens, 50);
        assert_eq!(parsed.cache_read_tokens, 10);
        assert_eq!(parsed.cache_creation_tokens, 5);
        assert_eq!(parsed.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(parsed.session_id.as_deref(), Some("sess-1"));
    }

    #[test]
    fn test_parse_assistant_line_skips_non_assistant() {
        let line = r#"{"type":"user","sessionId":"sess-1","message":{"id":"msg-2","content":"hi"}}"#;
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(parse_assistant_line(&value, None).is_none());
    }

    #[test]
    fn test_dedup_prefers_stop_reason() {
        let existing = ParsedAssistantUsage {
            message_id: "msg-1".to_string(),
            model: "claude-sonnet-4".to_string(),
            input_tokens: 100,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            stop_reason: None,
            timestamp: None,
            session_id: None,
            status_code: 200,
            error_message: None,
        };
        let with_stop_reason = ParsedAssistantUsage {
            output_tokens: 50,
            stop_reason: Some("end_turn".to_string()),
            ..ParsedAssistantUsage {
                message_id: "msg-1".to_string(),
                model: "claude-sonnet-4".to_string(),
                input_tokens: 100,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                stop_reason: None,
                timestamp: None,
                session_id: None,
                status_code: 200,
                error_message: None,
            }
        };
        assert!(should_replace(&existing, &with_stop_reason));
    }
}
