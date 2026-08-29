//! Anthropic 协议客户端(MiniMax 用)
//!
//! POST {base_url}/v1/messages
//! Headers: x-api-key: <key>, anthropic-version: 2023-06-01, content-type: application/json
//! Body: { model, max_tokens, messages: [{role, content}] }
//! Response: { content: [{type: "text", text: "..."}], ... }

use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct MessageResp {
    content: Vec<RespBlock>,
}

#[derive(Debug, Deserialize)]
struct RespBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// 单次对话,返回模型回复文本。`max_tokens` 由调用方根据 context_size 决定。
pub async fn chat(
    base_url: &str,
    api_key: &str,
    model: &str,
    max_tokens: usize,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": [{"role": "user", "content": user}],
    });
    let resp = reqwest::Client::new()
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("Anthropic 请求失败 (URL: {url}): {e}"))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| anyhow!("读取响应失败:{e}"))?;
    if !status.is_success() {
        return Err(anyhow!(
            "Anthropic {} 返回 {} (URL: {}): {}",
            base_url,
            status.as_u16(),
            url,
            &text[..text.len().min(200)]
        ));
    }
    let parsed: MessageResp = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析响应失败:{e}; body: {}", &text[..text.len().min(300)]))?;
    parsed.content.into_iter()
        .find(|b| b.kind == "text")
        .map(|b| b.text)
        .ok_or_else(|| anyhow!("响应里没有 text 块"))
}

/// 解析 max_tokens —— Anthropic 要求 max_tokens <= 模型 context,我们让 caller
/// 传完整 context_size,Anthropic API 自己会校验。这里不做限制。
pub fn default_max_tokens(context_size: usize) -> usize {
    // 输出预留上限;Anthropic 至少要 1,最大 8192(老)或 128k(新)
    context_size.min(8192)
}