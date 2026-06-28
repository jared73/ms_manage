//! MCP 代理核心
//!
//! 设计：本应用既可作为 GUI 桌面应用运行，也可以 stdio 子命令方式启动，
//! 作为单一 stdio MCP server 工作，统一代理多个子 MCP server。
//!
//! Claude Desktop 配置示例：
//! ```json
//! {
//!   "mcpServers": {
//!     "ms-manage": {
//!       "command": "ms_manage.exe",
//!       "args": ["proxy", "stdio"]
//!     }
//!   }
//! }
//! ```
//!
//! 通信模型：
//! - Claude Desktop → 本应用 stdin (newline-delimited JSON-RPC)
//! - 本应用 → Claude Desktop stdout (newline-delimited JSON-RPC)
//! - 本应用 ↔ 子 MCP server 各自一对 stdin/stdout pipe
//!
//! 命名空间：
//! - 子 server `filesystem` 的 `read_file` 工具，在外暴露为 `filesystem__read_file`
//! - tools/call 时按 `__` 前缀解析路由

pub mod connect;
pub mod routing;
pub mod stdio_proxy;

use crate::storage::StoredServer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Error,
    Disabled,
}

impl ServerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Error => "error",
            Self::Disabled => "disabled",
        }
    }
}

/// 单个子 MCP server 进程 + 双向 JSON-RPC 帧通道
pub struct ChildServer {
    pub id: String,
    pub name: String,
    pub child: Child,
    pub stdin: tokio::process::ChildStdin,
    pub stdout: BufReader<tokio::process::ChildStdout>,
    pub stderr: Option<tokio::process::ChildStderr>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub tools: Vec<serde_json::Value>, // 缓存 tools/list 结果
    pub next_id: u64,
}

impl ChildServer {
    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

pub struct ServerRegistry {
    inner: Mutex<HashMap<String, Arc<Mutex<ChildServer>>>>,
}

impl Default for ServerRegistry {
    fn default() -> Self { Self { inner: Mutex::new(HashMap::new()) } }
}

impl ServerRegistry {
    pub async fn insert(&self, id: String, srv: ChildServer) -> Arc<Mutex<ChildServer>> {
        let arc = Arc::new(Mutex::new(srv));
        self.inner.lock().await.insert(id, arc.clone());
        arc
    }
    pub async fn remove(&self, id: &str) -> Option<Arc<Mutex<ChildServer>>> {
        self.inner.lock().await.remove(id)
    }
    pub async fn get(&self, id: &str) -> Option<Arc<Mutex<ChildServer>>> {
        self.inner.lock().await.get(id).cloned()
    }
    pub async fn list_ids(&self) -> Vec<String> {
        self.inner.lock().await.keys().cloned().collect()
    }
    pub async fn list(&self) -> Vec<Arc<Mutex<ChildServer>>> {
        self.inner.lock().await.values().cloned().collect()
    }
}

/// 全局 server registry（仅 stdio 代理进程内使用）
static REGISTRY: once_cell::sync::Lazy<Arc<ServerRegistry>> =
    once_cell::sync::Lazy::new(|| Arc::new(ServerRegistry::default()));

pub fn registry() -> Arc<ServerRegistry> { REGISTRY.clone() }

/// SSE session 路由：每个 SSE 长连接对应一个 session_id，
/// POST 时带 ?session_id=xxx 的请求会把响应发回对应的 SSE 流上
/// （这是 legacy SSE transport 的要求，Qoder 之类的客户端走这条路）。
type SessionTx = tokio::sync::mpsc::UnboundedSender<String>;

pub struct SessionRegistry {
    inner: tokio::sync::Mutex<std::collections::HashMap<String, SessionTx>>,
}

impl Default for SessionRegistry {
    fn default() -> Self { Self { inner: tokio::sync::Mutex::new(std::collections::HashMap::new()) } }
}

impl SessionRegistry {
    pub async fn insert(&self, id: String, tx: SessionTx) {
        self.inner.lock().await.insert(id, tx);
    }
    pub async fn remove(&self, id: &str) -> Option<SessionTx> {
        self.inner.lock().await.remove(id)
    }
    pub async fn send(&self, id: &str, msg: String) -> bool {
        if let Some(tx) = self.inner.lock().await.get(id) {
            return tx.send(msg).is_ok();
        }
        false
    }
    pub async fn has(&self, id: &str) -> bool {
        self.inner.lock().await.contains_key(id)
    }
}

static SESSIONS: once_cell::sync::Lazy<Arc<SessionRegistry>> =
    once_cell::sync::Lazy::new(|| Arc::new(SessionRegistry::default()));

pub fn sessions() -> Arc<SessionRegistry> { SESSIONS.clone() }

/// 启动一个 stdio 子 MCP server 进程
pub async fn spawn_child(srv: &StoredServer) -> anyhow::Result<Arc<Mutex<ChildServer>>> {
    if srv.transport != "stdio" {
        anyhow::bail!("proxy currently only supports stdio children, got '{}'", srv.transport);
    }
    let cmd_str = srv.command.as_deref().ok_or_else(|| anyhow::anyhow!("command is required"))?;
    if cmd_str.trim().is_empty() {
        anyhow::bail!("command 字段为空，请在编辑对话框中填写可执行文件路径（如 npx、uvx、python 等）");
    }
    // Windows 上 npx / uvx 这种命令需要先解析成绝对路径，
    // uvx 再绕过到缓存的 .exe（避免 uvx 内部 conhost 闪窗）。
    let resolved = crate::process_util::resolve_cmd(cmd_str);
    let (final_cmd, final_args) = crate::process_util::resolve_spawn_command(&resolved, &srv.args);

    // .cmd/.bat 直接解析成真实 .exe + .js 脚本（彻底绕过 cmd.exe，不闪窗）
    let mut c = if cfg!(windows) && final_cmd.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .map_or(false, |e| e == "cmd" || e == "bat")
    {
        // 解析 npx.cmd → (node.exe, [npx-cli.js])
        match crate::process_util::parse_cmd_file(&final_cmd) {
            Some((real_exe, mut extra_args)) => {
                extra_args.extend(final_args.iter().cloned());
                let mut c = Command::new(&real_exe);
                c.args(extra_args.iter());
                c
            }
            None => {
                // 解析失败（不是 npx/npm .cmd），fallback 到 cmd.exe /c
                let mut c = Command::new("cmd.exe");
                c.arg("/c").arg(&final_cmd);
                c.args(final_args.iter());
                c
            }
        }
    } else {
        // .exe 直接 spawn
        let mut c = Command::new(&final_cmd);
        c.args(final_args.iter());
        c
    };
    if let Some(cwd) = &srv.cwd { c.current_dir(cwd); }
    for (k, v) in &srv.env { c.env(k, v); }
    inject_npm_registry(&mut c);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    c.kill_on_drop(true);
    crate::process_util::apply_no_window(&mut c);
    let mut child = c.spawn().map_err(|e| {
        let friendly = crate::diagnose::friendly_spawn_error(cmd_str, &e);
        if let Some(st) = crate::state::try_current() {
            st.storage.live_states.write().insert(srv.id.clone(), ServerStatus::Error.as_str().to_string());
        }
        anyhow::anyhow!("{}", friendly)
    })?;

    let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let stderr = child.stderr.take();

    let mut child_srv = ChildServer {
        id: srv.id.clone(),
        name: srv.name.clone(),
        child,
        stdin,
        stdout: BufReader::new(stdout),
        stderr,
        started_at: chrono::Utc::now(),
        tools: Vec::new(),
        next_id: 0,
    };

    // initialize handshake
    let init_req = serde_json::json!({
        "jsonrpc": "2.0", "id": child_srv.next_id(), "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-proxy", "version": "0.1.0" }
        }
    });
    child_srv.stdin.write_all(format!("{}\n", init_req).as_bytes()).await?;
    child_srv.stdin.flush().await?;
    let mut line = String::new();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        child_srv.stdout.read_line(&mut line),
    ).await;
    if read.is_err() || line.trim().is_empty() {
        anyhow::bail!("initialize timed out or empty response: {line:?}");
    }
    let _: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|e| anyhow::anyhow!("invalid initialize response: {e}: {line}"))?;

    // 发送 initialized notification
    let notif = serde_json::json!({
        "jsonrpc": "2.0", "method": "notifications/initialized"
    });
    child_srv.stdin.write_all(format!("{}\n", notif).as_bytes()).await?;
    child_srv.stdin.flush().await?;

    // 拉取 tools/list 缓存
    let list_req = serde_json::json!({
        "jsonrpc": "2.0", "id": child_srv.next_id(), "method": "tools/list", "params": {}
    });
    child_srv.stdin.write_all(format!("{}\n", list_req).as_bytes()).await?;
    child_srv.stdin.flush().await?;
    let mut line = String::new();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        child_srv.stdout.read_line(&mut line),
    ).await;
    if let Ok(Ok(_)) = read {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if let Some(tools) = v.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                child_srv.tools = tools.clone();
            }
        }
    }

    let id = srv.id.clone();
    Ok(registry().insert(id, child_srv).await)
}

/// 读取 settings，给 stdio child 注入 NPM_CONFIG_REGISTRY / UV_INDEX_URL 等加速环境变量
fn inject_npm_registry(c: &mut Command) {
    let Some(state) = crate::state::try_current() else { return; };
    let Ok(settings) = state.storage.load_settings() else { return; };
    let cmd_path = c.as_std().get_program();
    let stem = std::path::Path::new(cmd_path).file_stem()
        .and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

    // Node.js 包管理器加速（默认淘宝镜像）
    if matches!(stem.as_str(), "npx" | "npm" | "pnpm" | "pnpx" | "yarn" | "bun" | "bunx" | "node") {
        if settings.auto_inject_npm_registry {
            let npm_registry = settings.npm_registry.trim();
            if !npm_registry.is_empty() {
                c.env("NPM_CONFIG_REGISTRY", npm_registry);
                c.env("npm_config_registry", npm_registry);
                c.env("npm_registry", npm_registry);
            }
        }
    }
    // uv 加速（Python MCP server）
    if matches!(stem.as_str(), "uv" | "uvx") {
        c.env("UV_INDEX_URL", "https://mirrors.aliyun.com/pypi/simple/");
        c.env("PIP_INDEX_URL", "https://mirrors.aliyun.com/pypi/simple/");
    }
}

/// 关闭所有子进程
pub async fn stop_all() {
    let ids = registry().list_ids().await;
    for id in ids {
        if let Some(srv) = registry().remove(&id).await {
            let mut s = srv.lock().await;
            let _ = s.child.kill().await;
        }
    }
    if let Some(st) = crate::state::try_current() {
        let mut stats = st.storage.proxy_stats.write();
        stats.running = false;
        stats.connected_clients = 0;
    }
}

/// 关闭单个子 server（按 id）
pub async fn stop(id: &str) {
    if let Some(srv) = registry().remove(id).await {
        let mut s = srv.lock().await;
        let _ = s.child.kill().await;
    }
    if let Some(st) = crate::state::try_current() {
        st.storage.live_states.write().insert(id.to_string(), ServerStatus::Stopped.as_str().to_string());
    }
}

/// 仅拉取 tools/list（spawn→init→tools/list→kill），返回 tools 数组
pub async fn fetch_tools(s: &StoredServer) -> Result<Vec<serde_json::Value>, String> {
    if s.transport != "stdio" {
        return Err(format!("only stdio is supported ({} transport)", s.transport));
    }
    let cmd_str = s.command.as_deref().filter(|c| !c.is_empty())
        .ok_or_else(|| "command is empty".to_string())?;
    let resolved = crate::process_util::resolve_cmd(cmd_str);
    let (final_cmd, final_args) = crate::process_util::resolve_spawn_command(&resolved, &s.args);
    // .cmd/.bat 直接解析成真实 .exe（彻底绕过 cmd.exe，不闪窗）
    let mut command = if cfg!(windows) && final_cmd.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .map_or(false, |e| e == "cmd" || e == "bat")
    {
        match crate::process_util::parse_cmd_file(&final_cmd) {
            Some((real_exe, mut extra_args)) => {
                extra_args.extend(final_args.iter().cloned());
                let mut c = Command::new(&real_exe);
                c.args(extra_args.iter());
                c
            }
            None => {
                let mut c = Command::new("cmd.exe");
                c.arg("/c").arg(&final_cmd);
                c.args(final_args.iter());
                c
            }
        }
    } else {
        let mut c = Command::new(&final_cmd);
        c.args(final_args.iter());
        c
    };
    if let Some(cwd) = &s.cwd { command.current_dir(cwd); }
    for (k, v) in &s.env { command.env(k, v); }
    inject_npm_registry(&mut command);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command.kill_on_drop(true);
    crate::process_util::apply_no_window(&mut command);
    let mut c = command.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let mut stdin = c.stdin.take().ok_or_else(|| "no stdin".to_string())?;
    let stdout = c.stdout.take().ok_or_else(|| "no stdout".to_string())?;
    let mut reader = BufReader::new(stdout);

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-viewer", "version": "0.1.0" }
        }
    });
    stdin.write_all(format!("{}\n", init).as_bytes()).await.map_err(|e| format!("write init: {e}"))?;
    stdin.flush().await.ok();
    let mut buf = String::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), reader.read_line(&mut buf)).await;
    // notifications/initialized（一些 server 需要）
    let notif = serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let _ = stdin.write_all(format!("{}\n", notif).as_bytes()).await;
    let _ = stdin.flush().await;
    // tools/list
    let list = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    stdin.write_all(format!("{}\n", list).as_bytes()).await.map_err(|e| format!("write list: {e}"))?;
    stdin.flush().await.ok();
    buf.clear();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), reader.read_line(&mut buf)).await;
    let _ = c.kill().await;

    let tools: Vec<serde_json::Value> = serde_json::from_str::<serde_json::Value>(&buf).ok()
        .and_then(|v| v.get("result").and_then(|r| r.get("tools")).cloned())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    Ok(tools)
}

/// 测试单个 server（spawn→initialize→tools/list→kill）
pub async fn test(s: &StoredServer) -> Result<serde_json::Value, String> {
    if s.transport != "stdio" {
        return Ok(serde_json::json!({ "ok": false, "error": format!("only stdio is testable in MVP ({} transport)", s.transport) }));
    }
    let cmd_str = match s.command.as_deref() {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(serde_json::json!({ "ok": false, "error": "command is empty" })),
    };
    let resolved = crate::process_util::resolve_cmd(cmd_str);
    let started = std::time::Instant::now();
    let (final_cmd, final_args) = crate::process_util::resolve_spawn_command(&resolved, &s.args);
    // .cmd/.bat 直接解析成真实 .exe（彻底绕过 cmd.exe，不闪窗）
    let mut command = if cfg!(windows) && final_cmd.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .map_or(false, |e| e == "cmd" || e == "bat")
    {
        match crate::process_util::parse_cmd_file(&final_cmd) {
            Some((real_exe, mut extra_args)) => {
                extra_args.extend(final_args.iter().cloned());
                let mut c = Command::new(&real_exe);
                c.args(extra_args.iter());
                c
            }
            None => {
                let mut c = Command::new("cmd.exe");
                c.arg("/c").arg(&final_cmd);
                c.args(final_args.iter());
                c
            }
        }
    } else {
        let mut c = Command::new(&final_cmd);
        c.args(final_args.iter());
        c
    };
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command.kill_on_drop(true);
    crate::process_util::apply_no_window(&mut command);
    let mut c = match command.spawn() {
        Ok(c) => c,
        Err(e) => return Ok(serde_json::json!({ "ok": false, "error": format!("spawn failed: {e}") })),
    };
    let mut stdin = c.stdin.take().ok_or_else(|| "stdin".to_string())?;
    let stdout = c.stdout.take().ok_or_else(|| "stdout".to_string())?;
    let mut reader = BufReader::new(stdout);

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-test", "version": "0.1.0" }
        }
    });
    let _ = stdin.write_all(format!("{}\n", init).as_bytes()).await;
    let _ = stdin.flush().await;
    let mut buf = String::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), reader.read_line(&mut buf)).await;
    let list = serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
    let _ = stdin.write_all(format!("{}\n", list).as_bytes()).await;
    let _ = stdin.flush().await;
    buf.clear();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), reader.read_line(&mut buf)).await;
    let tools_val: Option<serde_json::Value> = serde_json::from_str(&buf).ok()
        .and_then(|v: serde_json::Value| v.get("result").and_then(|r| r.get("tools")).cloned());
    let _ = c.kill().await;
    let ms = started.elapsed().as_millis() as u64;
    Ok(serde_json::json!({
        "ok": true,
        "latency_ms": ms,
        "tools": tools_val.unwrap_or(serde_json::Value::Null),
    }))
}

/// 启动 stdio 代理（在 CLI 子命令调用）
pub async fn run_stdio() -> anyhow::Result<()> {
    stdio_proxy::run().await
}

/// 启动 GUI 内的代理（HTTP 模式 tokio task）。
///
/// 注意：stdio 模式不能从 GUI 启动（GUI 进程 stdin/stdout 被 WebView 占用），
/// 需用 CLI：`ms_manage.exe proxy stdio`
pub async fn start(_app: &AppHandle, _state: &Arc<crate::state::AppState>) -> anyhow::Result<()> {
    let st = match crate::state::try_current() {
        Some(s) => s,
        None => anyhow::bail!("app state not initialized"),
    };
    // 已经有 running 的 task？
    {
        let stats = st.storage.proxy_stats.read();
        if stats.running {
            anyhow::bail!("代理已经在运行（http://127.0.0.1:{}/mcp）", st.storage.proxy_stats_loaded_port().unwrap_or(0));
        }
    }
    let settings = st.storage.load_settings()?;
    let port = settings.proxy_http_port;
    let state = st.clone();
    // 后台 task 跑 HTTP 代理
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::proxy::connect::run_http(state.clone(), port).await {
            eprintln!("[proxy] HTTP proxy error: {e}");
            let mut stats = state.storage.proxy_stats.write();
            stats.running = false;
        }
    });
    // 给前端一点时间 listen
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    {
        let mut stats = st.storage.proxy_stats.write();
        stats.running = true;
        stats.started_at = Some(chrono::Utc::now().to_rfc3339());
        stats.connected_clients = 0;
        stats.total_requests = 0;
    }
    Ok(())
}

/// 由 Tauri command 调用：根据 server id 在 GUI 内启动单个子 server（不暴露给 Claude Desktop，仅用作调试/单 server 管理）
pub async fn start_one(srv: &StoredServer) -> anyhow::Result<()> {
    let _ = spawn_child(srv).await?;
    if let Some(st) = crate::state::try_current() {
        st.storage.live_states.write().insert(srv.id.clone(), ServerStatus::Running.as_str().to_string());
    }
    Ok(())
}
