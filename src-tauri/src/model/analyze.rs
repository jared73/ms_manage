//! 用 LLM 解析单个 skill:读取 SKILL.md → 调模型 → 存到 analyses.json
//!
//! Prompt 设计要点:
//! - 强制中文输出
//! - 1-2 句,80 字以内
//! - 模板稳定,避免模型自由发挥导致输出格式漂移
//! - 解析失败时返回原始错误,不静默吞

use super::{anthropic, ModelConfig, SkillAnalysis};
use crate::state::AppState;
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

const SYSTEM_PROMPT: &str = "你是 skill 摘要助手。用户会给你一段 SKILL.md 内容,你需要用 1-2 句简体中文简要说明这个 skill 是做什么的、什么场景下使用。要求:简洁(80 字以内)、不要重复 skill 名字、不要说\"这个 skill\"、不要堆砌营销词、用平实语言。只输出摘要本身,不要加标题/前缀/引号。";

const USER_TEMPLATE: &str = "SKILL.md 内容:\n---\n{content}\n---\n请输出 1-2 句中文摘要:";

/// 单 skill 解析 —— 读 SKILL.md → 调 LLM → 写 analyses.json → 返回新结果
pub async fn analyze_one(
    data_dir: &Path,
    cfg: &ModelConfig,
    skill_id: &str,
    skill_md_content: &str,
) -> Result<SkillAnalysis> {
    // 截断过长内容,避免 prompt 超限
    let truncated = if skill_md_content.len() > 6000 {
        format!("{}\n\n[内容过长,已截断]", &skill_md_content[..6000])
    } else {
        skill_md_content.to_string()
    };
    let user_msg = USER_TEMPLATE.replace("{content}", &truncated);
    let max_tokens = 512.min(cfg.context_size);

    let text = anthropic::chat(
        &cfg.base_url, &cfg.api_key, &cfg.model,
        max_tokens, SYSTEM_PROMPT, &user_msg,
    ).await?;

    // 后处理:去掉模型偶尔包的外层引号/换行
    let cleaned = text.trim().trim_matches('"').trim().to_string();
    if cleaned.is_empty() {
        return Err(anyhow!("模型返回空内容"));
    }

    let analysis = SkillAnalysis {
        skill_id: skill_id.to_string(),
        summary_zh: cleaned,
        analyzed_at: Utc::now().to_rfc3339(),
        model: cfg.model.clone(),
    };
    crate::model::storage::upsert(data_dir, &analysis)?;
    Ok(analysis)
}

/// 全局更新所有 skill —— 并发但有上限,带进度事件
pub async fn analyze_all(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    cfg: ModelConfig,
) -> Result<usize, String> {
    let skills = state.storage.load_skills().map_err(|e| e.to_string())?;
    let data_dir = state.data_dir.clone();
    let cfg = cfg;
    let total = skills.len();
    let _ = app.emit("skill_analysis_progress", serde_json::json!({
        "done": 0, "total": total, "current": ""
    }));

    let mut results: Vec<SkillAnalysis> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // 简单串行 + 进度事件 —— skill 数 < 50,没必要并发
    // (并发也要考虑供应商 rate limit,串行更稳)
    for (idx, skill) in skills.iter().enumerate() {
        let _ = app.emit("skill_analysis_progress", serde_json::json!({
            "done": idx, "total": total, "current": skill.name
        }));
        let content = match std::fs::read_to_string(std::path::Path::new(&skill.dir_path).join("SKILL.md")) {
            Ok(c) => c,
            Err(e) => { errors.push(format!("{}: 读 SKILL.md 失败 {e}", skill.name)); continue; }
        };
        match analyze_one(&data_dir, &cfg, &skill.id, &content).await {
            Ok(a) => results.push(a),
            Err(e) => errors.push(format!("{}: {e}", skill.name)),
        }
    }

    let _ = app.emit("skill_analysis_progress", serde_json::json!({
        "done": total, "total": total, "current": ""
    }));

    if !errors.is_empty() {
        // 不整体失败,把错误塞到事件
        let _ = app.emit("skill_analysis_errors", errors);
    }
    Ok(results.len())
}

/// 用 Arc 共享 cfg 的 helper —— `analyze_all` 在 async 上下文里反复用 cfg
pub fn _shared_cfg(cfg: ModelConfig) -> Arc<ModelConfig> {
    Arc::new(cfg)
}