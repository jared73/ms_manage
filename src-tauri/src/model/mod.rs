//! LLM 模型配置 + skill 解析模块
//!
//! 设计：4 个国内主流 LLM 预设(MiniMax/DeepSeek/Qwen/GLM) + 自定义 endpoint。
//! MiniMax 用 Anthropic 协议,其他三家都用 OpenAI 协议兼容接口。
//!
//! 不会执行 skill,只是用 LLM 阅读 SKILL.md 后给一段中文摘要,存到独立 JSON 文件
//! (不污染 SKILL.md)。用户场景:看到一堆英文描述的 skill 时,中文摘要一眼能明白用途。

pub mod analyze;
pub mod anthropic;
pub mod openai;
pub mod presets;
pub mod storage;

use serde::{Deserialize, Serialize};

/// 模型配置(用户填的) —— 持久化到 settings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// preset id:`minimax` / `deepseek` / `qwen` / `zhipu` / `custom`
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    /// 上下文大小(用作 max_tokens 上限)。常见值 4096 / 8192 / 16384 / 32768 / 65536 / 128000 / 200000
    pub context_size: usize,
}

/// 给前端的 preset 清单(不包含敏感信息)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreset {
    pub id: String,
    pub name: String,
    /// `anthropic` / `openai`
    pub protocol: String,
    pub default_base_url: String,
    pub default_model: String,
    /// 默认推荐的上下文大小
    pub default_context_size: usize,
    /// 列出该 preset 所有预设的预设上下文大小选项,前端直接渲染
    pub context_options: Vec<usize>,
    /// 该供应商常见模型清单 —— 即使 /v1/models 接口拉不到也能选
    #[serde(default)]
    pub known_models: Vec<String>,
}

/// 单个 skill 的解析结果(中文摘要)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAnalysis {
    pub skill_id: String,
    pub summary_zh: String,
    pub analyzed_at: String,    // ISO8601
    pub model: String,          // 哪个模型产生的(便于追溯)
}

impl ModelConfig {
    /// 3 个国内供应商都用 Anthropic 协议
    pub fn protocol(&self) -> &'static str {
        "anthropic"
    }
}