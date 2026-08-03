use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 事件沉降窗：只导入早于「现在 − 2 分钟」的事件，避免与代理双算
const SETTLE_WINDOW_SECONDS: i64 = 120;

/// 同步 Grok Build 使用数据（~/.grok/{sessions,archived_sessions}/**/updates.jsonl）
pub fn sync_grokbuild_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let mut files = Vec::new();
    for root in [paths.grok_dir.join("sessions"), paths.grok_dir.join("archived_sessions")] {
        collect_files_named(&root, "updates.jsonl", &mut files);
    }

    let mut result = SessionSyncResult {
        files_scanned: files.len() as u32,
        ..Default::default()
    };

    for file_path in &files {
        match sync_single_grok_file(db, file_path) {
            Ok(file_result) => result.merge(file_result),
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                eprintln!("[GROK-SYNC] 解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        eprintln!("[GROK-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

fn collect_files_named(root: &Path, name: &str, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_named(&path, name, files);
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            files.push(path);
        }
    }
}

fn sync_single_grok_file(db: &UsageDb, file_path: &Path) -> Result<SessionSyncResult> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, _) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok(SessionSyncResult::default());
    }

    let content = fs::read_to_string(file_path).context("无法读取文件")?;
    let events = parse_grok_usage_events(&content);

    let session_id = file_path
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut result = SessionSyncResult::default();
    let mut deferred = false;

    for (idx, event) in events.iter().enumerate() {
        if now.saturating_sub(event.created_at) < SETTLE_WINDOW_SECONDS {
            deferred = true;
            break;
        }

        for (model, turn) in &event.per_model {
            if turn.input == 0 && turn.output == 0 && turn.cached == 0 {
                continue;
            }
            let turn_key = if event.prompt_id.is_empty() {
                format!("idx{idx}")
            } else {
                event.prompt_id.clone()
            };
            let request_id = format!("grok_session:{session_id}:{turn_key}:{model}");

            // 有 CLI 自报成本则优先使用，否则定价表估算
            let reported = if event.cost_ticks > 0 {
                Decimal::from(event.cost_ticks) / Decimal::from(10_000_000_000u64)
            } else {
                Decimal::ZERO
            };

            let usage = TokenUsage {
                input_tokens: turn.input,
                output_tokens: turn.output,
                cache_read_tokens: turn.cached,
                cache_creation_tokens: 0,
                model: Some(model.clone()),
            };
            let conn = db.conn.lock();
            let (costs, total) = match find_model_pricing(&conn, model) {
                Some(pricing) if reported == Decimal::ZERO || event.cost_partial => {
                    let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("grokbuild"));
                    let t = if reported == Decimal::ZERO {
                        c.total_cost.to_string()
                    } else {
                        reported.to_string()
                    };
                    (c, t)
                }
                Some(_) => (
                    crate::usage::pricing::CostBreakdown::default(),
                    reported.to_string(),
                ),
                None => (
                    crate::usage::pricing::CostBreakdown::default(),
                    if reported == Decimal::ZERO { "0".to_string() } else { reported.to_string() },
                ),
            };
            drop(conn);

            let inserted = crate::usage::session_common::insert_request_log(
                db,
                &request_id,
                "grokbuild",
                model,
                turn.input,
                turn.output,
                turn.cached,
                0,
                &costs,
                &total,
                Some(&session_id),
                event.created_at,
            )?;
            if inserted {
                result.imported += 1;
            } else {
                result.skipped += 1;
            }
        }
    }

    if deferred {
        result.deferred_files += 1;
    } else {
        update_sync_state(db, &file_path_str, file_modified, events.len() as i64)?;
    }

    Ok(result)
}

struct GrokCounters {
    input: u32,
    output: u32,
    cached: u32,
    cost_ticks: u64,
    cost_partial: bool,
}

struct GrokUsageEvent {
    created_at: i64,
    prompt_id: String,
    cost_ticks: u64,
    cost_partial: bool,
    per_model: Vec<(String, GrokCounters)>,
}

fn parse_grok_usage_events(content: &str) -> Vec<GrokUsageEvent> {
    let mut events = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if record.get("method").and_then(|v| v.as_str()) != Some("_x.ai/session/update") {
            continue;
        }
        let update = record.get("params").and_then(|p| p.get("update"));
        let kind = update.and_then(|u| u.get("sessionUpdate")).and_then(|v| v.as_str());
        if kind.is_some() && kind != Some("turn_completed") {
            continue;
        }
        let Some(usage) = update.and_then(|u| u.get("usage")).filter(|u| u.is_object()) else { continue };
        let Some(created_at) = parse_event_timestamp(record.get("timestamp")) else { continue };

        let prompt_id = update.and_then(|u| u.get("prompt_id")).and_then(|v| v.as_str()).unwrap_or("").to_string();

        let mut per_model: Vec<(String, GrokCounters)> = usage
            .get("modelUsage")
            .and_then(|m| m.as_object())
            .map(|map| {
                map.iter()
                    .map(|(model, c)| (model.clone(), parse_grok_counters(c)))
                    .collect()
            })
            .unwrap_or_default();
        if per_model.is_empty() {
            per_model.push(("unknown".to_string(), parse_grok_counters(usage)));
        }
        per_model.sort_by(|a, b| a.0.cmp(&b.0));

        // CLI 成本在各模型对象内（modelUsage.<model>.costUsdTicks），从 per_model 聚合；
        // 无 modelUsage 时 fallback 为 unknown 计数器，已含 usage 顶层字段
        let cost_ticks = per_model.iter().map(|(_, c)| c.cost_ticks).sum();
        let cost_partial = per_model.iter().any(|(_, c)| c.cost_partial);

        events.push(GrokUsageEvent {
            created_at,
            prompt_id,
            cost_ticks,
            cost_partial,
            per_model,
        });
    }
    events
}

fn parse_grok_counters(value: &serde_json::Value) -> GrokCounters {
    let get = |key: &str| value.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
    GrokCounters {
        input: get("inputTokens").min(u32::MAX as u64) as u32,
        output: get("outputTokens").min(u32::MAX as u64) as u32,
        cached: get("cachedReadTokens").min(u32::MAX as u64) as u32,
        cost_ticks: get("costUsdTicks"),
        cost_partial: value.get("costIsPartial").and_then(|v| v.as_bool()).unwrap_or(false),
    }
}

fn parse_event_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    let s = value.as_str()?;
    // 纯数字字符串（如 "1700000000"）按秒/毫秒处理
    if let Ok(n) = s.parse::<i64>() {
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_EPOCH: i64 = 1_700_000_000;

    fn usage_line(epoch: i64, prompt_id: &str) -> String {
        format!(
            r#"{{"timestamp":{epoch},"method":"_x.ai/session/update","params":{{"update":{{"sessionUpdate":"turn_completed","prompt_id":"{prompt_id}","usage":{{"modelUsage":{{"grok-4.5-build":{{"inputTokens":16632,"outputTokens":104,"cachedReadTokens":0,"reasoningTokens":0,"costUsdTicks":338880000}}}}}}}}}}}}"#
        )
    }

    #[test]
    fn parses_turn_completed_ignores_noise() {
        let content = concat!(
            "not json\n",
            "{\"timestamp\":\"2026-07-20T13:26:20Z\",\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"usage_snapshot\",\"prompt_id\":\"px\",\"usage\":{\"inputTokens\":9999,\"outputTokens\":9,\"cachedReadTokens\":0}}}}\n",
            "{\"timestamp\":\"2026-07-20T13:26:24Z\",\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"turn_completed\",\"prompt_id\":\"p1\",\"usage\":{\"modelUsage\":{\"grok-4.5-build\":{\"inputTokens\":16632,\"outputTokens\":104,\"cachedReadTokens\":0,\"costUsdTicks\":338880000}}}}}}\n"
        );
        let events = parse_grok_usage_events(content);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].prompt_id, "p1");
        assert_eq!(events[0].per_model[0].0, "grok-4.5-build");
        assert_eq!(events[0].per_model[0].1.input, 16632);
        assert_eq!(events[0].cost_ticks, 338_880_000);
    }

    #[test]
    fn missing_model_usage_falls_back() {
        let line = format!(
            r#"{{"timestamp":"{OLD_EPOCH}","method":"_x.ai/session/update","params":{{"update":{{"prompt_id":"p1","usage":{{"inputTokens":100,"outputTokens":10,"cachedReadTokens":5}}}}}}}}"#
        );
        let events = parse_grok_usage_events(&line);
        assert_eq!(events[0].per_model[0].0, "unknown");
        assert_eq!(events[0].per_model[0].1.input, 100);
    }
}
