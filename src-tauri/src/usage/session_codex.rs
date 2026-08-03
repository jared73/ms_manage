use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// 同步 Codex 会话日志（~/.codex/sessions/**/rollout-*.jsonl）
pub fn sync_codex_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let codex_dir = &paths.codex_sessions;
    let mut result = SessionSyncResult::default();

    let mut files = Vec::new();
    let sessions_dir = codex_dir.join("sessions");
    if sessions_dir.is_dir() {
        collect_jsonl_recursive(&sessions_dir, &mut files);
    }
    let archived = codex_dir.join("archived_sessions");
    if archived.is_dir() {
        collect_jsonl_recursive(&archived, &mut files);
    }

    for file_path in &files {
        result.files_scanned += 1;
        match sync_single_file(db, file_path) {
            Ok((imported, skipped)) => {
                result.imported += imported;
                result.skipped += skipped;
            }
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                eprintln!("[CODEX-SYNC] 文件解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        eprintln!("[CODEX-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

fn collect_jsonl_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_recursive(&path, files);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
}

#[derive(Debug, Clone, Default)]
struct CumulativeTokens {
    input: u64,
    cached_input: u64,
    output: u64,
}

#[derive(Debug, Clone, Default)]
struct DeltaTokens {
    input: u32,
    cached_input: u32,
    output: u32,
}

impl DeltaTokens {
    fn is_zero(&self) -> bool {
        self.input == 0 && self.cached_input == 0 && self.output == 0
    }
}

fn sync_single_file(db: &UsageDb, file_path: &Path) -> Result<(u32, u32)> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, last_offset) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let file = fs::File::open(file_path).context("无法打开文件")?;
    let reader = BufReader::new(file);

    let mut line_offset = 0i64;
    let mut prev_total: Option<CumulativeTokens> = None;
    let mut event_index = 0u32;
    let mut imported = 0u32;
    let mut skipped = 0u32;

    // session_id = 文件名中的 UUID
    let session_id = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    for line_result in reader.lines() {
        line_offset += 1;
        if line_offset <= last_offset {
            continue;
        }
        let Ok(line) = line_result else { continue };
        if line.trim().is_empty() || !line.contains("token_count") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if value.get("type").and_then(|t| t.as_str()) != Some("event_msg") {
            continue;
        }
        let Some(payload) = value.get("payload") else { continue };
        if payload.get("type").and_then(|t| t.as_str()) != Some("token_count") {
            continue;
        }
        let Some(info) = payload.get("info").filter(|i| !i.is_null()) else { continue };
        let Some(model) = info
            .get("model")
            .or_else(|| info.get("model_name"))
            .or_else(|| payload.get("model"))
            .and_then(|v| v.as_str())
        else {
            continue;
        };

        // 取累计或单轮 token
        let (cumulative, is_total) = if let Some(total) = info.get("total_token_usage") {
            (parse_cumulative(total), true)
        } else if let Some(last) = info.get("last_token_usage") {
            (parse_cumulative(last), false)
        } else {
            continue;
        };
        let Some(cum) = cumulative else { continue };

        let delta = if is_total {
            let d = compute_delta(&prev_total, &cum);
            prev_total = Some(cum);
            d
        } else {
            DeltaTokens {
                input: cum.input as u32,
                cached_input: cum.cached_input as u32,
                output: cum.output as u32,
            }
        };
        let delta = DeltaTokens {
            cached_input: delta.cached_input.min(delta.input),
            ..delta
        };
        if delta.is_zero() {
            continue;
        }

        event_index += 1;
        let created_at = parse_timestamp_secs(value.get("timestamp"));
        let request_id = format!("codex_session:{session_id}:{event_index}");

        let usage = TokenUsage {
            input_tokens: delta.input,
            output_tokens: delta.output,
            cache_read_tokens: delta.cached_input,
            cache_creation_tokens: 0,
            model: Some(model.to_string()),
        };
        let conn = db.conn.lock();
        let (costs, total) = match find_model_pricing(&conn, model) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("codex"));
                let t = c.total_cost.to_string();
                (c, t)
            }
            None => (Default::default(), "0".to_string()),
        };
        drop(conn);

        // insert_request_log 失败时直接 ? 传播错误，保证同步进度不会推进到失败行
        let inserted = crate::usage::session_common::insert_request_log(
            db,
            &request_id,
            "codex",
            model,
            delta.input,
            delta.output,
            delta.cached_input,
            0,
            &costs,
            &total,
            Some(&session_id),
            created_at,
        )?;
        if inserted {
            imported += 1;
        } else {
            skipped += 1;
        }
    }

    update_sync_state(db, &file_path_str, file_modified, line_offset)?;
    Ok((imported, skipped))
}

fn parse_cumulative(info: &serde_json::Value) -> Option<CumulativeTokens> {
    Some(CumulativeTokens {
        input: info.get("input").and_then(|v| v.as_u64()).unwrap_or(0),
        cached_input: info.get("cached_input").and_then(|v| v.as_u64()).unwrap_or(0),
        output: info.get("output").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

fn compute_delta(prev: &Option<CumulativeTokens>, cur: &CumulativeTokens) -> DeltaTokens {
    match prev {
        None => DeltaTokens {
            input: cur.input as u32,
            cached_input: cur.cached_input as u32,
            output: cur.output as u32,
        },
        Some(p) => DeltaTokens {
            input: cur.input.saturating_sub(p.input) as u32,
            cached_input: cur.cached_input.saturating_sub(p.cached_input) as u32,
            output: cur.output.saturating_sub(p.output) as u32,
        },
    }
}

/// 解析 ISO8601 时间戳为 unix 秒，失败返回 0
fn parse_timestamp_secs(value: Option<&serde_json::Value>) -> i64 {
    let Some(v) = value else { return 0 };
    if let Some(n) = v.as_i64() {
        return n;
    }
    v.as_str()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_delta() {
        let prev = Some(CumulativeTokens { input: 100, cached_input: 50, output: 30 });
        let cur = CumulativeTokens { input: 150, cached_input: 60, output: 55 };
        let d = compute_delta(&prev, &cur);
        assert_eq!(d.input, 50);
        assert_eq!(d.cached_input, 10);
        assert_eq!(d.output, 25);
    }

    #[test]
    fn test_compute_delta_first() {
        let cur = CumulativeTokens { input: 100, cached_input: 0, output: 20 };
        let d = compute_delta(&None, &cur);
        assert_eq!(d.input, 100);
        assert_eq!(d.output, 20);
    }

    #[test]
    fn test_compute_delta_saturating() {
        // 文件滚动或重置导致累计值变小 → saturating 为 0，不产生负 delta
        let prev = Some(CumulativeTokens { input: 200, cached_input: 0, output: 100 });
        let cur = CumulativeTokens { input: 50, cached_input: 0, output: 30 };
        let d = compute_delta(&prev, &cur);
        assert!(d.is_zero());
    }
}
