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
    let token = settings.mcp_token.clone().unwrap_or_default();
    let url = format!("http://{host}:{port}/mcp");
    let proxy_running = state.storage.proxy_stats.read().running;
    Ok(serde_json::json!({
        "endpoint": url,
        "host": host,
        "port": port,
        "token": token,
        "running": proxy_running,
        "mcpServers": {
            "ai-manage": {
                "type": "http",
                "url": url,
                "headers": { "Authorization": format!("Bearer {token}") }
            }
        },
        "instructions": "Copy the mcpServers object into your MCP client's config. For Claude Desktop: %APPDATA%\\Claude\\claude_desktop_config.json. For Claude Code: ~/.claude/mcp_servers.json. For Cursor: ~/.cursor/mcp.json. Then restart the client. Make sure the AI Manage HTTP service is running first (click 'Start' on this page)."
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
