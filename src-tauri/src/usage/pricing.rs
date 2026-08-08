use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

/// 模型定价（每百万 token 的人民币价格，CNY）
/// 国内模型使用官方人民币价；国外模型（Claude/GPT/Gemini/Grok）按官方美元价 × 7.2 汇率换算。
/// 未公布 API 按量价的模型（如 GLM-5.2、GLM-5）按同系列近似档位给出参考值，可在 UI 中修改。
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_million: Decimal,
    pub output_cost_per_million: Decimal,
    pub cache_read_cost_per_million: Decimal,
    pub cache_creation_cost_per_million: Decimal,
}

impl ModelPricing {
    pub fn from_strings(
        input: &str,
        output: &str,
        cache_read: &str,
        cache_creation: &str,
    ) -> Result<Self> {
        Ok(Self {
            input_cost_per_million: Decimal::from_str(input).context("input 定价非法")?,
            output_cost_per_million: Decimal::from_str(output).context("output 定价非法")?,
            cache_read_cost_per_million: Decimal::from_str(cache_read).context("cache_read 定价非法")?,
            cache_creation_cost_per_million: Decimal::from_str(cache_creation).context("cache_creation 定价非法")?,
        })
    }
}

/// 一次请求的 token 用量
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CostBreakdown {
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub cache_read_cost: Decimal,
    pub cache_creation_cost: Decimal,
    pub total_cost: Decimal,
}

/// 费用计算器。input_includes_cache_read=true 表示 input_tokens 已含 cache 部分
/// （OpenAI/Codex/Gemini 风格），需扣除后再按输入价计费。
/// cost_multiplier 为 app 级成本倍率（默认 1.0），total = 四部分之和 × 倍率。
pub fn calculate_cost(
    usage: &TokenUsage,
    pricing: &ModelPricing,
    input_includes_cache_read: bool,
    cost_multiplier: Decimal,
) -> CostBreakdown {
    let million = Decimal::from(1_000_000u64);
    let billable_input = if input_includes_cache_read {
        usage
            .input_tokens
            .saturating_sub(usage.cache_read_tokens)
            .saturating_sub(usage.cache_creation_tokens)
    } else {
        usage.input_tokens
    };
    let mut c = CostBreakdown {
        input_cost: Decimal::from(billable_input) * pricing.input_cost_per_million / million,
        output_cost: Decimal::from(usage.output_tokens) * pricing.output_cost_per_million / million,
        cache_read_cost: Decimal::from(usage.cache_read_tokens)
            * pricing.cache_read_cost_per_million
            / million,
        cache_creation_cost: Decimal::from(usage.cache_creation_tokens)
            * pricing.cache_creation_cost_per_million
            / million,
        ..Default::default()
    };
    c.total_cost = (c.input_cost + c.output_cost + c.cache_read_cost + c.cache_creation_cost)
        * cost_multiplier;
    c
}

/// 读取某 app 的成本倍率（默认 1.0）
pub fn get_app_multiplier(conn: &rusqlite::Connection, app_type: &str) -> Decimal {
    conn.query_row(
        "SELECT cost_multiplier FROM app_config WHERE app_type = ?1",
        [app_type],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| Decimal::from_str(&s).ok())
    .unwrap_or_else(|| Decimal::from(1))
}

/// input_tokens 已含 cache read/write 的 app（单一语义源，写入侧/聚合侧共用）
pub const CACHE_INCLUSIVE_APP_TYPES: &[&str] = &["codex", "gemini", "grokbuild"];

/// app_type 的 input_tokens 是否包含 cache read（true: codex/gemini/grokbuild; false: opencode/claude）
pub fn input_includes_cache_read(app_type: &str) -> bool {
    CACHE_INCLUSIVE_APP_TYPES.contains(&app_type)
}

/// 读取 per-app settle_window（grokbuild 用），app_config 中 0 / 行不存在 = `default`
pub fn get_settle_window_seconds(
    conn: &rusqlite::Connection,
    app_type: &str,
    default: i64,
) -> i64 {
    let stored: i64 = conn
        .query_row(
            "SELECT settle_window_seconds FROM app_config WHERE app_type = ?1",
            [app_type],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0);
    if stored > 0 {
        stored
    } else {
        default
    }
}

/// 读取 per-app incomplete_tolerance（opencode 用），0 / 行不存在 = `default`
pub fn get_incomplete_tolerance_seconds(
    conn: &rusqlite::Connection,
    app_type: &str,
    default: i64,
) -> i64 {
    let stored: i64 = conn
        .query_row(
            "SELECT incomplete_tolerance_seconds FROM app_config WHERE app_type = ?1",
            [app_type],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0);
    if stored > 0 {
        stored
    } else {
        default
    }
}

/// 从 usage.db 查询模型定价；未找到则从内置 seed 表加载并尝试落库。
pub fn find_model_pricing(
    conn: &rusqlite::Connection,
    model_id: &str,
) -> Option<ModelPricing> {
    let row = conn.query_row(
        "SELECT input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million
         FROM model_pricing WHERE model_id = ?1 COLLATE NOCASE",
        [model_id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
    );
    match row {
        Ok((i, o, cr, cc)) => ModelPricing::from_strings(&i, &o, &cr, &cc).ok(),
        Err(_) => {
            // 精确匹配未命中（首次同步/新模型）：尝试 seed 内置定价（前缀匹配不区分大小写）。
            // seed 命中属于正常路径（自动落库后下次即可精确命中），不打错误日志；
            // 仅当 seed 也没有时才报告，避免未知模型刷屏。
            let Some(seed) = seed_pricing(model_id) else {
                eprintln!("[USAGE-PRICING] 模型无内置定价 {model_id}");
                return None;
            };
            if let Err(e) = conn.execute(
                "INSERT OR IGNORE INTO model_pricing (model_id, display_name,
                    input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million)
                 VALUES (?1, ?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![model_id, seed.0, seed.1, seed.2, seed.3],
            ) {
                eprintln!("[USAGE-PRICING] 写入 seed 定价失败 {model_id}: {e}");
            }
            Some(seed.4)
        }
    }
}

/// 带缓存的 find_model_pricing —— 同步 session 内同一 model 可能被查数千次,
/// 用 `&mut HashMap` 避免每次都走 SQL。第一次调用仍走 DB（命中后会落 seed）,后续命中缓存。
/// 缓存 `Option<ModelPricing>`:None 也缓存,避免反复触发"查 DB → 查 seed 都失败"的路径。
pub fn find_model_pricing_cached(
    conn: &rusqlite::Connection,
    model_id: &str,
    cache: &mut HashMap<String, Option<ModelPricing>>,
) -> Option<ModelPricing> {
    if let Some(cached) = cache.get(model_id) {
        return cached.clone();
    }
    let result = find_model_pricing(conn, model_id);
    cache.insert(model_id.to_string(), result.clone());
    result
}

/// 内置默认定价表：每一项 (model_prefix, input, output, cache_read, cache_creation)。
/// 单位：元/百万 token（CNY）。顺序即匹配优先级，窄前缀必须排在宽前缀之前（如 gpt-4o-mini 在 gpt-4o 之前）。
pub fn default_pricing_map(
) -> Vec<(&'static str, &'static str, &'static str, &'static str, &'static str)> {
    vec![
        // ── Claude（官方美元价 × 7.2）──
        ("claude-3-5-haiku", "5.76", "28.8", "0.576", "7.2"),
        ("claude-3-5-sonnet", "21.6", "108", "2.16", "27"),
        ("claude-3-7-sonnet", "21.6", "108", "2.16", "27"),
        ("claude-3-opus", "108", "540", "10.8", "135"),
        ("claude-sonnet-4", "21.6", "108", "2.16", "27"),
        ("claude-opus-4", "36", "180", "3.6", "45"),
        // ── OpenAI（官方美元价 × 7.2）──
        ("gpt-4o-mini", "1.08", "4.32", "0.54", "1.08"),
        ("gpt-4o", "18", "72", "9", "18"),
        ("gpt-4.1-mini", "2.88", "11.52", "0.72", "2.88"),
        ("gpt-4.1-nano", "0.72", "2.88", "0.18", "0.72"),
        ("gpt-4.1", "14.4", "57.6", "3.6", "14.4"),
        ("o3-mini", "7.92", "31.68", "3.96", "7.92"),
        ("o3", "14.4", "57.6", "3.6", "14.4"),
        ("o4-mini", "7.92", "31.68", "3.96", "7.92"),
        ("o4", "14.4", "57.6", "3.6", "14.4"),
        // ── Gemini（官方美元价 × 7.2）──
        ("gemini-2.5-pro", "9", "72", "2.25", "9"),
        ("gemini-2.5-flash", "2.16", "18", "0.54", "2.16"),
        // ── DeepSeek（官方人民币价）──
        ("deepseek-v4-flash", "1", "2", "0.02", "1"),
        ("deepseek-v4-pro", "3", "6", "0.025", "3"),
        ("deepseek-r1", "3.96", "15.77", "1.01", "3.96"),
        ("deepseek-reasoner", "3.96", "15.77", "1.01", "3.96"),
        ("deepseek", "1.94", "7.92", "0.50", "1.94"),
        // ── Kimi（官方人民币价）──
        ("k3", "20", "100", "2", "20"),
        ("kimi-k2.7", "6.5", "27", "1.3", "6.5"),
        ("kimi-for-coding", "6.5", "27", "1.3", "6.5"),
        ("k2.7", "6.5", "27", "1.3", "6.5"),
        ("k2.6", "6.5", "27", "1.3", "6.5"),
        // ── MiniMax（官方人民币价）──
        ("MiniMax-M3", "4.2", "16.8", "0.84", "4.2"),
        ("MiniMax-M2.7-highspeed", "4.2", "16.8", "0.42", "2.625"),
        ("MiniMax-M2.7", "2.1", "8.4", "0.42", "2.625"),
        // ── 小米 MiMo（官方人民币价）──
        ("mimo-v2.5-pro", "3", "6", "0.025", "3"),
        ("mimo-v2.5", "1", "2", "0.02", "1"),
        // ── Qwen（官方人民币价；3.7-Max 12/36，3.5-Plus 输入 0.8）──
        ("qwen3.7-max", "12", "36", "2.4", "12"),
        ("qwen3.5-plus", "0.8", "2.4", "0.16", "0.8"),
        ("qwen", "3.6", "14.4", "0.72", "3.6"),
        // ── Grok（官方美元价 × 7.2）──
        ("grok-4.5-build", "14.4", "43.2", "2.16", "14.4"),
        ("grok", "14.4", "43.2", "2.16", "14.4"),
        // ── 智谱（GLM-5.1 官方价 6/24；未公布按量价的按同系列参考值）──
        ("glm-5.2", "8", "28", "0.8", "8"),
        ("glm-5.1", "6", "24", "0.6", "6"),
        ("glm-5", "8", "28", "0.8", "8"),
        ("glm-4.7", "5", "20", "0.5", "5"),
        ("glm-4.6", "5", "20", "0.5", "5"),
        ("glm-4.5-air", "0.6", "0.6", "0.1", "0.6"),
        // ── 腾讯混元（Hy3 官方价）──
        ("hunyuan", "1", "4", "0.25", "1"),
        // ── 字节豆包（Doubao-1.5-pro-32k 官方价）──
        ("doubao", "0.8", "2", "0.16", "0.8"),
    ]
}

/// 内置定价 seed：model_id -> (input, output, cache_read, cache_creation, ModelPricing)
/// 前缀匹配不区分大小写（如 GLM-5.2 / glm-5.2 均可命中）
fn seed_pricing(
    model_id: &str,
) -> Option<(&'static str, &'static str, &'static str, &'static str, ModelPricing)> {
    let lower = model_id.to_lowercase();
    let (_, input, output, cache_read, cache_creation) = default_pricing_map()
        .into_iter()
        .find(|(prefix, _, _, _, _)| lower.starts_with(&prefix.to_lowercase()))?;
    let p = ModelPricing::from_strings(input, output, cache_read, cache_creation).ok()?;
    Some((input, output, cache_read, cache_creation, p))
}

/// 批量写入所有内置默认定价（启动时调用，幂等）
/// 阶段 1：插入新 model_id（用户库里没有的）
/// 阶段 2：更新已有 seed 记录（display_name == model_id 视为「未自定义」）
///         到最新价格 —— 用户改过 display_name 的记录保留不动。
pub fn seed_default_pricing(conn: &rusqlite::Connection) -> Result<()> {
    let map = default_pricing_map();
    let mut updated = 0u32;
    let mut inserted = 0u32;
    for (prefix, input, output, cache_read, cache_creation) in &map {
        // 阶段 1：插入新（INSERT OR IGNORE：用户库里已有则跳过）
        let n = conn.execute(
            "INSERT OR IGNORE INTO model_pricing (model_id, display_name,
                input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million)
             VALUES (?1, ?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![prefix, input, output, cache_read, cache_creation],
        )?;
        inserted += n as u32;
        // 阶段 2：更新「未自定义」的已有 seed 记录到最新价格
        // 判断依据：display_name 仍等于 model_id（seed 写入时就是 model_id；
        // 用户改过 display_name 后这条更新不会命中）。
        let m = conn.execute(
            "UPDATE model_pricing SET
                input_cost_per_million = ?2,
                output_cost_per_million = ?3,
                cache_read_cost_per_million = ?4,
                cache_creation_cost_per_million = ?5
             WHERE model_id = ?1 AND display_name = ?1",
            rusqlite::params![prefix, input, output, cache_read, cache_creation],
        )?;
        updated += m as u32;
    }
    if inserted > 0 || updated > 0 {
        eprintln!(
            "[PRICING] seed 初始化完成: 插入 {inserted} 条, 更新 {updated} 条"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_anthropic_semantics() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            model: None,
        };
        let pricing = ModelPricing::from_strings("3.0", "15.0", "0.3", "3.75").unwrap();
        let c = calculate_cost(&usage, &pricing, false, Decimal::from(1));
        assert_eq!(c.input_cost, Decimal::from_str("0.003").unwrap());
        assert_eq!(c.output_cost, Decimal::from_str("0.0075").unwrap());
        assert_eq!(c.cache_read_cost, Decimal::from_str("0.00006").unwrap());
        assert_eq!(c.cache_creation_cost, Decimal::from_str("0.000375").unwrap());
        assert_eq!(c.total_cost, Decimal::from_str("0.010935").unwrap());
    }

    #[test]
    fn test_calculate_cache_inclusive() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            model: None,
        };
        let pricing = ModelPricing::from_strings("3.0", "15.0", "0.3", "3.75").unwrap();
        let c = calculate_cost(&usage, &pricing, true, Decimal::from(1));
        assert_eq!(c.input_cost, Decimal::from_str("0.0021").unwrap());
    }

    #[test]
    fn test_calculate_with_multiplier() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            model: None,
        };
        let pricing = ModelPricing::from_strings("3.0", "15.0", "0.3", "3.75").unwrap();
        let c = calculate_cost(&usage, &pricing, false, Decimal::from_str("2.0").unwrap());
        // 无倍率 total = 0.010935；×2 = 0.02187
        assert_eq!(c.total_cost, Decimal::from_str("0.02187").unwrap());
    }

    #[test]
    fn test_seed_lookup() {
        assert!(seed_pricing("claude-sonnet-4-20250514").is_some());
        assert!(seed_pricing("grok-4.5-build").is_some());
        assert!(seed_pricing("totally-unknown-model").is_none());
    }

    #[test]
    fn test_seed_case_insensitive() {
        assert_eq!(seed_pricing("GLM-5.2").unwrap().0, "8");
        assert_eq!(seed_pricing("GLM-5.1-highspeed").unwrap().0, "6");
        assert_eq!(seed_pricing("MiniMax-M3").unwrap().0, "4.2");
        assert_eq!(seed_pricing("DEEPSEEK-V4-FLASH").unwrap().0, "1");
        assert_eq!(seed_pricing("Kimi-K2.7-Code").unwrap().0, "6.5");
        assert_eq!(seed_pricing("claude-opus-4-8").unwrap().0, "36");
        assert_eq!(seed_pricing("MiniMax-M2.7-highspeed").unwrap().0, "4.2");
        assert_eq!(seed_pricing("MiniMax-M2.7").unwrap().0, "2.1");
    }

    #[test]
    fn test_seed_narrow_prefixes() {
        assert_eq!(seed_pricing("gpt-4o-mini").unwrap().0, "1.08");
        assert_eq!(seed_pricing("gpt-4o").unwrap().0, "18");
        assert_eq!(seed_pricing("o4-mini").unwrap().0, "7.92");
        assert_eq!(seed_pricing("grok-4.5-build").unwrap().0, "14.4");
        assert_eq!(seed_pricing("deepseek-v4-flash").unwrap().0, "1");
        assert_eq!(seed_pricing("k3").unwrap().0, "20");
        assert_eq!(seed_pricing("MiniMax-M3").unwrap().0, "4.2");
    }

    #[test]
    fn test_default_pricing_map_nonempty() {
        let map = default_pricing_map();
        assert!(!map.is_empty());
        let prefixes: Vec<&str> = map.iter().map(|(p, _, _, _, _)| *p).collect();
        assert!(prefixes.contains(&"claude-sonnet-4"));
        assert!(prefixes.contains(&"gpt-4o"));
        assert!(prefixes.contains(&"deepseek-reasoner"));
        assert!(prefixes.contains(&"k3"));
        assert!(prefixes.contains(&"MiniMax-M3"));
    }

    #[test]
    fn test_seed_default_pricing_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE model_pricing (
                model_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                input_cost_per_million TEXT NOT NULL,
                output_cost_per_million TEXT NOT NULL,
                cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
            );",
        )
        .unwrap();
        seed_default_pricing(&conn).unwrap();
        seed_default_pricing(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM model_pricing", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, default_pricing_map().len() as i64);
    }

    /// #40 验证 seed 更新机制：display_name 仍 == model_id 的行会被更新到最新价格；
    /// display_name 已被用户改过的行不会被覆盖。
    #[test]
    fn test_seed_update_only_when_display_name_unchanged() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE model_pricing (
                model_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                input_cost_per_million TEXT NOT NULL,
                output_cost_per_million TEXT NOT NULL,
                cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
                cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
            );",
        )
        .unwrap();
        // 第一次 seed：插入全部默认值
        seed_default_pricing(&conn).unwrap();
        // 用户修改某个 seed 的价格 + display_name（视为已自定义）
        conn.execute(
            "UPDATE model_pricing SET input_cost_per_million='999', display_name='我的自定义' WHERE model_id='claude-sonnet-4'",
            [],
        ).unwrap();
        // 模拟发布改了 seed 默认价：把 default_pricing_map 中 claude-sonnet-4 改一个不同的 input
        // （实际实现是：seed 启动时直接读 default_pricing_map 的最新值，不需要动态改）
        // 这里我们直接验证：再跑一次 seed_default_pricing，用户改过 display_name 的行不应被覆盖
        seed_default_pricing(&conn).unwrap();
        let custom_input: String = conn
            .query_row(
                "SELECT input_cost_per_million FROM model_pricing WHERE model_id='claude-sonnet-4'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(custom_input, "999", "display_name 已改的行不应被 seed 覆盖");
        // 另一个未被用户改 display_name 的 seed,重新 seed 时若 default 改了应该被更新
        // (此处不变,但仍是 display_name == model_id 的状态,确认不抛错)
        let other_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM model_pricing WHERE display_name=model_id", [], |r| r.get(0))
            .unwrap();
        assert!(other_count > 0, "其它未被自定义的 seed 行应仍存在");
    }
}
