//! 用 OpenAI 兼容的 GET /v1/models 端点拉模型列表
//!
//! 借鉴 cc-switch 的候选 URL 策略 —— DeepSeek/Kimi 的 baseURL 末尾挂
//! `/anthropic` 兼容子路径,直接拼 `/v1/models` 会得到 .../anthropic/v1/models 404。
//! 必须先剥掉已知兼容后缀,再按顺序试多个候选。
//!
//! 参考:cc-switch/src-tauri/src/services/model_fetch.rs
//!
//! 已知兼容后缀按长度降序排,确保最长前缀优先命中。
const KNOWN_COMPAT_SUFFIXES: &[&str] = &[
    "/api/claudecode",
    "/api/anthropic",
    "/apps/anthropic",
    "/api/coding",
    "/claudecode",
    "/anthropic",
    "/step_plan",
    "/coding",
    "/claude",
];

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

///  FetchedModel 给前端展示
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchedModel {
    pub id: String,
    pub owned_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Option<Vec<ModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    owned_by: Option<String>,
}

const FETCH_TIMEOUT_SECS: u64 = 15;

/// 拉模型列表 —— 候选 URL 全部失败才返回错误
pub async fn list_models(base_url: &str, api_key: &str) -> Result<Vec<FetchedModel>> {
    if api_key.trim().is_empty() {
        return Err(anyhow!("API Key 不能为空"));
    }
    let candidates = build_candidates(base_url);
    if candidates.is_empty() {
        return Err(anyhow!("无法从 base_url 推导 models 端点"));
    }

    let client = reqwest::Client::new();
    let mut last_err: Option<String> = None;

    for url in &candidates {
        let resp = match client
            .get(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => { last_err = Some(format!("请求失败:{e}")); continue; }
        };

        let status = resp.status();
        if status.is_success() {
            let parsed: ModelsResponse = match resp.json().await {
                Ok(p) => p,
                Err(e) => {
                    last_err = Some(format!("解析失败:{e}"));
                    continue;
                }
            };
            let mut models: Vec<FetchedModel> = parsed.data.unwrap_or_default()
                .into_iter()
                .map(|m| FetchedModel { id: m.id, owned_by: m.owned_by })
                .collect();
            models.sort_by(|a, b| a.id.cmp(&b.id));
            return Ok(models);
        }
        // 404/405 → 这个候选不对,试下一个
        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            last_err = Some(format!("HTTP {status}"));
            continue;
        }
        // 其他错误(401/403/500) → 立即返回,不浪费后续候选
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("HTTP {}: {}", status.as_u16(), truncate_body(&body, 300)));
    }

    Err(anyhow!("所有候选 URL 均失败:{}", last_err.unwrap_or_else(|| "未知错误".to_string())))
}

/// 构造候选 URL(借鉴 cc-switch,简化版)
fn build_candidates(base_url: &str) -> Vec<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return vec![];
    }
    let mut candidates: Vec<String> = Vec::new();

    // baseURL 已以版本段 /v{N} 结尾时(如 /v1、/v4),模型端点是 {base}/models
    // 否则补 /v1/models
    if ends_with_version_segment(trimmed) {
        candidates.push(format!("{trimmed}/models"));
        if !trimmed.ends_with("/v1") {
            candidates.push(format!("{trimmed}/v1/models"));
        }
    } else {
        candidates.push(format!("{trimmed}/v1/models"));
    }

    // 命中已知兼容后缀 → 剥离后追加 /v1/models、/models
    if let Some(stripped) = strip_compat_suffix(trimmed) {
        let root = stripped.trim_end_matches('/');
        if !root.is_empty() && root.contains("://") {
            candidates.push(format!("{root}/v1/models"));
            candidates.push(format!("{root}/models"));
        }
    }

    // 去重保序
    let mut unique: Vec<String> = Vec::with_capacity(candidates.len());
    for url in candidates {
        if !unique.iter().any(|u| u == &url) {
            unique.push(url);
        }
    }
    unique
}

fn strip_compat_suffix(base_url: &str) -> Option<&str> {
    for suffix in KNOWN_COMPAT_SUFFIXES {
        if base_url.ends_with(*suffix) {
            return Some(&base_url[..base_url.len() - suffix.len()]);
        }
    }
    None
}

fn ends_with_version_segment(url: &str) -> bool {
    let last = url.rsplit('/').next().unwrap_or("");
    last.strip_prefix('v')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

fn truncate_body(body: &str, max: usize) -> String {
    if body.chars().count() <= max {
        body.to_string()
    } else {
        let mut s: String = body.chars().take(max).collect();
        s.push('…');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_root() {
        assert_eq!(build_candidates("https://api.MiniMax.cn"), vec!["https://api.MiniMax.cn/v1/models"]);
    }

    #[test]
    fn trailing_slash() {
        assert_eq!(build_candidates("https://api.example.com/"), vec!["https://api.example.com/v1/models"]);
    }

    #[test]
    fn deepseek_anthropic_suffix() {
        let c = build_candidates("https://api.deepseek.com/anthropic");
        // 候选必须包含剥后缀后的正确路径(没这个 strip_compat_suffix 就是摆设)
        assert!(c.contains(&"https://api.deepseek.com/v1/models".to_string()), "候选:{:?}", c);
    }

    #[test]
    fn kimi_anthropic_suffix() {
        let c = build_candidates("https://api.moonshot.cn/anthropic");
        assert!(c.contains(&"https://api.moonshot.cn/v1/models".to_string()), "候选:{:?}", c);
    }

    #[test]
    fn v4_suffix_uses_models_not_v1_models() {
        let c = build_candidates("https://open.bigmodel.cn/api/coding/paas/v4");
        assert_eq!(c[0], "https://open.bigmodel.cn/api/coding/paas/v4/models");
    }

    #[test]
    fn ends_with_v() {
        assert!(ends_with_version_segment("https://x.com/v1"));
        assert!(ends_with_version_segment("https://x.com/v4"));
        assert!(!ends_with_version_segment("https://x.com/vX"));
        assert!(!ends_with_version_segment("https://x.com/anthropic"));
    }
}