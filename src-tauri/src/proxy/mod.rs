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
use std::time::Instant;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// SSE session map 上限。local-only 端口本机恶意进程可以无限 GET /mcp/sse,
/// 没上限就 OOM。加常量上限,connect.rs 收到 insert 失败就 503 + 关流。
pub(crate) const MAX_SESSIONS: usize = 256;
/// 单 session 静默超时:超过这个时间没活动就视为 stale,
/// 后台 cleanup task 每 60s 清一次。
pub(crate) const SESSION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
/// 后台 cleanup 周期。
pub(crate) const SESSION_CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
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

/// 单个子 server 的元数据(id/name/tools/next_id/start time)。
///
/// 锁策略:`ChildEntry::meta` 用 `parking_lot::RwLock` 持有。
/// 读多写少:tools 偶发 refresh,next_id 每请求 +1,持锁时间短(纳秒级)。
pub struct ChildMeta {
    pub id: String,
    pub name: String,
    pub tools: Vec<serde_json::Value>, // 缓存 tools/list 结果
    pub next_id: u64,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl ChildMeta {
    pub fn next_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

/// 子 server 的 IO 资源(stdin/stdout/pending) + drain 任务 + kill 信号。
/// 通过 `ChildEntry::io` (`tokio::sync::Mutex<ChildIo>`) 互斥访问,
/// 只在 tools/call 写入 + 读取响应期间持锁(最多 120s)。
pub struct ChildIo {
    pub stdin: tokio::process::ChildStdin,
    pub stdout: BufReader<tokio::process::ChildStdout>,
    /// stderr 已被 `drain_stderr` 接管(返回时通过 `kill_tx` 通知退出);
    /// 这里保留 Option 占位以保持字段一致(`spawn_child` 给 None)。
    pub stderr: Option<tokio::process::ChildStderr>,
    /// 已经从 stdout 读出但还没匹配的 JSON-RPC 帧缓冲。
    /// 用于过滤 server 在响应前发的 notification / 日志行。
    pub pending: std::collections::VecDeque<String>,
    pub drain_handle: Option<tokio::task::JoinHandle<()>>,
    /// 给 `drain_stderr` 用的 kill 信号。
    /// `Option` 是因为 `kill_and_drain` 在 `let _ = tx.send(())` 时需要 take 走。
    pub kill_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

/// 单个子 MCP server 进程 + 双向 JSON-RPC 帧通道。
///
/// 拆分动机:原 `Arc<Mutex<ChildServer>>` 把握住 stdin/stdout 整个 tools/call
/// 期间(最多 120s),Dashboard `get_exposed_tools` 永远拿不到锁。拆为三段:
/// - `meta`:读写都轻,`parking_lot::RwLock` 持锁时间短(纳秒级)
/// - `io`:只在 write+read 期间持锁,响应到了立即释放
/// - `child`:专用 `Mutex<Child>`,只在 stop 时 `.kill().await`
pub struct ChildEntry {
    pub meta: parking_lot::RwLock<ChildMeta>,
    pub io: tokio::sync::Mutex<ChildIo>,
    pub child: tokio::sync::Mutex<tokio::process::Child>,
}

/// 兼容老 API: `child.lock().await` 返回 ChildMeta 读 guard。
/// `proxy_endpoint.rs` / stdio 早期路径只读 tools/name,无需 IO 锁。
/// 内部走 `parking_lot::RwLock::read()`,瞬间,支持并发读。
pub struct ChildMetaGuard<'a> {
    _guard: parking_lot::RwLockReadGuard<'a, ChildMeta>,
}

impl std::ops::Deref for ChildMetaGuard<'_> {
    type Target = ChildMeta;
    fn deref(&self) -> &ChildMeta { &*self._guard }
}

impl ChildEntry {
    /// 兼容老 API: `child.lock().await` -> 读 ChildMeta 字段。
    /// 实现是 `parking_lot::RwLock::read()` 的 sync 调用,被 async fn 包装
    /// 以兼容 `.await` 调用约定(body 无 await,Future 立即 Ready)。
    pub async fn lock(&self) -> ChildMetaGuard<'_> {
        ChildMetaGuard { _guard: self.meta.read() }
    }
}

/// 后台消费子进程 stderr 直至 EOF / kill 信号，避免 pipe 满阻塞子进程。
/// 行级日志只在 RUST_LOG/MS_MANAGE_DEBUG 下输出。
///
/// 安全/性能考虑:
/// - 截断单行 4 KB —— npm/uv install 失败时经常一行 stack trace 几百 KB,
///   不截断会塞满 state.storage.logs 5000 容量,挤掉用户的关键日志。
/// - 检测 binary / 不可打印字符(除 \t)→ 记为 "binary blob, N bytes" 不写原文。
/// - `kill_rx` 触发时立即退出循环,配合 `stop_all` 快速回收后台任务。
fn drain_stderr(
    label: String,
    stderr: tokio::process::ChildStderr,
    mut kill_rx: tokio::sync::oneshot::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    const MAX_LINE_BYTES: usize = 4 * 1024;
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = BufReader::new(stderr).lines();
        loop {
            tokio::select! {
                // 优先级:kill 信号一到立即退出,不等下一次 I/O 唤醒。
                _ = &mut kill_rx => break,
                line = reader.next_line() => {
                    match line {
                        Ok(Some(line)) => {
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
                        _ => break,
                    }
                }
            }
        }
    })
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

/// 同 read_response_line，但接受 ChildIo 并按引用一次性借用 stdout + pending。
///
/// 三个返回路径(语义不变):
/// - `Ok(Some((id, json)))` —— 正常拿到带 id 的帧
/// - `Ok(None)` —— 子进程已关闭(EOF)
/// - `Err(io::Error)` of `TimedOut` —— 读取超时
async fn read_response_from_child(
    io: &mut ChildIo,
    timeout: std::time::Duration,
) -> std::io::Result<Option<(serde_json::Value, serde_json::Value)>> {
    read_response_line(&mut io.stdout, &mut io.pending, timeout).await
}

pub struct ServerRegistry {
    inner: tokio::sync::Mutex<HashMap<String, Arc<ChildEntry>>>,
}

impl Default for ServerRegistry {
    fn default() -> Self { Self { inner: tokio::sync::Mutex::new(HashMap::new()) } }
}

impl ServerRegistry {
    pub async fn insert(&self, id: String, entry: ChildEntry) -> Arc<ChildEntry> {
        let arc = Arc::new(entry);
        self.inner.lock().await.insert(id, arc.clone());
        arc
    }
    pub async fn remove(&self, id: &str) -> Option<Arc<ChildEntry>> {
        self.inner.lock().await.remove(id)
    }
    pub async fn get(&self, id: &str) -> Option<Arc<ChildEntry>> {
        self.inner.lock().await.get(id).cloned()
    }
    pub async fn list_ids(&self) -> Vec<String> {
        self.inner.lock().await.keys().cloned().collect()
    }
    pub async fn list(&self) -> Vec<Arc<ChildEntry>> {
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
///
/// 容量治理:
/// - **MAX_SESSIONS** 上限 —— 满后 `insert` 返回 `Err`,调用方 503 + 关流。
/// - **每 session mpsc channel(64)** —— buffer 不深,backpressure 让客户端自然受限。
/// - **last_seen_at** —— send 时更新;后台 cleanup task 每 60s 清超过
///   `SESSION_IDLE_TIMEOUT` (5min) 没活动的 session。
type SessionTx = tokio::sync::mpsc::Sender<String>;

pub struct SessionRegistry {
    inner: tokio::sync::Mutex<std::collections::HashMap<String, SessionEntry>>,
}

struct SessionEntry {
    tx: SessionTx,
    last_seen_at: std::sync::Mutex<Instant>,
}

impl Default for SessionRegistry {
    fn default() -> Self { Self { inner: tokio::sync::Mutex::new(std::collections::HashMap::new()) } }
}

impl SessionRegistry {
    /// 插入新 session。map 已满返回 `Err(id)`,调用方 503 + 拒绝。
    pub async fn insert(&self, id: String, tx: SessionTx) -> Result<(), String> {
        let mut guard = self.inner.lock().await;
        if guard.len() >= MAX_SESSIONS {
            return Err(id);
        }
        guard.insert(id, SessionEntry { tx, last_seen_at: std::sync::Mutex::new(Instant::now()) });
        Ok(())
    }
    pub async fn remove(&self, id: &str) -> Option<SessionTx> {
        self.inner.lock().await.remove(id).map(|e| e.tx)
    }
    pub async fn send(&self, id: &str, msg: String) -> bool {
        let guard = self.inner.lock().await;
        if let Some(entry) = guard.get(id) {
            // 先更新时间戳 —— 即使 send 失败也记,防止对端已经断开但我们还在 retry 续命。
            // 失败路径表示 channel 已满(client 不消费),这种情况视作"还在通信"
            // (只是客户端背压),所以仍然更新 last_seen_at。
            if let Ok(mut t) = entry.last_seen_at.lock() {
                *t = Instant::now();
            }
            return entry.tx.try_send(msg).is_ok();
        }
        false
    }
    pub async fn has(&self, id: &str) -> bool {
        self.inner.lock().await.contains_key(id)
    }
    /// 后台 cleanup 用:拿当前所有 session 的 (id, last_seen) 快照。
    /// 调用方根据 time_since 比 SESSION_IDLE_TIMEOUT 决定要不要 remove。
    async fn snapshot(&self) -> Vec<(String, Instant)> {
        self.inner
            .lock()
            .await
            .iter()
            .filter_map(|(id, e)| e.last_seen_at.lock().ok().map(|t| (id.clone(), *t)))
            .collect()
    }
    /// 后台 cleanup 用:删除指定 id 的 session(同时关 tx 让连接 task 退出)。
    async fn reap(&self, id: &str) -> bool {
        self.inner.lock().await.remove(id).is_some()
    }
}

static SESSIONS: once_cell::sync::Lazy<Arc<SessionRegistry>> =
    once_cell::sync::Lazy::new(|| Arc::new(SessionRegistry::default()));

pub fn sessions() -> Arc<SessionRegistry> { SESSIONS.clone() }

/// 启动 SSE session 的后台清理 task:每 60s 扫描一次,清掉 5min 没活动的 session。
/// 在第一次 `sessions()` 调用时 lazy 启动一次(由 `serve_sse_session` 触发即可)。
pub fn spawn_session_cleanup() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SESSION_CLEANUP_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let reg = sessions();
                let stale: Vec<String> = reg
                    .snapshot()
                    .await
                    .into_iter()
                    .filter_map(|(id, last)| {
                        if last.elapsed() >= SESSION_IDLE_TIMEOUT { Some(id) } else { None }
                    })
                    .collect();
                for id in stale {
                    if reg.reap(&id).await {
                        eprintln!("[ms-manage] SSE session stale ({} min idle), closed", SESSION_IDLE_TIMEOUT.as_secs() / 60);
                    }
                }
            }
        });
    });
}

/// 启动一个 stdio 子 MCP server 进程
pub async fn spawn_child(srv: &StoredServer) -> anyhow::Result<Arc<ChildEntry>> {
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
        log_event("error", &format!("server:{}", srv.name), format!("启动失败：{friendly}"), None);
        if let Some(st) = crate::state::try_current() {
            st.storage.live_states.write().insert(srv.id.clone(), ServerStatus::Error.as_str().to_string());
        }
        anyhow::anyhow!("{}", friendly)
    })?;

    let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let stderr = child.stderr.take();
    // 立即接管 stderr 到后台 drain，否则大量 banner/日志会撑满 pipe 阻塞子进程。
    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();
    let drain_handle = stderr.map(|se| drain_stderr(srv.name.clone(), se, kill_rx));

    let meta = ChildMeta {
        id: srv.id.clone(),
        name: srv.name.clone(),
        tools: Vec::new(),
        next_id: 0,
        started_at: chrono::Utc::now(),
    };
    let io = ChildIo {
        stdin,
        stdout: BufReader::new(stdout),
        stderr: None, // 已被 drain_stderr 接管
        pending: std::collections::VecDeque::new(),
        drain_handle,
        kill_tx: Some(kill_tx),
    };
    let entry = ChildEntry {
        meta: parking_lot::RwLock::new(meta),
        io: tokio::sync::Mutex::new(io),
        child: tokio::sync::Mutex::new(child),
    };

    // initialize handshake
    let init_id_num = entry.meta.write().next_id();
    let init_req = serde_json::json!({
        "jsonrpc": "2.0", "id": init_id_num, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ms-manage-proxy", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    {
        let mut io = entry.io.lock().await;
        io.stdin.write_all(format!("{}\n", init_req).as_bytes()).await?;
        io.stdin.flush().await?;
    }
    let init_id = serde_json::json!(init_id_num);
    let read = {
        let mut io = entry.io.lock().await;
        read_response_from_child(&mut io, std::time::Duration::from_secs(15)).await
    };
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
    {
        let mut io = entry.io.lock().await;
        io.stdin.write_all(format!("{}\n", notif).as_bytes()).await?;
        io.stdin.flush().await?;
    }

    // 拉取 tools/list 缓存
    let list_id_num = entry.meta.write().next_id();
    let list_req = serde_json::json!({
        "jsonrpc": "2.0", "id": list_id_num, "method": "tools/list", "params": {}
    });
    {
        let mut io = entry.io.lock().await;
        io.stdin.write_all(format!("{}\n", list_req).as_bytes()).await?;
        io.stdin.flush().await?;
    }
    let list_id = serde_json::json!(list_id_num);
    let read = {
        let mut io = entry.io.lock().await;
        read_response_from_child(&mut io, std::time::Duration::from_secs(15)).await
    };
    if let Ok(Some((id, v))) = read {
        if id == list_id {
            if let Some(tools) = v.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                entry.meta.write().tools = tools.clone();
            }
        }
    }

    let id = srv.id.clone();
    let tool_count = entry.meta.read().tools.len();
    let arc = registry().insert(id, entry).await;
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
///
/// 严格按顺序释放锁,避免持锁链:
/// 1. `entry.child.lock().await` —— `.kill().await` 异步,持锁到子进程退出
/// 2. `entry.io.lock().await` —— drain_handle.await 后 kill_tx.send(())
/// 3. `entry.meta.read()` —— 读 name
pub async fn stop_all() {
    let ids = registry().list_ids().await;
    for id in ids {
        if let Some(entry) = registry().remove(&id).await {
            kill_and_drain(&entry).await;
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
    if let Some(entry) = registry().remove(id).await {
        let name = entry.meta.read().name.clone();
        kill_and_drain(&entry).await;
        log_event("info", "server", format!("server「{name}」已停止"), None);
    }
    if let Some(st) = crate::state::try_current() {
        st.storage.live_states.write().insert(id.to_string(), ServerStatus::Stopped.as_str().to_string());
    }
}

/// 关闭一个 ChildEntry 的子进程 + 回收 IO 资源。
/// 抽取出来避免 stop_all / stop 两处重复。
async fn kill_and_drain(entry: &Arc<ChildEntry>) {
    // 1. kill 子进程(等到 OS 真正回收),子进程关闭后 stderr 的 pipe 自然 EOF,
    //    drain_stderr 任务读到 Ok(None) 后退出 — 但我们同时主动 signal 让它更快退出。
    {
        let mut c = entry.child.lock().await;
        let _ = c.kill().await;
    }
    // 2. 通知 drain 任务 + join 它的 handle;kill_tx 失败说明接收端已退出(正常),
    //    drain_handle 没东西可 join 时直接跳过。kill_tx 只能 take 一次,所以 Option。
    let mut io = entry.io.lock().await;
    if let Some(tx) = io.kill_tx.take() {
        let _ = tx.send(());
    }
    if let Some(h) = io.drain_handle.take() {
        let _ = h.await;
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
        // fetch_tools 用完即弃,无需 kill 信号 — 返回一个会让 kill_rx 永远等不到的
        // receiver, drain_stderr 任务在 EOF 时自然退出,但 kill_rx 那个分支永远不会被 poll。
        let (_kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        drain_stderr(s.name.clone(), se, kill_rx);
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
        // test() 用完即弃,无需 kill 信号(drain 在 EOF 时自然退出)。
        let (_kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        drain_stderr(s.name.clone(), se, kill_rx);
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
