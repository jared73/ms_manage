use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 同步 Gemini 会话日志（~/.gemini/tmp/*/chats/session-*.json）
pub fn sync_gemini_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let files = collect_gemini_session_files(&paths.gemini_dir);
    let mut result = SessionSyncResult {
        files_scanned: files.len() as u32,
        ..Default::default()
    };

    for file_path in &files {
        match sync_single_gemini_file(db, file_path) {
            Ok((imported, skipped)) => {
                result.imported += imported;
                result.skipped += skipped;
            }
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                eprintln!("[GEMINI-SYNC] 解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        eprintln!("[GEMINI-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

fn collect_gemini_session_files(gemini_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let tmp_dir = gemini_dir.join("tmp");
    let Ok(project_dirs) = fs::read_dir(&tmp_dir) else { return files };
    for entry in project_dirs.flatten() {
        let chats_dir = entry.path().join("chats");
        let Ok(chat_files) = fs::read_dir(&chats_dir) else { continue };
        for file_entry in chat_files.flatten() {
            let path = file_entry.path();
            let is_session = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("session-") && n.ends_with(".json"))
                .unwrap_or(false);
            if is_session {
                files.push(path);
            }
        }
    }
    files
}

fn sync_single_gemini_file(db: &UsageDb, file_path: &Path) -> Result<(u32, u32)> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, _) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let content = fs::read_to_string(file_path).context("无法读取文件")?;
    let value: serde_json::Value = serde_json::from_str(&content).context("JSON 解析失败")?;

    let session_id = value.get("sessionId").and_then(|v| v.as_str()).map(str::to_string);
    let Some(messages) = value.get("messages").and_then(|m| m.as_array()) else {
        return Ok((0, 0));
    };

    let mut imported = 0u32;
    let mut skipped = 0u32;

    for msg in messages {
        let Some(data) = parse_gemini_message(msg) else { continue };

        // 时间戳缺失/无法解析时回退为当前时间
        let created_at = if data.created_at == 0 {
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        } else {
            data.created_at
        };

        let request_id = format!(
            "gemini_session:{}:{}",
            session_id.as_deref().unwrap_or("unknown"),
            data.message_id
        );

        let usage = TokenUsage {
            input_tokens: data.input,
            output_tokens: data.output,
            cache_read_tokens: data.cached,
            cache_creation_tokens: 0,
            model: Some(data.model.clone()),
        };
        let conn = db.conn.lock();
        let multiplier = crate::usage::pricing::get_app_multiplier(&conn, "gemini");
        let (costs, total) = match find_model_pricing(&conn, &data.model) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("gemini"), multiplier);
                let t = c.total_cost.to_string();
                (c, t)
            }
            None => (Default::default(), "0".to_string()),
        };
        drop(conn);

        let inserted = crate::usage::session_common::insert_request_log(
            db,
            &request_id,
            "gemini",
            &data.model,
            data.input,
            data.output,
            data.cached,
            0,
            &costs,
            &total,
            &multiplier.to_string(),
            session_id.as_deref(),
            created_at,
            crate::usage::sql_helpers::INPUT_TOKEN_SEMANTICS_TOTAL,
        )?;
        if inserted {
            imported += 1;
        } else {
            skipped += 1;
        }
    }

    update_sync_state(db, &file_path_str, file_modified, messages.len() as i64)?;
    Ok((imported, skipped))
}

/// 单条 gemini 消息解析出的 token 使用数据
struct GeminiMessageData {
    message_id: String,
    model: String,
    input: u32,
    output: u32,
    cached: u32,
    created_at: i64,
}

/// 解析单条 gemini 消息的 token 使用数据；非 gemini 类型/无 tokens/全零返回 None。
/// output 已包含 thoughts 合并值；created_at 为 unix 秒，时间戳缺失或无法解析时为 0。
fn parse_gemini_message(msg: &serde_json::Value) -> Option<GeminiMessageData> {
    if msg.get("type").and_then(|t| t.as_str()) != Some("gemini") {
        return None;
    }
    let tokens_obj = msg.get("tokens").filter(|t| t.is_object())?;

    let input = tokens_obj.get("input").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let output = tokens_obj.get("output").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let cached = tokens_obj.get("cached").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let thoughts = tokens_obj.get("thoughts").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if input == 0 && output == 0 && cached == 0 && thoughts == 0 {
        return None;
    }

    let created_at = msg
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or(0);

    Some(GeminiMessageData {
        message_id: msg.get("id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        model: msg.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        input,
        output: output + thoughts,
        cached,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_collect_nonexistent() {
        assert!(collect_gemini_session_files(Path::new("/nonexistent")).is_empty());
    }

    #[test]
    fn test_parse_gemini_message_full() {
        let msg = json!({
            "id": "msg-1",
            "type": "gemini",
            "model": "gemini-2.5-pro",
            "timestamp": "2024-01-01T00:00:00Z",
            "tokens": {
                "input": 100,
                "output": 50,
                "cached": 20,
                "thoughts": 10
            }
        });
        let data = parse_gemini_message(&msg).expect("应解析成功");
        assert_eq!(data.message_id, "msg-1");
        assert_eq!(data.model, "gemini-2.5-pro");
        assert_eq!(data.input, 100);
        assert_eq!(data.output, 60);
        assert_eq!(data.cached, 20);
        assert_eq!(data.created_at, 1704067200);
    }

    #[test]
    fn test_parse_gemini_message_skips_non_gemini() {
        let msg = json!({
            "id": "msg-2",
            "type": "user",
            "tokens": { "input": 10, "output": 5 }
        });
        assert!(parse_gemini_message(&msg).is_none());
    }

    #[test]
    fn test_parse_gemini_message_skips_zero_tokens() {
        let msg = json!({
            "id": "msg-3",
            "type": "gemini",
            "model": "gemini-2.5-flash",
            "timestamp": "2024-01-01T00:00:00Z",
            "tokens": { "input": 0, "output": 0, "cached": 0, "thoughts": 0 }
        });
        assert!(parse_gemini_message(&msg).is_none());
    }

    #[test]
    fn test_parse_gemini_message_missing_timestamp() {
        let msg = json!({
            "id": "msg-4",
            "type": "gemini",
            "model": "gemini-2.5-flash",
            "tokens": { "input": 3, "output": 2 }
        });
        let data = parse_gemini_message(&msg).expect("应解析成功");
        assert_eq!(data.created_at, 0);
    }
}
