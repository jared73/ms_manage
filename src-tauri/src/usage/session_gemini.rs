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
        if msg.get("type").and_then(|t| t.as_str()) != Some("gemini") {
            continue;
        }
        let Some(tokens_obj) = msg.get("tokens").filter(|t| t.is_object()) else { continue };

        let input = tokens_obj.get("input").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let output = tokens_obj.get("output").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let cached = tokens_obj.get("cached").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let thoughts = tokens_obj.get("thoughts").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if input == 0 && output == 0 && cached == 0 && thoughts == 0 {
            continue;
        }

        let message_id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model = msg.get("model").and_then(|v| v.as_str()).unwrap_or("unknown");
        let timestamp = msg.get("timestamp").and_then(|v| v.as_str());

        let created_at = timestamp
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            });

        // thoughts 合并到 output 计费
        let output_tokens = output + thoughts;
        let request_id = format!("gemini_session:{}:{}", session_id.as_deref().unwrap_or("unknown"), message_id);

        let usage = TokenUsage {
            input_tokens: input,
            output_tokens,
            cache_read_tokens: cached,
            cache_creation_tokens: 0,
            model: Some(model.to_string()),
        };
        let conn = db.conn.lock();
        let (costs, total) = match find_model_pricing(&conn, model) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("gemini"));
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
            model,
            input,
            output_tokens,
            cached,
            0,
            &costs,
            &total,
            session_id.as_deref(),
            created_at,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_nonexistent() {
        assert!(collect_gemini_session_files(Path::new("/nonexistent")).is_empty());
    }
}
