use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::str::FromStr;

/// 模型定价（每百万 token 的 USD 价格）
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

/// 从 usage.db 查询模型定价；未找到则从内置 seed 表加载并尝试落库。
pub fn find_model_pricing(
    conn: &rusqlite::Connection,
    model_id: &str,
) -> Option<ModelPricing> {
    let row = conn.query_row(
        "SELECT input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million
         FROM model_pricing WHERE model_id = ?1",
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
        Err(e) => {
            eprintln!("[USAGE-PRICING] 查询模型定价失败 {model_id}: {e}");
            // 尝试 seed 内置定价
            let seed = seed_pricing(model_id)?;
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

/// 内置定价 seed：model_id -> (input, output, cache_read, cache_creation, ModelPricing)
fn seed_pricing(model_id: &str) -> Option<(&'static str, &'static str, &'static str, &'static str, ModelPricing)> {
    let entry: Option<(&str, &str, &str, &str)> = match model_id {
        m if m.starts_with("claude-3-5-sonnet") => Some(("3.0", "15.0", "0.3", "3.75")),
        m if m.starts_with("claude-sonnet-4") => Some(("3.0", "15.0", "0.3", "3.75")),
        m if m.starts_with("claude-3-7-sonnet") => Some(("3.0", "15.0", "0.3", "3.75")),
        m if m.starts_with("claude-3-5-haiku") => Some(("0.80", "4.0", "0.08", "1.0")),
        m if m.starts_with("claude-3-opus") => Some(("15.0", "75.0", "1.5", "18.75")),
        m if m.starts_with("gpt-4o-mini") => Some(("0.15", "0.60", "0.075", "0.15")),
        m if m.starts_with("gpt-4o") => Some(("2.50", "10.0", "1.25", "2.50")),
        m if m.starts_with("gpt-4.1-mini") => Some(("0.40", "1.60", "0.10", "0.40")),
        m if m.starts_with("gpt-4.1-nano") => Some(("0.10", "0.40", "0.025", "0.10")),
        m if m.starts_with("gpt-4.1") => Some(("2.0", "8.0", "0.5", "2.0")),
        m if m.starts_with("o3-mini") || m.starts_with("o4-mini") => Some(("1.10", "4.40", "0.55", "1.10")),
        m if m.starts_with("o3") || m.starts_with("o4") => Some(("2.0", "8.0", "0.5", "2.0")),
        m if m.starts_with("gemini-2.5-pro") => Some(("1.25", "10.0", "0.3125", "1.25")),
        m if m.starts_with("gemini-2.5-flash") => Some(("0.30", "2.50", "0.075", "0.30")),
        m if m.starts_with("deepseek") && m.contains("reasoner") => Some(("0.55", "2.19", "0.14", "0.55")),
        m if m.starts_with("deepseek") => Some(("0.27", "1.10", "0.07", "0.27")),
        m if m.starts_with("grok-4.5-build") => Some(("2.0", "6.0", "0.30", "2.0")),
        m if m.starts_with("grok") => Some(("2.0", "6.0", "0.30", "2.0")),
        m if m.starts_with("qwen") => Some(("0.50", "2.0", "0.10", "0.50")),
        _ => None,
    };
    let entry = entry?;
    let p = ModelPricing::from_strings(entry.0, entry.1, entry.2, entry.3).ok()?;
    Some((entry.0, entry.1, entry.2, entry.3, p))
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
    fn test_seed_narrow_prefixes() {
        assert_eq!(seed_pricing("gpt-4o-mini").unwrap().0, "0.15");
        assert_eq!(seed_pricing("gpt-4o").unwrap().0, "2.50");
        assert_eq!(seed_pricing("o4-mini").unwrap().0, "1.10");
        assert_eq!(seed_pricing("grok-4.5-build").unwrap().0, "2.0");
    }
}
