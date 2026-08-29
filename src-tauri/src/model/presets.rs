//! 3 个 LLM 预设 —— 都用 Anthropic 协议
//!
//! baseURL 借鉴 cc-switch:
//! - MiniMax: `https://api.MiniMax.cn`(直接接 Anthropic 协议)
//! - DeepSeek: `https://api.deepseek.com/anthropic`(挂 /anthropic 兼容子路径)
//! - Kimi: `https://api.moonshot.cn/anthropic`(挂 /anthropic 兼容子路径)
//!
//! 注:3 家 chat 走 Anthropic 协议,但拉模型都暴露 OpenAI 兼容的 GET /v1/models
//! 端点(在去掉 /anthropic 后缀后的根路径)。详见 openai.rs 的候选 URL 策略。

use super::ModelPreset;

/// 给前端展示的所有 preset
pub fn all_presets() -> Vec<ModelPreset> {
    vec![
        ModelPreset {
            id: "minimax".into(),
            name: "MiniMax".into(),
            protocol: "anthropic".into(),
            // 域名从 cc-switch 抄来 —— 国内是 minimaxi.com(注意域名里多一个 "ni"),
            // 不是 MiniMax.cn。我之前配错所以一直 404。
            // 国际版是 api.minimax.io,需要切的话手动改 baseURL。
            default_base_url: "https://api.minimaxi.com/anthropic".into(),
            default_model: "MiniMax-M2.7".into(),
            // MiniMax-M2 系列支持 1M context,用户确认最大 1M
            default_context_size: 200_000,
            context_options: vec![8_192, 32_000, 128_000, 256_000, 1_000_000],
            // 内置常见模型 —— 即使 /v1/models 接口 404 也能选
            known_models: vec![
                "MiniMax-M2.7".into(),
                "MiniMax-M2.7-highspeed".into(),
                "MiniMax-Text-01".into(),
                "MiniMax-VL-01".into(),
            ],
        },
        ModelPreset {
            id: "deepseek".into(),
            name: "DeepSeek".into(),
            protocol: "anthropic".into(),
            default_base_url: "https://api.deepseek.com/anthropic".into(),
            default_model: "deepseek-chat".into(),
            default_context_size: 64_000,
            context_options: vec![8_192, 32_000, 64_000],
            known_models: vec![
                "deepseek-chat".into(),
                "deepseek-reasoner".into(),
                "deepseek-coder".into(),
            ],
        },
        ModelPreset {
            id: "kimi".into(),
            name: "Kimi (Moonshot)".into(),
            protocol: "anthropic".into(),
            default_base_url: "https://api.moonshot.cn/anthropic".into(),
            // 从 cc-switch 抄:Kimi Coding 默认模型是 kimi-k2.7-code
            default_model: "kimi-k2.7-code".into(),
            default_context_size: 128_000,
            context_options: vec![8_192, 32_000, 128_000],
            known_models: vec![
                "kimi-k2.7-code".into(),
                "kimi-k2-0711-preview".into(),
                "moonshot-v1-8k".into(),
                "moonshot-v1-32k".into(),
                "moonshot-v1-128k".into(),
            ],
        },
    ]
}

/// 按 id 查 preset
pub fn preset_by_id(id: &str) -> Option<ModelPreset> {
    all_presets().into_iter().find(|p| p.id == id)
}