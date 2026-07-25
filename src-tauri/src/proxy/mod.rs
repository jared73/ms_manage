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

/// 写一条日志到 AppState.logs（GUI "日志" 页可见）。
/// 全局可用：proxy 核心、HTTP 连接层、stdio 转发都走这里。
pub(crate) fn log_event(level: &str, source: &str, message: String, detail: Option<String>) {
    let rec = crate::storage::LogRecord {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: level.to_string(),
        source: source.to_string(),
        message,
        detail,
    };
    if let Some(state) = crate::state::try_current() {
        let mut logs = state.storage.logs.write();
        logs.push(rec);
        if logs.len() > 5000 {
            let drop = logs.len() - 5000;
            logs.drain(0..drop);
        }
    }
}

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
    /// stderr 在 spawn 后立即被 `drain_stderr` 接管为后台任务；
    /// 这里保留 Option 让调用方知道 stderr 是否成功接管。
    pub stderr: Option<tokio::process::ChildStderr>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub tools: Vec<serde_json::Value>, // 缓存 tools/list 结果
    pub next_id: u64,
    /// 已经从 stdout 读出但还没匹配的 JSON-RPC 帧缓冲。
    /// 用于过滤 server 在响应前发的 notification / 日志行。
    pub pending: std::collections::VecDeque<String>,
}

/// 后台消费子进程 stderr 直至 EOF，避免 pipe 满阻塞子进程。
/// 行级日志只在 RUST_LOG/MS_MANAGE_DEBUG 下输出。
///
/// 安全/性能考虑:
/// - 截断单行 4 KB —— npm/uv install 失败时经常一行 stack trace 几百 KB,
///   不截断会塞满 state.storage.logs 5000 容量,挤掉用户的关键日志。
/// - 检测 binary / 不可打印字符(除 \t)→ 记为 "binary blob, N bytes" 不写原文。
fn drain_stderr(label: String, stderr: tokio::process::ChildStderr) {
    const MAX_LINE_BYTES: usize = 4 * 1024;
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if line.is_empty() {
                continue;
            }
            // 单行超长截断 + 标注被截掉的字节数
            let (summary, original_len) = if line.len() > MAX_LINE_BYTES {
                let cut = line.floor_char_boundary(MAX_LINE_BYTES);
                // 找出 floor_char_boundary 后的字节数,做到 floor_char_boundary 对 UTF-8 安全
                let drop = line.len() - cut;
                (format!("{}…[+{} bytes]", &line[..cut], drop), line.len())
            } else {
                (line.clone(), line.len())
            };
            // 检测 binary:控制字符(0..=31 除 \t)+ 0x7f 都算不可打印。
            // 一些 npm 输出的 ANSI 转义常带 \x1b[,但日志染色通常只占头几个字符,
            // 整体仍可读 —— 所以只有「不可打印字符占比 > 10%」时才当作 binary。
            let non_printable = summary
                .bytes()
                .filter(|b| b < &0x20 && b != &b'\t' || *b == 0x7f)
                .count();
            let printable = if original_len == 0 { 1 } else { original_len };
            if non_printable * 10 > printable {
                eprintln!(
                    "[mcp:{label}!] [binary stderr skipped, {} bytes]",
                    original_len
                );
                continue;
            }
            eprintln!("[mcp:{label}!] {summary}");
        }
    });
}

/// 从子 server 读取一行 JSON-RPC，过滤掉无 id 的 notification / 日志行。
/// 一直读到 (1) 拿到带 id 的帧，(2) EOF，(3) 超时。
/// 超时返回 `Err(io::Error)`，EOF 返回 `Ok(None)`，正常帧返回 `Ok(Some((id, json)))`。
async fn read_response_line(
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    pending: &mut std::collections::VecDeque<String>,
    timeout: std::time::Duration,
) -> std::io::Result<Option<(serde_json::Value, serde_json::Value)>> {
    loop {
        // 先看缓冲里有没有匹配的
        while let Some(line) = pending.pop_front() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if v.get("id").is_some() {
                    return Ok(Some((v.get("id").cloned().unwrap_or(serde_json::Value::Null), v)));
                }
                // notification / 进度 / 日志行 → 丢弃，继续
            }
        }
        // 缓冲空，从 stdout 读一行
        let mut line = String::new();
        let read = tokio::time::timeout(timeout, stdout.read_line(&mut line)).await;
        match read {
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "read response timed out",
                ));
            }
            Ok(Ok(0)) => return Ok(None),
            Ok(Ok(_)) => {
                pending.push_back(line);
            }
            Ok(Err(e)) => return Err(e),
        }
    }
}

/// 同 read_response_line，但接受 ChildServer 并按引用一次性借用 stdout + pending。
async fn read_response_from_child(
    srv: &mut ChildServer,
    timeout: std::time::Duration,
) -> std::io::Result<Option<(serde_json::Value, serde_json::Value)>> {
    read_response_line(&mut srv.stdout, &mut srv.pending, timeout).await
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
    inject_npm_registry(&mut c, &srv.env);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    c.kill_on_drop(true);
    crate::process_util::apply_no_window(&mut c);
    let mut child = c.spawn().map_err(|e| {
        let friendly = crate::diagnose::friendly_spawn_error(cmd_str, &e);
        log_event("warn", "server", format!("server「{}」启动失败：{friendly}", srv.name), None);
        if let Some(st) = crate::state::try_current() {
            st.storage.live_states.write().insert(srv.id.clone(), ServerStatus::Error.as_str().to_string());
        }
        anyhow::anyhow!("{}", friendly)
    })?;

    let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let stderr = child.stderr.take();
    // 立即接管 stderr 到后台 drain，否则大量 banner/日志会撑满 pipe 阻塞子进程。
    if let Some(se) = stderr {
        drain_stderr(srv.name.clone(), se);
    }

    let mut child_srv = ChildServer {
        id: srv.id.clone(),
        name: srv.name.clone(),
        child,
        stdin,
        stdout: BufReader::new(stdout),
        stderr: None, // 已被 drain_stderr 接管
        started_at: chrono::Utc::now(),
        tools: Vec::new(),
        next_id: 0,
        pending: std::collections::VecDeque::new(),
    };

    // initialize handshake
    let init_id_num = child_srv.next_id();
    let init_req = serde_json::json!({
        "jsonrpc": "2.0", "id": init_id_num, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-proxy", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    child_srv.stdin.write_all(format!("{}\n", init_req).as_bytes()).await?;
    child_srv.stdin.flush().await?;
    let init_id = serde_json::json!(init_id_num);
    let read = read_response_from_child(&mut child_srv, std::time::Duration::from_secs(15)).await;
    match read {
        Ok(Some((id, _))) if id == init_id => {} // ok
        Ok(Some((id, _))) => anyhow::bail!("initialize 响应 id 不匹配：期望 {init_id}，实际 {id}"),
        Ok(None) => anyhow::bail!("initialize: 子进程已关闭"),
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            anyhow::bail!("initialize 超时")
        }
        Err(e) => anyhow::bail!("initialize 读取失败：{e}"),
    }

    // 发送 initialized notification
    let notif = serde_json::json!({
        "jsonrpc": "2.0", "method": "notifications/initialized"
    });
    child_srv.stdin.write_all(format!("{}\n", notif).as_bytes()).await?;
    child_srv.stdin.flush().await?;

    // 拉取 tools/list 缓存
    let list_id_num = child_srv.next_id();
    let list_req = serde_json::json!({
        "jsonrpc": "2.0", "id": list_id_num, "method": "tools/list", "params": {}
    });
    child_srv.stdin.write_all(format!("{}\n", list_req).as_bytes()).await?;
    child_srv.stdin.flush().await?;
    let list_id = serde_json::json!(list_id_num);
    if let Ok(Some((id, v))) = read_response_from_child(&mut child_srv, std::time::Duration::from_secs(15)).await {
        if id == list_id {
            if let Some(tools) = v.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                child_srv.tools = tools.clone();
            }
        }
    }

    let id = srv.id.clone();
    let tool_count = child_srv.tools.len();
    let arc = registry().insert(id, child_srv).await;
    log_event("info", "server", format!("server「{}」已启动（{} 个工具）", srv.name, tool_count), None);
    Ok(arc)
}

/// 读取 settings，给 stdio child 注入 NPM_CONFIG_REGISTRY / UV_INDEX_URL 等加速环境变量。
/// `user_env` 是 server 配置里用户显式设置的 env —— 注入**不覆盖**这些值。
/// 单一来源:`crate::process_util::inject_registry_env`(避免两套实现漂移)。
fn inject_npm_registry(c: &mut Command, user_env: &HashMap<String, String>) {
    let prog = c.as_std().get_program().to_string_lossy().to_string();
    crate::process_util::inject_registry_env(c, &prog, user_env);
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
        let name = s.name.clone();
        let _ = s.child.kill().await;
        log_event("info", "server", format!("server「{name}」已停止"), None);
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
    inject_npm_registry(&mut command, &s.env);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command.kill_on_drop(true);
    crate::process_util::apply_no_window(&mut command);
    let mut c = command.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let mut stdin = c.stdin.take().ok_or_else(|| "no stdin".to_string())?;
    let stdout = c.stdout.take().ok_or_else(|| "no stdout".to_string())?;
    if let Some(se) = c.stderr.take() {
        drain_stderr(s.name.clone(), se);
    }
    let mut reader = BufReader::new(stdout);
    let mut pending: std::collections::VecDeque<String> = std::collections::VecDeque::new();

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-viewer", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    stdin.write_all(format!("{}\n", init).as_bytes()).await.map_err(|e| format!("write init: {e}"))?;
    stdin.flush().await.ok();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        read_response_line(&mut reader, &mut pending, std::time::Duration::from_secs(10)),
    ).await;
    // 检查 response id == 1 —— 老的 MCP server 可能把 id 编为字符串 "1" 而不是数字 1。
    // 用 JSON eq 而非 number 严格比较,兼容两种序列化。
    let init_ok = matches!(read, Ok(Ok(Some((ref id, _)))) if id == &serde_json::json!(1) || id.as_str() == Some("1"));
    if !init_ok {
        let _ = c.kill().await;
        return Err("initialize 未收到有效响应（可能子进程启动失败或 stdout 输出非 JSON-RPC 内容）".to_string());
    }
    // notifications/initialized（一些 server 需要）
    let notif = serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let _ = stdin.write_all(format!("{}\n", notif).as_bytes()).await;
    let _ = stdin.flush().await;
    // tools/list
    let list = serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}});
    stdin.write_all(format!("{}\n", list).as_bytes()).await.map_err(|e| format!("write list: {e}"))?;
    stdin.flush().await.ok();
    let read2 = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        read_response_line(&mut reader, &mut pending, std::time::Duration::from_secs(10)),
    ).await;
    let _ = c.kill().await;

    let tools: Vec<serde_json::Value> = match read2 {
        Ok(Ok(Some((_id, v)))) => v.get("result").and_then(|r| r.get("tools")).cloned()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
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
    if let Some(se) = c.stderr.take() {
        drain_stderr(s.name.clone(), se);
    }
    let mut reader = BufReader::new(stdout);
    let mut pending: std::collections::VecDeque<String> = std::collections::VecDeque::new();

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-test", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    let _ = stdin.write_all(format!("{}\n", init).as_bytes()).await;
    let _ = stdin.flush().await;
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        read_response_line(&mut reader, &mut pending, std::time::Duration::from_secs(10)),
    ).await;
    let list = serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
    let _ = stdin.write_all(format!("{}\n", list).as_bytes()).await;
    let _ = stdin.flush().await;
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        read_response_line(&mut reader, &mut pending, std::time::Duration::from_secs(10)),
    ).await;
    let tools_val: Option<serde_json::Value> = match read {
        Ok(Ok(Some((_id, v)))) => v.get("result").and_then(|r| r.get("tools")).cloned(),
        _ => None,
    };
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
///
/// 并发安全：用 `state.proxy_starting` 做 AtomicBool compare_exchange,
/// 保证「检查 stats.running + 标记为 starting + spawn bind」三步只被一个调用者
/// 拿到。败者直接 bail,不触发任何 spawn。
pub async fn start(_app: &AppHandle, _state: &Arc<crate::state::AppState>) -> anyhow::Result<()> {
    let st = match crate::state::try_current() {
        Some(s) => s,
        None => anyhow::bail!("app state not initialized"),
    };
    // 先看 running,再用 compare_exchange 抢占 starting 标记。
    // 两步之间理论上仍可能有一个窗口:两个调用都看到 running=false,
    // 第一个 compare_exchange 成功,第二个 compare_exchange 失败 bail —— 正是我们要的。
    {
        let stats = st.storage.proxy_stats.read();
        if stats.running {
            anyhow::bail!("代理已经在运行（http://127.0.0.1:{}/mcp）", st.storage.proxy_stats_loaded_port().unwrap_or(0));
        }
    }
    // compare_exchange:从 false 改为 true。已经是 true(说明另一个并发任务正在启动)
    // → 直接 bail,避免双 bind。
    if st.proxy_starting
        .compare_exchange(false, true, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire)
        .is_err()
    {
        anyhow::bail!("代理正在启动中,请稍候");
    }

    // 注意:从这之后任何 return 都必须把 proxy_starting reset 回 false,
    // 否则会卡死到下次重启。
    let result = start_inner(&st).await;
    // 无论成功失败,本次启动流程已结束,重置 starting 标记。
    st.proxy_starting.store(false, std::sync::atomic::Ordering::Release);
    result
}

/// 实际启动代理的执行体(单独抽出来便于在 'starting' 互斥内做兜底清理)。
async fn start_inner(st: &Arc<crate::state::AppState>) -> anyhow::Result<()> {
    let settings = st.storage.load_settings()?;
    let port = settings.proxy_http_port;
    let state = st.clone();
    // oneshot：run_http bind 成功/失败都会通知，避免"sleep 200ms 后假装已启动"
    let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<u16>>();
    // 后台 task 跑 HTTP 代理
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::proxy::connect::run_http_with_ready(state.clone(), port, Some(tx)).await {
            eprintln!("[proxy] HTTP proxy error: {e}");
            let mut stats = state.storage.proxy_stats.write();
            stats.running = false;
        }
    });
    // 等 bind 结果（最多 5s；正常是毫秒级）
    match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
        Ok(Ok(Ok(_port))) => {
            let mut stats = st.storage.proxy_stats.write();
            stats.running = true;
            stats.started_at = Some(chrono::Utc::now().to_rfc3339());
            stats.connected_clients = 0;
            stats.total_requests = 0;
            Ok(())
        }
        Ok(Ok(Err(e))) => Err(e),
        Ok(Err(_)) => anyhow::bail!("代理启动任务异常退出（未收到 bind 结果）"),
        Err(_) => anyhow::bail!("等待代理 bind 超时"),
    }
}

/// 由 Tauri command 调用：根据 server id 在 GUI 内启动单个子 server（不暴露给 Claude Desktop，仅用作调试/单 server 管理）
pub async fn start_one(srv: &StoredServer) -> anyhow::Result<()> {
    // 关键:spawn_child 内部失败时已经 log_event + 写 live_states=Error。
    // 这里 ? 立刻 return,不要再覆盖 live_states 成 Running。
    // 否则 GUI 会显示绿色 running 但实际 server 没起来,后续 stop/stop 都对不上 id。
    spawn_child(srv).await?;
    if let Some(st) = crate::state::try_current() {
        st.storage.live_states.write().insert(srv.id.clone(), ServerStatus::Running.as_str().to_string());
    }
    Ok(())
}
