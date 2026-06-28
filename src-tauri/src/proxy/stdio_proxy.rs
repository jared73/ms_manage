//! stdio 代理主循环：
//! 从 stdin 读取 JSON-RPC 请求，按 server 路由到对应子 MCP server，
//! 把响应写回 stdout。

use crate::proxy::registry;
use crate::storage::Storage;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_INFO: &str = "ms-manage-proxy";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 入口：启动 stdio 代理（CLI 子命令 `ms_manage proxy stdio` 调用）
pub async fn run() -> anyhow::Result<()> {
    // 初始化 AppState（和 GUI 用同一个数据目录 + migration + settings）
    let state = crate::state::AppState::initialize()
        .map_err(|e| anyhow::anyhow!("init app state: {e}"))?;
    let state = std::sync::Arc::new(state);
    state.clone().install_as_current();
    let storage = state.storage.clone();

    // 自动启动所有 enabled + auto_start 的 server（和 GUI 行为一致）
    if let Ok(servers) = storage.load_servers() {
        for srv in &servers {
            if srv.enabled && srv.auto_start.unwrap_or(false) && srv.transport == "stdio" {
                if let Err(e) = crate::proxy::start_one(srv).await {
                    eprintln!("[auto-start] failed to start {}: {}", srv.name, e);
                }
            }
        }
    }

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    use tokio::io::AsyncWriteExt;

    // 主循环
    while let Ok(Some(line)) = reader.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = serde_json::json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                });
                let s = serde_json::to_string(&err).unwrap_or_default();
                let _ = stdout.write_all(s.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
                continue;
            }
        };
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        let resp = match method.as_str() {
            "initialize" => handle_initialize(id.clone(), params),
            "ping" => jsonrpc_ok(id.clone(), serde_json::json!({})),
            "tools/list" => handle_tools_list(&storage, id.clone()).await,
            "tools/call" => handle_tools_call(&storage, id.clone(), params).await,
            "resources/list" => jsonrpc_ok(id.clone(), serde_json::json!({ "resources": [] })),
            "resources/read" => jsonrpc_error(id.clone(), -32602, "resources not supported by proxy"),
            "prompts/list" => jsonrpc_ok(id.clone(), serde_json::json!({ "prompts": [] })),
            "prompts/get" => jsonrpc_error(id.clone(), -32602, "prompts not supported by proxy"),
            _ => {
                if id.is_some() {
                    jsonrpc_error(id.clone(), -32601, format!("Method not found: {method}"))
                } else {
                    continue;
                }
            }
        };

        if let Some(resp_value) = resp {
            if let Ok(s) = serde_json::to_string(&resp_value) {
                let _ = stdout.write_all(s.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
            }
        }
    }
    Ok(())
}

fn jsonrpc_ok(id: Option<Value>, result: Value) -> Option<Value> {
    id.map(|i| serde_json::json!({ "jsonrpc": "2.0", "id": i, "result": result }))
}

fn jsonrpc_error(id: Option<Value>, code: i32, message: impl Into<String>) -> Option<Value> {
    Some(serde_json::json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message.into() }
    }))
}

fn handle_initialize(id: Option<Value>, _params: Value) -> Option<Value> {
    jsonrpc_ok(id, serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_INFO, "version": SERVER_VERSION }
    }))
}

async fn handle_tools_list(storage: &Storage, id: Option<Value>) -> Option<Value> {
    let servers = match storage.load_servers() {
        Ok(s) => s,
        Err(e) => return jsonrpc_error(id, -32603, format!("load servers: {e}")),
    };
    // 只返回已在 registry 中运行的 server 的 tools（不自动 spawn）
    let mut all_tools: Vec<Value> = Vec::new();
    let reg = registry();

    for s in &servers {
        if !s.enabled || s.transport != "stdio" {
            continue;
        }
        // 只处理已在 registry 中的 server（用户手动启动的）
        if let Some(child) = reg.get(&s.id).await {
            let srv = child.lock().await;
            // iter() 而非 drain(..) —— 客户端会反复调 tools/list，
            // drain 会把缓存清空，第二次起就拿不到工具了
            for tool in srv.tools.iter() {
                let mut t = tool.clone();
                if let Some(obj) = t.as_object_mut() {
                    let orig = obj.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                    obj.insert("name".into(), Value::String(format!("{}__{}", s.name, orig)));
                    obj.insert("description".into(), Value::String(
                        format!("[{}] {}", s.name, obj.get("description").and_then(|d| d.as_str()).unwrap_or(""))
                    ));
                }
                all_tools.push(t);
            }
        }
    }
    jsonrpc_ok(id, serde_json::json!({ "tools": all_tools }))
}

async fn handle_tools_call(storage: &Storage, id: Option<Value>, params: Value) -> Option<Value> {
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let (server_name, tool_name) = match name.split_once("__") {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => return jsonrpc_error(id, -32602, format!("tool name '{name}' must be prefixed with '<server>__'")),
    };

    let servers = match storage.load_servers() {
        Ok(s) => s,
        Err(e) => return jsonrpc_error(id, -32603, format!("load servers: {e}")),
    };
    let srv = match servers.iter().find(|s| s.name == server_name && s.enabled) {
        Some(s) => s.clone(),
        None => return jsonrpc_error(id, -32602, format!("server '{server_name}' not found or disabled")),
    };

    // 只允许调用已在 registry 中的 server（用户手动启动的）
    let reg = registry();
    let child = match reg.get(&srv.id).await {
        Some(c) => c,
        None => return jsonrpc_error(id, -32603, format!("server '{server_name}' is not running, please start it first")),
    };
    let mut child = child.lock().await;
    let call_id = child.next_id();
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": call_id,
        "method": "tools/call",
        "params": { "name": tool_name, "arguments": arguments }
    });
    use tokio::io::AsyncWriteExt;
    if let Err(e) = child.stdin.write_all(format!("{}\n", req).as_bytes()).await {
        return jsonrpc_error(id, -32603, format!("write to {server_name}: {e}"));
    }
    if let Err(e) = child.stdin.flush().await {
        return jsonrpc_error(id, -32603, format!("flush {server_name}: {e}"));
    }

    // 读取响应（带超时）
    let mut line = String::new();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        child.stdout.read_line(&mut line),
    ).await;
    match read {
        Ok(Ok(_)) => match serde_json::from_str::<Value>(line.trim()) {
            Ok(mut v) => {
                // 把子 server 的 id 替换成 client 的 id
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("id".into(), id.clone().unwrap_or(Value::Null));
                }
                Some(v)
            }
            Err(e) => jsonrpc_error(id, -32603, format!("invalid response from {server_name}: {e}")),
        },
        Ok(Err(e)) => jsonrpc_error(id, -32603, format!("read {server_name}: {e}")),
        Err(_) => jsonrpc_error(id, -32603, format!("timeout waiting for {server_name}")),
    }
}
