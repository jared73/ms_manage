//! HTTP/SSE 代理 + MCPR_TOKEN 鉴权（学 mcp-router）
//!
//! 启动后在 127.0.0.1:<port> 暴露：
//! - POST /mcp           —— JSON-RPC over HTTP
//! - GET  /mcp/sse       —— Server-Sent Events 流
//!
//! 鉴权：客户端请求必须带 `Authorization: Bearer <token>`，
//! token 从 settings.json 读（首次启动随机生成，存盘）。
//!
//! 启用场景：Claude Code / Cursor / Cline / Windsurf 等支持远程 MCP 的客户端。
//! Claude Desktop 仍走 stdio 模式。

use crate::proxy::registry;
use crate::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;

/// 写一条日志到 AppState.logs（GUI "日志" 页可见）
fn log(level: &str, source: &str, message: String, detail: Option<String>) {
    super::log_event(level, source, message, detail);
}
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

/// 单个请求 body 上限：16MB。防止本机恶意/异常进程用巨大 Content-Length
/// 触发超额分配或让连接 task 永久挂起。
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// 读取 body 的总超时
const BODY_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

pub async fn run_http(state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    run_http_with_ready(state, port, None).await
}

/// 启动 HTTP 代理；`ready` 提供时，bind 成功后发 Ok(())，bind 失败发 Err，
/// 调用方可以可靠地知道代理是否真正起来了（而不是靠 sleep 猜）。
pub async fn run_http_with_ready(
    state: Arc<AppState>,
    port: u16,
    ready: Option<tokio::sync::oneshot::Sender<anyhow::Result<u16>>>,
) -> anyhow::Result<()> {
    // 默认只监听 127.0.0.1，避免向局域网暴露 MCP proxy。
    // 如需远程访问，后续可在设置中增加 "bind_address" 配置项。
    let listen_addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = match TcpListener::bind(&listen_addr).await {
        Ok(l) => l,
        Err(e) => {
            if let Some(tx) = ready {
                let _ = tx.send(Err(anyhow::anyhow!("bind 127.0.0.1:{port} 失败：{e}")));
            }
            return Err(anyhow::anyhow!("bind 127.0.0.1:{port} 失败：{e}"));
        }
    };
    let local = listener.local_addr().ok();
    if let Some(tx) = ready {
        let _ = tx.send(Ok(port));
    }
    let initial_token = state.storage.load_settings()
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .mcp_token
        .clone()
        .unwrap_or_default();
    eprintln!("[ms-manage] HTTP MCP proxy listening on http://{local:?}");
    // token 前 8 位只用于开发/调试期核对 —— release build 完全禁止打印
    #[cfg(debug_assertions)]
    {
        let token_preview: String = initial_token.chars().take(8).collect();
        eprintln!("[ms-manage] token preview (Authorization: Bearer ...): {token_preview}...");
    }
    log("info", "proxy", format!("HTTP MCP proxy listening on 127.0.0.1:{port}"), None);

    loop {
        let (stream, _peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, state).await {
                eprintln!("[ms-manage] conn error: {e}");
                log("error", "proxy", format!("连接处理失败：{e}"), None);
            }
        });
    }
}

async fn handle_conn(mut stream: TcpStream, state: Arc<AppState>) -> anyhow::Result<()> {
    let (read, mut write) = stream.split();
    let mut reader = BufReader::new(read);

    // 极简 HTTP 解析 —— 只支持 GET / POST，足够 MCP 协议
    // 限制请求行和单个头部的大小，防止恶意本地进程发送超长行耗尽内存。
    const MAX_REQUEST_LINE: usize = 8192;
    const MAX_HEADER_LINE: usize = 8192;
    const MAX_HEADERS: usize = 64;

    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    if request_line.len() > MAX_REQUEST_LINE {
        anyhow::bail!("request line too long: {} bytes", request_line.len());
    }
    let mut headers = std::collections::HashMap::new();
    let mut content_length: usize = 0;
    let mut header_count = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 || line == "\r\n" || line == "\n" { break; }
        header_count += 1;
        if header_count > MAX_HEADERS {
            anyhow::bail!("too many headers: >{MAX_HEADERS}");
        }
        if line.len() > MAX_HEADER_LINE {
            anyhow::bail!("header line too long: {} bytes", line.len());
        }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_lowercase();
            let v = v.trim().to_string();
            if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            }
            headers.insert(k, v);
        }
    }

    // 解析 query string（用于 token 鉴权 + session_id 路由）
    let mut query_token = String::new();
    let mut query_session_id = String::new();
    if let Some(q_start) = request_line.find('?') {
        if let Some(q_end) = request_line[q_start..].find(' ') {
            let qs = &request_line[q_start + 1..q_start + q_end];
            for kv in qs.split('&') {
                if let Some((k, v)) = kv.split_once('=') {
                    match k {
                        "token" | "access_token" => query_token = urldecode(v),
                        "session_id" => query_session_id = urldecode(v),
                        _ => {}
                    }
                }
            }
        }
    }

    let auth = headers.get("authorization").cloned().unwrap_or_default();
    let expected_token = state.storage.load_settings()
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .mcp_token
        .clone()
        .unwrap_or_default();
    // 空 token 视为未配置：直接拒绝所有需要鉴权的请求，
    // 避免 settings 里 mcp_token="" 时任何人都能裸连。
    let bearer_ok = !expected_token.is_empty() && auth == format!("Bearer {expected_token}");
    let query_ok = !expected_token.is_empty() && !query_token.is_empty() && query_token == expected_token;
    let token_ok = bearer_ok || query_ok;
    if !token_ok {
        // 健康检查 / SSE endpoint / OPTIONS 都不需要 token（GET 试探开放）
        let (m2, p2) = {
            let line = request_line.trim();
            let mut parts = line.split_whitespace();
            (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string())
        };
        if (m2 == "GET" && (p2 == "/" || p2 == "/health"))
            || m2 == "OPTIONS"
            || (m2 == "GET" && (p2 == "/mcp/sse" || p2 == "/sse" || p2 == "/sse/"))
        {
            // 走下面的主路由（auth 仍记 MISSING 但不 401）
        } else {
            let path_for_log = request_line.split_whitespace().nth(1).unwrap_or("")
                .split('?').next().unwrap_or("").to_string();
            log("warn", "proxy.http", format!("鉴权失败 401：{m2} {path_for_log}"), None);
            let _ = write_all(&mut write, b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n").await;
            return Ok(());
        }
    }

    // 路由
    let (method, path) = {
        let line = request_line.trim();
        let mut parts = line.split_whitespace();
        let m = parts.next().unwrap_or("").to_string();
        let p = parts.next().unwrap_or("").to_string();
        (m, p)
    };
    // 把 query string 从 path 里剥掉（`/mcp/sse?token=xxx` → `/mcp/sse`）。
    // 注意：日志里只能出现 path_no_q —— path 含完整 query string，
    // 而 `?token=xxx` 是支持的鉴权方式，落日志等于把 token 写进日志文件。
    let path_no_q = path.split('?').next().unwrap_or(&path).to_string();
    log("debug", "proxy.http", format!("{} {}", method, path_no_q), None);
    eprintln!("[proxy] {} {} (auth={})", method, path_no_q,
        if token_ok { "OK" } else { "MISSING" });

    // Content-Length 上限保护：超过 16MB 直接 413，
    // 否则 vec![0u8; content_length] 会被本机任意进程用来 OOM。
    if content_length > MAX_BODY_BYTES {
        let _ = write_all(&mut write,
            b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n").await;
        log("warn", "proxy.http", format!("拒绝超大请求体：{} bytes (上限 {})", content_length, MAX_BODY_BYTES), None);
        return Ok(());
    }

    match (method.as_str(), path_no_q.as_str()) {
        ("POST", "/mcp") => {
            if let Some(st) = crate::state::try_current() {
                let mut stats = st.storage.proxy_stats.write();
                stats.total_requests += 1;
                stats.last_request_at = Some(chrono::Utc::now().to_rfc3339());
            }
            let mut body = vec![0u8; content_length];
            tokio::time::timeout(
                BODY_READ_TIMEOUT,
                tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body),
            ).await
                .map_err(|_| anyhow::anyhow!("读取请求体超时"))??;
            let body_str = String::from_utf8_lossy(&body).to_string();
            // 两种 transport 路径：
            //   1) 带 ?session_id=xxx → legacy SSE transport：
            //      POST 立刻返回 202，响应通过 SSE 流送回。Qoder 走这条路。
            //   2) 不带 session_id → streamable_http / 单请求模式：
            //      POST 直接返回 SSE-format 响应在 body 里。
            if !query_session_id.is_empty() && crate::proxy::sessions().has(&query_session_id).await {
                // 路径 1：session 路由
                let resp = handle_jsonrpc_to_string(&state, &body_str).await?;
                if let Some(json) = resp {
                    let event = format!("event: message\r\ndata: {json}\r\n\r\n");
                    let _ = crate::proxy::sessions().send(&query_session_id, event).await;
                }
                let _ = write_all(&mut write,
                    b"HTTP/1.1 202 Accepted\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\n\r\n"
                ).await;
            } else {
                // 路径 2：直接返回（SSE 帧格式）
                handle_jsonrpc(&state, &body_str, &mut write).await?;
            }
        }
        // Qoder / 一些客户端直接用 EventSource(configured_url) 打开 SSE 流，
        // 不走 /mcp/sse 而是走配置的 /mcp。这种情况下必须返回 SSE + endpoint 事件，
        // 否则客户端等不到事件就 timeout waiting for endpoint。
        ("GET", "/mcp") | ("GET", "/mcp/") => {
            serve_sse_session(&mut write, state.clone()).await?;
        }
        // SSE 兼容：旧版 MCP SSE transport + 一些客户端默认路径
        ("GET", "/mcp/sse") | ("GET", "/sse") | ("GET", "/sse/") => {
            serve_sse_session(&mut write, state.clone()).await?;
        }
        // 健康检查 + 根路径
        ("GET", "/") | ("GET", "/health") => {
            let resp = format!(
                "{{\"status\":\"ok\",\"proxy\":\"ms-manage\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
            let _ = write_all(&mut write,
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\r\n{resp}").as_bytes()
            ).await;
        }
        // CORS preflight
        ("OPTIONS", _) => {
            let _ = write_all(&mut write,
                b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nAccess-Control-Max-Age: 86400\r\n\r\n"
            ).await;
        }
        // 未知路径：返回 405 + Allow header 而不是 404，避免客户端重试循环
        _ if method == "POST" => {
            // 任何 POST 都当作 JSON-RPC 处理（宽容模式）
            if let Some(st) = crate::state::try_current() {
                let mut stats = st.storage.proxy_stats.write();
                stats.total_requests += 1;
                stats.last_request_at = Some(chrono::Utc::now().to_rfc3339());
            }
            let mut body = vec![0u8; content_length];
            let _ = tokio::time::timeout(
                BODY_READ_TIMEOUT,
                tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body),
            ).await;
            let body_str = String::from_utf8_lossy(&body).to_string();
            eprintln!("[proxy] unknown POST path {}, treating as /mcp", path_no_q);
            // 注意：body 不落日志 —— tool call 参数里可能含密钥。
            log("warn", "proxy.http", format!("未知 POST 路径 {}, 当 /mcp 处理 ({} bytes)", path_no_q, body_str.len()), None);
            handle_jsonrpc(&state, &body_str, &mut write).await?;
        }
        _ => {
            eprintln!("[proxy] unknown {} {}, returning 405", method, path_no_q);
            log("warn", "proxy.http", format!("未知 {} {}, 返回 405", method, path_no_q), None);
            let _ = write_all(&mut write,
                b"HTTP/1.1 405 Method Not Allowed\r\nAccess-Control-Allow-Origin: *\r\nAllow: GET, POST, OPTIONS\r\nContent-Length: 0\r\n\r\n"
            ).await;
        }
    }
    Ok(())
}

async fn write_all<W: tokio::io::AsyncWrite + Unpin>(w: &mut W, buf: &[u8]) -> std::io::Result<()> {
    w.write_all(buf).await?;
    w.flush().await
}

/// 简单 URL decode（仅处理 %XX 和 +）
fn urldecode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// 处理单个 JSON-RPC 请求，把响应 JSON 序列化为字符串（用于 SSE session 路由）
async fn handle_jsonrpc_to_string(state: &AppState, body: &str) -> anyhow::Result<Option<String>> {
    let req: serde_json::Value = serde_json::from_str(body)?;
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
    let params = req.get("params").cloned().unwrap_or(serde_json::Value::Null);

    let resp = match method.as_str() {
        "initialize" => {
            let client = params.get("clientInfo").and_then(|c| c.get("name")).and_then(|n| n.as_str()).unwrap_or("unknown");
            log("info", "mcp", format!("客户端 initialize：{client}"), None);
            Some(serde_json::json!({
                "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
                "result": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "ms-manage-http-proxy", "version": env!("CARGO_PKG_VERSION") }
                }
            }))
        }
        "ping" => Some(jsonrpc_ok(id, serde_json::json!({}))),
        "tools/list" => Some(http_tools_list(state, id).await),
        "tools/call" => Some(http_tools_call(state, id, params).await),
        _ => {
            if id.is_some() {
                Some(serde_json::json!({
                    "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
                    "error": { "code": -32601, "message": format!("Method not found: {method}") }
                }))
            } else { None }
        }
    };

    Ok(resp.map(|r| serde_json::to_string(&r).unwrap_or_default()))
}

/// SSE 长连接处理：发 endpoint 事件 + 持有 session + 转发响应 + 心跳
async fn serve_sse_session<W: tokio::io::AsyncWrite + Unpin>(
    write: &mut W,
    _state: Arc<AppState>,
) -> anyhow::Result<()> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    crate::proxy::sessions().insert(session_id.clone(), tx).await;
    log("info", "client", format!("SSE 客户端已连接（session {}）", &session_id[..8]), None);

    let _ = write_all(write,
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\nX-Accel-Buffering: no\r\n\r\n"
    ).await;
    // endpoint 事件：把 session_id 放在 query string 里，client POST 时带上
    let endpoint_data = format!("/mcp?session_id={session_id}");
    let _ = write_all(write,
        format!("event: endpoint\r\ndata: {endpoint_data}\r\n\r\n").as_bytes()
    ).await;

    loop {
        tokio::select! {
            // POST handler 通过 channel 推过来的响应
            Some(msg) = rx.recv() => {
                if write_all(write, msg.as_bytes()).await.is_err() { break; }
            }
            // 15s 心跳保活（SSE 客户端会断开不活跃的连接）
            _ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {
                if write_all(write, b": ping\r\n\r\n").await.is_err() { break; }
            }
        }
    }
    crate::proxy::sessions().remove(&session_id).await;
    log("info", "client", format!("SSE 客户端已断开（session {}）", &session_id[..8]), None);
    Ok(())
}

/// 处理单个 JSON-RPC 请求（MCP 协议），返回响应字符串
async fn handle_jsonrpc<W: tokio::io::AsyncWrite + Unpin>(
    state: &AppState,
    body: &str,
    write: &mut W,
) -> anyhow::Result<()> {
    let req: serde_json::Value = serde_json::from_str(body)?;
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
    let params = req.get("params").cloned().unwrap_or(serde_json::Value::Null);

    let resp = match method.as_str() {
        "initialize" => {
            let client = params.get("clientInfo").and_then(|c| c.get("name")).and_then(|n| n.as_str()).unwrap_or("unknown");
            log("info", "mcp", format!("客户端 initialize：{client}"), None);
            Some(serde_json::json!({
                "jsonrpc": "2.0", "id": id.clone().unwrap_or(serde_json::Value::Null),
                "result": {
                    "protocolVersion": "2025-03-26",
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "ms-manage-http-proxy", "version": env!("CARGO_PKG_VERSION") }
                }
            }))
        }
        "ping" => Some(jsonrpc_ok(id.clone(), serde_json::json!({}))),
        "tools/list" => Some(http_tools_list(state, id.clone()).await),
        "tools/call" => Some(http_tools_call(state, id.clone(), params).await),
        _ => {
            if id.is_some() {
                Some(serde_json::json!({
                    "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
                    "error": { "code": -32601, "message": format!("Method not found: {method}") }
                }))
            } else { None }
        }
    };

    if let Some(r) = resp {
        let s = serde_json::to_string(&r)?;
        // Streamable HTTP transport 要求 POST /mcp 的响应是 text/event-stream 格式
        // （event: message + data: <json>\n\n），不能用裸 application/json，
        // 否则 Qoder / 现代 Claude Desktop 这类客户端会卡在 "waiting for endpoint"。
        // 同时保留裸 JSON 作为 fallback：很多简单 client 两种都吃。
        let body = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\n\r\nevent: message\r\ndata: {s}\r\n\r\n"
        );
        write_all(write, body.as_bytes()).await?;
    } else {
        // notification（无 id）：204 No Content 即可，客户端不期待响应
        write_all(write, b"HTTP/1.1 204 No Content\r\n\r\n").await?;
    }
    Ok(())
}

fn jsonrpc_ok(id: Option<serde_json::Value>, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null), "result": result })
}

async fn http_tools_list(state: &AppState, id: Option<serde_json::Value>) -> serde_json::Value {
    let servers = match state.storage.load_servers() {
        Ok(s) => s,
        Err(e) => return serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32603, "message": format!("load: {e}") }
        }),
    };
    // 只返回已在 registry 中运行的 server 的 tools（不自动 spawn）
    let mut all_tools: Vec<serde_json::Value> = Vec::new();
    let reg = registry();
    for s in &servers {
        if !s.enabled || s.transport != "stdio" {
            continue;
        }
        // 过滤掉用户在 GUI 里关闭的工具
        let disabled: std::collections::HashSet<&str> = s.disabled_tools.iter().map(|s| s.as_str()).collect();
        // 只处理已在 registry 中的 server（用户手动启动的）
        if let Some(child) = reg.get(&s.id).await {
            let srv = child.lock().await;
            // iter() 而非 drain(..) —— 客户端会反复调 tools/list，
            // drain 会把缓存清空，第二次起就拿不到工具了
            for tool in srv.tools.iter() {
                let orig_name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                if disabled.contains(orig_name.as_str()) {
                    continue;
                }
                let mut t = tool.clone();
                if let Some(obj) = t.as_object_mut() {
                    obj.insert("name".into(), serde_json::Value::String(format!("{}__{}", s.name, orig_name)));
                }
                all_tools.push(t);
            }
        }
    }
    jsonrpc_ok(id, serde_json::json!({ "tools": all_tools }))
}

async fn http_tools_call(state: &AppState, id: Option<serde_json::Value>, params: serde_json::Value) -> serde_json::Value {
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let arguments = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
    let (server_name, tool_name) = match name.split_once("__") {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => return serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32602, "message": format!("tool name '{name}' must be prefixed with '<server>__'") }
        }),
    };
    let servers = match state.storage.load_servers() {
        Ok(s) => s,
        Err(e) => return serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32603, "message": format!("load: {e}") }
        }),
    };
    let srv = match servers.iter().find(|s| s.name == server_name && s.enabled) {
        Some(s) => s.clone(),
        None => return serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32602, "message": format!("server '{server_name}' not found or disabled") }
        }),
    };
    // 只允许调用已在 registry 中的 server（用户手动启动的）
    let reg = registry();
    let child = match reg.get(&srv.id).await {
        Some(c) => c,
        None => return serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32603, "message": format!("server '{server_name}' is not running, please start it first") }
        }),
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
        log("warn", "mcp.tools", format!("tools/call {server_name}__{tool_name} 写入失败：{e}"), None);
        return serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32603, "message": format!("write: {e}") }
        });
    }
    let _ = child.stdin.flush().await;
    let read = crate::proxy::read_response_from_child(
        &mut child,
        std::time::Duration::from_secs(120),
    ).await;
    match read {
        Ok(Some((_id, mut v))) => {
            let is_err = v.get("error").is_some()
                || v.get("result").and_then(|r| r.get("isError")).and_then(|b| b.as_bool()).unwrap_or(false);
            log("debug", "mcp.tools", format!("tools/call {server_name}__{tool_name} {}", if is_err { "→ 错误" } else { "→ 成功" }), None);
            // 把子 server 的 id 替换成 client 的 id
            if let Some(obj) = v.as_object_mut() {
                obj.insert("id".into(), id.unwrap_or(serde_json::Value::Null));
            }
            v
        }
        Ok(None) => {
            log("warn", "mcp.tools", format!("tools/call {server_name}__{tool_name} 子进程已关闭"), None);
            serde_json::json!({
                "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
                "error": { "code": -32603, "message": format!("{server_name} 子进程已关闭") }
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            log("warn", "mcp.tools", format!("tools/call {server_name}__{tool_name} 超时"), None);
            serde_json::json!({
                "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
                "error": { "code": -32603, "message": "timeout" }
            })
        }
        Err(e) => serde_json::json!({
            "jsonrpc": "2.0", "id": id.unwrap_or(serde_json::Value::Null),
            "error": { "code": -32603, "message": format!("read {server_name}: {e}") }
        }),
    }
}
