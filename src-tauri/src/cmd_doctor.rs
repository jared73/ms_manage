//! 一键体检：检查当前机器是否具备运行 MCP server / 接入客户端的条件。
//! 每项返回 状态 + 说明 + 修复建议，前端卡片化展示。

use serde::Serialize;
use std::sync::Arc;
use tauri::State;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct CheckItem {
    /// 机器可读的 key，前端用来分组/图标
    pub id: String,
    pub title: String,
    /// ok | warn | fail
    pub status: String,
    /// 检查到的实际情况（如版本号、源地址）
    pub detail: String,
    /// 不通过时的修复建议（可以是命令）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

fn ok(id: &str, title: &str, detail: String) -> CheckItem {
    CheckItem { id: id.into(), title: title.into(), status: "ok".into(), detail, suggestion: None }
}
fn warn(id: &str, title: &str, detail: String, suggestion: String) -> CheckItem {
    CheckItem { id: id.into(), title: title.into(), status: "warn".into(), detail, suggestion: Some(suggestion) }
}
fn fail(id: &str, title: &str, detail: String, suggestion: String) -> CheckItem {
    CheckItem { id: id.into(), title: title.into(), status: "fail".into(), detail, suggestion: Some(suggestion) }
}

/// 解析 `node --version` / `npm --version` 输出，取主版本号。
fn major_of(ver: &str) -> Option<u32> {
    ver.trim().trim_start_matches('v').split('.').next()?.parse().ok()
}

async fn check_tool(id: &str, title: &str, prog: &str, min_major: u32, install_hint: &str) -> CheckItem {
    match crate::process_util::run_capture_output(prog, &["--version"]).await {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            match major_of(&ver) {
                Some(m) if m >= min_major => ok(id, title, format!("已安装 {ver}")),
                Some(_) => fail(id, title, format!("版本过低 {ver}（需要 >= {min_major}）"), install_hint.into()),
                None => ok(id, title, format!("已安装 {ver}")),
            }
        }
        _ => fail(id, title, format!("未检测到 {prog}"), install_hint.into()),
    }
}

#[tauri::command]
pub async fn doctor_check(state: State<'_, Arc<AppState>>) -> Result<Vec<CheckItem>, String> {
    let mut items: Vec<CheckItem> = Vec::new();

    let settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let npm_registry = settings.npm_registry.trim().to_string();

    // 1. Node.js（CLI 需要 >=18）
    items.push(check_tool(
        "node", "Node.js", "node", 18,
        "安装 Node.js 18+：https://nodejs.org （客户端用 npx 接入必须）",
    ).await);

    // 2. npm
    items.push(check_tool(
        "npm", "npm", "npm", 6,
        "npm 随 Node.js 一起安装，重装 Node.js 即可",
    ).await);

    // 3. npm registry 可达性（HTTP 探测 —— `npm ping` 对 npmmirror 等源不可靠，会误报失败）
    let registry_to_test = if npm_registry.is_empty() { "https://registry.npmjs.org".to_string() } else { npm_registry.clone() };
    let reg_ok = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map(|c| {
            let url = registry_to_test.clone();
            async move { c.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false) }
        });
    let reg_ok = match reg_ok {
        Ok(fut) => fut.await,
        Err(_) => false,
    };
    if reg_ok {
        items.push(ok("registry", "npm 源可达", format!("{registry_to_test} 连接正常")));
    } else {
        items.push(fail(
            "registry", "npm 源不可达",
            format!("无法连接 {registry_to_test}"),
            "检查网络/代理；国内建议换源：npm config set registry https://registry.npmmirror.com".into(),
        ));
    }

    // 4. npm 镜像是否国内源（慢/被墙预警）
    let is_cn_mirror = registry_to_test.contains("npmmirror")
        || registry_to_test.contains("taobao")
        || registry_to_test.contains("tuna")
        || registry_to_test.contains("aliyun")
        || registry_to_test.contains("cnpm");
    if !is_cn_mirror && !registry_to_test.is_empty() {
        items.push(warn(
            "npm_mirror", "npm 源（国内加速）",
            format!("当前源 {registry_to_test}，国内可能较慢"),
            "建议换国内源：npm config set registry https://registry.npmmirror.com".into(),
        ));
    } else {
        items.push(ok("npm_mirror", "npm 源（国内加速）", format!("已使用国内镜像 {registry_to_test}")));
    }

    // 5. uv（Python 类 server）
    match crate::process_util::run_capture_output("uv", &["--version"]).await {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            items.push(ok("uv", "uv / uvx", format!("已安装 {ver}")));
        }
        _ => items.push(warn(
            "uv", "uv / uvx", "未检测到 uv".into(),
            "运行 Python 类 MCP server（uvx）需要。安装：https://docs.astral.sh/uv/".into(),
        )),
    }

    // 6. MS Manage proxy 是否在监听
    let proxy_running = state.storage.proxy_stats.read().running;
    if proxy_running {
        items.push(ok("proxy", "MS Manage 代理", format!("HTTP 代理运行中（127.0.0.1:{}）", settings.proxy_http_port)));
    } else {
        items.push(fail(
            "proxy", "MS Manage 代理", "代理未运行".into(),
            "重启 MS Manage；客户端接入依赖此代理".into(),
        ));
    }

    Ok(items)
}
