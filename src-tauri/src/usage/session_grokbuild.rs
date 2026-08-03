use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{
    calculate_cost, find_model_pricing, input_includes_cache_read, CostBreakdown, ModelPricing,
    TokenUsage,
};
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

            // CLI 自报成本（完整则 ground truth；costIsPartial 时只是下界）与
            // 本地定价的取舍见 compute_costs 注释。用 per-model 的 cost_ticks
            // 而非事件级求和，避免多模型 turn 的整单成本被每行重复计一次
            let reported = if turn.cost_ticks > 0 {
                Decimal::from(turn.cost_ticks) / Decimal::from(10_000_000_000u64)
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
            let pricing = find_model_pricing(&conn, model);
            let multiplier = crate::usage::pricing::get_app_multiplier(&conn, "grokbuild");
            drop(conn);
            // 顶层 costIsPartial 覆盖整事件；per-model 标记再单独兜底该行
            let (costs, total) = compute_costs(
                pricing.as_ref(),
                &usage,
                reported,
                event.cost_partial || turn.cost_partial,
                multiplier,
            );

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
                &multiplier.to_string(),
                Some(&session_id),
                event.created_at,
                crate::usage::sql_helpers::INPUT_TOKEN_SEMANTICS_TOTAL,
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

/// total_cost 取值优先级（对齐 cc-switch 参考实现 insert_grok_session_entry）：
/// 1. 有自报且完整（costIsPartial=false）→ 以自报为准（上游 ground truth），
///    本地定价只负责分项与展示；
/// 2. 自报不完整（costIsPartial=true）→ 自报只是下界：有本地价用本地全额复算
///    （token 数完整）；无价则仍用自报下界（好过记 0）；
/// 3. 无自报 → 本地复算；彻底无价才整单记 0。
/// 分项（CostBreakdown）始终按本地定价复算，供展示；total 按上述优先级。
/// cost_multiplier 为 app 级成本倍率：本地复算分支的 total 已由 calculate_cost 内置
/// 倍率（costs.total_cost），自报分支单独乘倍率，保证全局倍率一致生效。
fn compute_costs(
    pricing: Option<&ModelPricing>,
    usage: &TokenUsage,
    reported: Decimal,
    cost_partial: bool,
    multiplier: Decimal,
) -> (CostBreakdown, String) {
    let costs = if let Some(p) = &pricing {
        calculate_cost(usage, p, input_includes_cache_read("grokbuild"), multiplier)
    } else {
        CostBreakdown::default()
    };
    let has_reported = reported > Decimal::ZERO;
    let total_cost = if has_reported && !cost_partial {
        reported * multiplier
    } else if has_reported && cost_partial && pricing.is_some() {
        costs.total_cost
    } else if has_reported {
        reported * multiplier
    } else if pricing.is_some() {
        costs.total_cost
    } else {
        Decimal::ZERO
    };
    (costs, total_cost.to_string())
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

        // 成本以 per-model 的 costUsdTicks 为准（GrokCounters.cost_ticks），
        // 不再做事件级求和，避免多模型 turn 的整单成本被每行重复计入
        // costIsPartial 主位置在 usage 顶层（对本事件全部模型生效），per-model 仅兜底
        let cost_partial = usage
            .get("costIsPartial")
            .and_then(|v| v.as_bool())
            .or_else(|| per_model.iter().any(|(_, c)| c.cost_partial).then_some(true))
            .unwrap_or(false);

        events.push(GrokUsageEvent {
            created_at,
            prompt_id,
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
        let content = vec![
            "not json\n".to_string(),
            "{\"timestamp\":\"2026-07-20T13:26:20Z\",\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"usage_snapshot\",\"prompt_id\":\"px\",\"usage\":{\"inputTokens\":9999,\"outputTokens\":9,\"cachedReadTokens\":0}}}}\n".to_string(),
            usage_line(1_692_389_184, "p1"),
            "\n".to_string(),
        ]
        .concat();
        let events = parse_grok_usage_events(&content);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].prompt_id, "p1");
        assert_eq!(events[0].per_model[0].0, "grok-4.5-build");
        assert_eq!(events[0].per_model[0].1.input, 16632);
        assert_eq!(events[0].per_model[0].1.cost_ticks, 338_880_000);
    }

    /// 带事件级顶层 costIsPartial 标记的变体（per-model 未标 partial）
    fn usage_line_partial(epoch: i64, prompt_id: &str) -> String {
        format!(
            r#"{{"timestamp":{epoch},"method":"_x.ai/session/update","params":{{"update":{{"sessionUpdate":"turn_completed","prompt_id":"{prompt_id}","usage":{{"costIsPartial":true,"modelUsage":{{"grok-4.5-build":{{"inputTokens":16632,"outputTokens":104,"cachedReadTokens":0,"reasoningTokens":0,"costUsdTicks":1000}}}}}}}}}}}}"#
        )
    }

    #[test]
    fn partial_cost_prefers_local_pricing() {
        use std::str::FromStr;

        // 事件级 costIsPartial=true 位于 usage 顶层（per-model 未标 partial）
        let events = parse_grok_usage_events(&usage_line_partial(OLD_EPOCH, "p1"));
        assert_eq!(events.len(), 1);
        assert!(events[0].cost_partial, "顶层 costIsPartial 应提升为事件级标志");
        assert_eq!(events[0].per_model[0].1.cost_ticks, 1_000);

        let (model, turn) = &events[0].per_model[0];
        assert_eq!(model, "grok-4.5-build");
        let reported = Decimal::from(turn.cost_ticks) / Decimal::from(10_000_000_000u64);
        let pricing = ModelPricing::from_strings("2.0", "6.0", "0.30", "2.0").unwrap();
        let usage = TokenUsage {
            input_tokens: turn.input,
            output_tokens: turn.output,
            cache_read_tokens: turn.cached,
            cache_creation_tokens: 0,
            model: Some(model.clone()),
        };

        // costIsPartial=true：自报只是下界，有本地价时 total 必须用本地全额复算
        let (costs, total) = compute_costs(
            Some(&pricing),
            &usage,
            reported,
            events[0].cost_partial,
            Decimal::from(1),
        );
        let expected = calculate_cost(
            &usage,
            &pricing,
            input_includes_cache_read("grokbuild"),
            Decimal::from(1),
        )
        .total_cost;
        assert!(expected > reported, "测试前提：本地复算应高于自报下界");
        assert_eq!(
            Decimal::from_str(&total).unwrap(),
            expected,
            "partial + 有本地价：total 应为本地全额复算，而非自报下界"
        );
        assert_ne!(Decimal::from_str(&total).unwrap(), reported);
        assert_eq!(costs.total_cost, expected);
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

    /// 多模型 turn：每行只能用自己的 costUsdTicks，不得用事件级求和（否则双算整单成本）
    #[test]
    fn multi_model_event_does_not_double_count_cost() {
        use rusqlite::Connection;
        use std::str::FromStr;

        let line = format!(
            r#"{{"timestamp":{OLD_EPOCH},"method":"_x.ai/session/update","params":{{"update":{{"sessionUpdate":"turn_completed","prompt_id":"p1","usage":{{"modelUsage":{{"model-a":{{"inputTokens":1000,"outputTokens":10,"cachedReadTokens":0,"costUsdTicks":12500000}},"model-b":{{"inputTokens":2000,"outputTokens":20,"cachedReadTokens":0,"costUsdTicks":25000000}}}}}}}}}}}}"#
        );
        let dir = std::env::temp_dir().join(format!(
            "grok-ms-test-{}-{}",
            std::process::id(),
            line.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("updates.jsonl");
        std::fs::write(&file, &line).unwrap();

        let conn = Connection::open_in_memory().unwrap();
        let db = UsageDb { conn: parking_lot::Mutex::new(conn) };
        crate::usage::db::init_schema(&db).unwrap();

        let result = sync_single_grok_file(&db, &file).unwrap();
        assert_eq!(result.imported, 2, "两个模型应各导入一行");

        let rows = {
            let conn = db.conn.lock();
            let mut stmt = conn
                .prepare("SELECT model, input_tokens, total_cost_usd FROM request_logs ORDER BY model")
                .unwrap();
            stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .unwrap()
                .filter_map(Result::ok)
                .collect::<Vec<_>>()
        };

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(rows.len(), 2);
        // 每行 total 只能是该模型自己的 ticks，事件级求和 37_500_000 若被每行重复
        // 计入会得到 0.00375，这里必须各归各
        assert_eq!(rows[0].0, "model-a");
        assert_eq!(rows[0].1, 1000);
        assert_eq!(
            Decimal::from_str(&rows[0].2).unwrap(),
            Decimal::from(12_500_000u64) / Decimal::from(10_000_000_000u64)
        );
        assert_eq!(rows[1].0, "model-b");
        assert_eq!(rows[1].1, 2000);
        assert_eq!(
            Decimal::from_str(&rows[1].2).unwrap(),
            Decimal::from(25_000_000u64) / Decimal::from(10_000_000_000u64)
        );
    }
}
