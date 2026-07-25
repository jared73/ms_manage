//! 暴露给前端的代理控制命令（get_proxy_status / start_proxy / stop_proxy）

use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_proxy_status(state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let stats = state.storage.proxy_stats.read().clone();
    // 真实 endpoint：服务在跑（running）就显示 HTTP 端口（因为我们目前只有 HTTP 模式）；
    // 没跑就按用户配置显示期望模式
    let endpoint = if stats.running {
        Some(format!("http://127.0.0.1:{}/mcp", settings.proxy_http_port))
    } else {
        match settings.proxy_mode.as_str() {
            "stdio" => Some("stdio://".to_string()),
            "http"  => Some(format!("http://127.0.0.1:{}/mcp", settings.proxy_http_port)),
            _ => None,
        }
    };
    Ok(serde_json::json!({
        "running": stats.running,
        "endpoint": endpoint,
        "connected_clients": stats.connected_clients,
        "total_requests": stats.total_requests,
        "started_at": stats.started_at,
        "last_request_at": stats.last_request_at,
    }))
}

#[tauri::command]
pub async fn get_router_config(state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let host = "127.0.0.1";
    let port = settings.proxy_http_port;
    let url = format!("http://{host}:{port}/mcp");
    let proxy_running = state.storage.proxy_stats.read().running;
    // 安全:只把 token 的掩码版发到 IPC。完整 token 仍需 `reveal_token` 命令。
    // mcpServers 块里也不再含 Authorization 头 —— 用户用 reveal_token 拿真 token
    // 再 paste 客户端,而不是从 DOM 里抓。
    let token_present = settings.mcp_token.is_some();
    let token_mask = settings.mcp_token.as_deref().map(crate::cmd_settings::mask_token);
    Ok(serde_json::json!({
        "endpoint": url,
        "host": host,
        "port": port,
        "token": token_mask,
        "token_present": token_present,
        "running": proxy_running,
        "mcpServers": {
            "ms-manage": {
                "type": "http",
                "url": url,
                "_comment": "Authorization: Bearer <token> header not included — get the token via reveal_token and paste into your client"
            }
        },
        "instructions": "Click 'Copy token' below to copy the real token, then paste the mcpServers object into your MCP client's config (Claude Desktop / Cursor / Cline / Continue / Qoder). For Claude Desktop: %APPDATA%\\Claude\\claude_desktop_config.json. For Claude Code: ~/.claude/mcp_servers.json. For Cursor: ~/.cursor/mcp.json. Restart the client after editing. Make sure the MS Manage HTTP service is running first (click 'Start' on this page)."
    }))
}

#[tauri::command]
pub async fn start_proxy(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    crate::proxy::start(&app, state.inner()).await.map_err(|e| e.to_string())?;
    get_proxy_status(state).await
}

#[tauri::command]
pub async fn stop_proxy(_state: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::proxy::stop_all().await;
    Ok(())
}

/// Dashboard 统计：所有 running server 的 tool 总数累加
#[tauri::command]
pub async fn get_tools_total(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    let reg = crate::proxy::registry();
    let ids = reg.list_ids().await;
    let mut total = 0usize;
    for id in ids {
        if let Some(child) = reg.get(&id).await {
            let srv = child.lock().await;
            total += srv.tools.len();
        }
    }
    let _ = state;
    Ok(total)
}

/// 写一条日志（GUI 端"日志"页可见）
#[tauri::command]
pub fn log_event(
    level: String,
    source: String,
    message: String,
    detail: Option<String>,
    state: State<'_, Arc<crate::state::AppState>>,
) {
    let _ = state;
    use crate::storage::LogRecord;
    let log = LogRecord {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level,
        source,
        message,
        detail,
    };
    if let Some(app_state) = crate::state::try_current() {
        let mut logs = app_state.storage.logs.write();
        logs.push(log);
        // 超过 5000 条丢最老的
        if logs.len() > 5000 {
            let drop = logs.len() - 5000;
            logs.drain(0..drop);
        }
    }
}

/// Dashboard 用：聚合所有 running server 的 tool 列表（每个带 server 前缀），
/// 让用户在概览页就能看到"Qoder 现在能调哪些 tool"。
/// 注意：会按 server.disabled_tools 过滤，跟 proxy::http_tools_list 行为一致。
#[tauri::command]
pub async fn get_exposed_tools(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let reg = crate::proxy::registry();
    let ids = reg.list_ids().await;

    // 用注入的 state 读 servers（之前误用 AppState::initialize() 创建全新实例，浪费 IO）。
    // 如果 servers.json 损坏，返回错误而非静默返回空列表。
    let servers = state.storage.load_servers().map_err(|e| e.to_string())?;
    let disabled_map: std::collections::HashMap<String, std::collections::HashSet<String>> =
        servers.into_iter()
            .map(|s| (s.id, s.disabled_tools.into_iter().collect()))
            .collect();

    let mut out: Vec<serde_json::Value> = Vec::new();
    for id in ids {
        let disabled = disabled_map.get(&id).cloned().unwrap_or_default();
        if let Some(child) = reg.get(&id).await {
            let srv = child.lock().await;
            for tool in &srv.tools {
                let orig_name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                if disabled.contains(orig_name.as_str()) {
                    continue;  // 用户在 GUI 里关过的，过滤掉
                }
                let mut t = tool.clone();
                if let Some(obj) = t.as_object_mut() {
                    let desc = obj.get("description").and_then(|d| d.as_str()).map(|s| s.to_string());
                    obj.insert("name".into(), serde_json::Value::String(format!("{}__{}", srv.name, orig_name)));
                    if let Some(d) = desc {
                        obj.insert("description".into(), serde_json::Value::String(d));
                    }
                    obj.insert("__server".into(), serde_json::Value::String(srv.name.clone()));
                }
                out.push(t);
            }
        }
    }
    Ok(out)
}
