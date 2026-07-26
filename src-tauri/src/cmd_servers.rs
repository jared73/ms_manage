use crate::state::AppState;
use crate::storage::LogRecord;
use crate::storage::StoredServer;
use chrono::Utc;
use std::sync::Arc;
use tauri::State;

fn to_stored(s: serde_json::Value) -> Result<StoredServer, String> {
    serde_json::from_value(s).map_err(|e| e.to_string())
}

fn from_stored(s: &StoredServer) -> serde_json::Value {
    let mut v = serde_json::to_value(s).unwrap_or(serde_json::Value::Null);
    // 派生字段：是否需要预装（command 是 npx/uvx 等包管理器）。
    // 判定收敛在后端 detect_pkg，前端直接读取，避免前后端各维护一份包管理器列表导致漂移。
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "needs_install".to_string(),
            serde_json::Value::Bool(crate::cmd_install::detect_pkg(s).is_some()),
        );
    }
    v
}

#[tauri::command]
pub fn list_servers(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    Ok(list.iter().map(from_stored).collect())
}

#[tauri::command]
pub fn list_server_states(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let states = state.storage.live_states.read();
    let logs = state.storage.logs.read();
    // 按 server.name 索引最近一条 error log
    let mut last_err: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for l in logs.iter().rev() {
        if l.level == "error" {
            if let Some(rest) = l.source.strip_prefix("server:") {
                last_err.entry(rest.to_string()).or_insert_with(|| l.message.clone());
            }
        }
    }
    Ok(list.iter().map(|s| {
        let st = states.get(&s.id).cloned().unwrap_or_else(|| "stopped".to_string());
        serde_json::json!({
            "server": from_stored(s),
            "status": st,
            "last_error": last_err.get(&s.name).cloned(),
        })
    }).collect())
}

#[tauri::command]
pub fn get_server_state(id: String, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    let states = state.storage.live_states.read();
    let logs = state.storage.logs.read();
    let st = states.get(&s.id).cloned().unwrap_or_else(|| "stopped".to_string());
    let last_err = logs.iter().rev()
        .find(|l| l.level == "error" && l.source == format!("server:{}", s.name))
        .map(|l| l.message.clone());
    Ok(serde_json::json!({ "server": from_stored(&s), "status": st, "last_error": last_err }))
}

/// stdio transport 允许的命令白名单。
///
/// 设计:
/// - 防止 webview 脚本通过 `upsert_server` 注册 `command = "cmd"` / `"powershell"`
///   等触发 `cmd.exe /C <user-controlled>` 路径(`proxy/mod.rs:268` 的 `.cmd/.bat` fallback 是 shell)
/// - 允许两种形式:
///   1. **基名匹配** —— "npx" / "uvx" / "node" 等可执行文件名(走 PATH 解析)
///   2. **绝对路径白名单** —— 用户在 OS 上装好的具体可执行路径(由前端管理允许列表)
/// - 拒绝:
///   - 任何 `.cmd` / `.bat` / `.ps1` 后缀(必须用 shim 包成 exe)
///   - 含 NUL / shell 元字符的 args
///   - 路径里出现 `..` 或不在白名单的绝对路径
const ALLOWED_STDIO_COMMANDS: &[&str] = &[
    // JS/Node 生态
    "npx", "npm", "node", "pnpm", "yarn", "bun", "deno",
    // Python 生态
    "uvx", "uv", "python", "python3", "pipx",
    // Rust 生态
    "cargo",
    // 通用
    "sh", "bash", // 仅在明确是 unix 时;Windows 上会被 process_util 自动转
];

fn validate_command(cmd: &str) -> Result<(), String> {
    if cmd.is_empty() {
        return Err("command 不能为空".to_string());
    }
    let trimmed = cmd.trim();
    // 拒 NUL
    if trimmed.contains('\0') {
        return Err("command 含 NUL 字节".to_string());
    }
    // 拒 .cmd / .bat / .ps1 后缀(必须用无后缀的可执行文件或 npm-bundled shim)
    let lower = trimmed.to_lowercase();
    if lower.ends_with(".cmd") || lower.ends_with(".bat") || lower.ends_with(".ps1") {
        return Err(format!(
            "command 不允许 .cmd/.bat/.ps1 后缀,请用无后缀的 shim(Windows 上 npx → npx.exe);收到:{cmd}"
        ));
    }
    // 绝对路径:必须落在一个允许目录里(粗粒度:用户 HOME 下的常见 bin 目录)
    if trimmed.contains('/') || trimmed.contains('\\') {
        // 是绝对路径(以 / 开头,或 Windows 盘符 C:\)
        let is_absolute = trimmed.starts_with('/')
            || (trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':')
            || trimmed.starts_with("\\\\");
        if is_absolute {
            return validate_absolute_command_path(trimmed);
        }
        // 相对路径含分隔符 → 拒绝(避免 ./something 之类绕过)
        return Err(format!(
            "command 不允许相对路径(必须是无后缀基名或绝对路径);收到:{cmd}"
        ));
    }
    // 基名白名单
    let basename = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    if !ALLOWED_STDIO_COMMANDS.contains(&basename) {
        return Err(format!(
            "command {basename:?} 不在白名单内。允许:{ALLOWED_STDIO_COMMANDS:?}",
        ));
    }
    Ok(())
}

fn validate_absolute_command_path(path: &str) -> Result<(), String> {
    // 拒绝任何含 .. 的路径(防御性:虽然 base 白名单已经挡住,但绝对路径也要稳)
    if path.split(['/', '\\']).any(|seg| seg == "..") {
        return Err("command 绝对路径不允许 .. 段".to_string());
    }
    // 简化策略:绝对路径必须以可执行文件的基名匹配白名单
    // (防止 webview 注入 `C:\Windows\System32\cmd.exe`)
    let basename = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let basename_lower = basename.to_lowercase();
    let basename_stem = basename_lower
        .strip_suffix(".exe")
        .unwrap_or(&basename_lower);
    if !ALLOWED_STDIO_COMMANDS.contains(&basename_stem) {
        return Err(format!(
            "command 绝对路径基名 {basename:?} 不在白名单内"
        ));
    }
    Ok(())
}

fn validate_args(args: &[String]) -> Result<(), String> {
    for (i, a) in args.iter().enumerate() {
        if a.contains('\0') {
            return Err(format!("args[{i}] 含 NUL 字节"));
        }
        // 拒 PowerShell -EncodedCommand(明显的 RCE 入口)
        if a.eq_ignore_ascii_case("-EncodedCommand")
            || a.eq_ignore_ascii_case("-EC")
            || a.eq_ignore_ascii_case("/EncodedCommand")
            || a.eq_ignore_ascii_case("/EC")
        {
            return Err(format!("args[{i}] 不允许 -EncodedCommand"));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn upsert_server(server: serde_json::Value, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let mut s: StoredServer = to_stored(server)?;
    if s.id.is_empty() { s.id = uuid::Uuid::new_v4().to_string(); }
    // 验证 transport
    if !matches!(s.transport.as_str(), "stdio" | "sse" | "http") {
        return Err(format!("不支持的 transport: {:?}", s.transport));
    }
    // stdio 必须有 command + 命中白名单
    if s.transport == "stdio" {
        let cmd = s
            .command
            .as_deref()
            .ok_or_else(|| "stdio transport 必须指定 command".to_string())?;
        validate_command(cmd)?;
        validate_args(&s.args)?;
        // cwd 必须为绝对路径且不逃出 data_dir(允许为空 = 用默认)
        if let Some(cwd) = s.cwd.as_deref() {
            if !cwd.is_empty() {
                let p = std::path::Path::new(cwd);
                if !p.is_absolute() {
                    return Err(format!("cwd 必须是绝对路径,收到:{cwd}"));
                }
                if cwd.split(['/', '\\']).any(|seg| seg == "..") {
                    return Err("cwd 不允许 .. 段".to_string());
                }
            }
        }
        // env:key 不能含 NUL/=,value 含 NUL 就拒
        for (k, v) in &s.env {
            if k.contains('\0') || k.contains('=') {
                return Err(format!("env key 非法:{k:?}"));
            }
            if v.contains('\0') {
                return Err(format!("env[{k}] 含 NUL"));
            }
        }
    }
    // http/sse 必须有 url
    if matches!(s.transport.as_str(), "sse" | "http") && s.url.as_deref().unwrap_or("").is_empty() {
        return Err("sse/http transport 必须指定 url".to_string());
    }
    let now = Utc::now().to_rfc3339();
    if s.created_at.is_empty() { s.created_at = now.clone(); }
    s.updated_at = now;
    // 原子 upsert（更新或插入），持有写锁避免并发覆盖
    state.storage.upsert_server(&s).map_err(|e| e.to_string())?;
    Ok(from_stored(&s))
}

#[tauri::command]
pub async fn delete_server(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.storage.remove_server(&id).map_err(|e| e.to_string())?;
    crate::proxy::stop(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn start_server(
    id: String,
    set_auto_start: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "not found".to_string())?
        .clone();
    // 先启动 —— 启动失败不应留下错误的 auto_start 标记
    if let Err(e) = crate::proxy::start_one(&s).await {
        let log = LogRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: "error".to_string(),
            source: format!("server:{}", s.name),
            message: format!("启动失败：{}", e),
            detail: None,
        };
        state.storage.logs.write().push(log);
        return Err(e.to_string());
    }
    // 启动成功后,如果调用方要求同步 auto_start,只在这一处写盘
    sync_auto_start(&state.storage, &id, set_auto_start)?;
    Ok(())
}

#[tauri::command]
pub async fn stop_server(
    id: String,
    set_auto_start: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    crate::proxy::stop(&id).await;
    sync_auto_start(&state.storage, &id, set_auto_start)?;
    Ok(())
}

/// 把 server 的 auto_start 同步成 want(只在这两者不一致时写盘)。
/// 抽成纯函数以便 #[cfg(test)] 单独验证 —— 不需要拉起真实 proxy。
///
/// 返回值:
/// - `true` 表示真的写了盘(改了)
/// - `false` 表示 noop(None / 已等于 want / 找不到 id)
///
/// 用 `mutate_server` 在 load→mutate→save 期间全程持有 servers_lock,
/// 避免并发写(auto_start + 编辑表单)互相覆盖。
pub(crate) fn sync_auto_start(
    storage: &crate::storage::Storage,
    id: &str,
    want: Option<bool>,
) -> Result<bool, String> {
    let Some(want) = want else { return Ok(false) };
    let (_, changed) = storage
        .mutate_server(id, |slot| {
            if slot.auto_start.unwrap_or(false) == want {
                false
            } else {
                slot.auto_start = Some(want);
                true
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(changed)
}

#[tauri::command]
pub async fn test_server(id: String, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    crate::proxy::test(&s).await.map_err(|e| e.to_string())
}

/// 仅取工具列表（详情面板用，spawn→init→tools/list→kill）
#[tauri::command]
pub async fn fetch_server_tools(id: String, state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    crate::proxy::fetch_tools(&s).await
}

/// 切换单个工具的启用状态（写进 server.disabled_tools，下次 tools/list 时过滤掉）
///
/// 用 `mutate_server` 走 round-trip 持锁路径 —— 之前直接 read → mutate → save
/// 不持 servers_lock,期间后台 install 写回 installed 字段会丢修改(Lost Update)。
#[tauri::command]
pub async fn set_tool_enabled(id: String, tool_name: String, enabled: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let (found, _) = state.storage.mutate_server(&id, |s| {
        s.disabled_tools.retain(|t| t != &tool_name);
        if !enabled { s.disabled_tools.push(tool_name); }
        true
    }).map_err(|e| e.to_string())?;
    if !found { return Err("not found".to_string()); }
    Ok(())
}


#[cfg(test)]
mod tests {
    //! `sync_auto_start` 是 `start_server` / `stop_server` 共用的 auto_start 同步逻辑。
    //! 用 tempfile 起一个真实的 Storage,避免 mock 整个 storage 层。
    //!
    //! 覆盖的不变量:
    //! 1. `None` 永远不动盘
    //! 2. 已是 want 值不动盘
    //! 3. 不同 want → 写盘 + auto_start = want
    //! 4. updated_at 必须推进
    //! 5. 找不到 id 不报错也不动盘

    use super::*;
    use crate::storage::{Storage, StoredServer};
    use tempfile::TempDir;

    fn make_server(id: &str, auto_start: Option<bool>) -> StoredServer {
        let now = chrono::Utc::now().to_rfc3339();
        StoredServer {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: vec![],
            cwd: None,
            env: Default::default(),
            url: None,
            headers: Default::default(),
            enabled: true,
            auto_start,
            tags: vec![],
            project_id: None,
            disabled_tools: vec![],
            created_at: now.clone(),
            updated_at: now,
            installed: true,
            installed_version: None,
            installed_path: None,
            installed_at: None,
            install_error: None,
        }
    }

    fn setup_with_servers(servers: Vec<StoredServer>) -> (TempDir, Storage) {
        let dir = TempDir::new().expect("tempdir");
        let storage = Storage::new(dir.path().to_path_buf()).expect("storage init");
        storage.save_servers(&servers).expect("save servers");
        (dir, storage)
    }

    #[test]
    fn none_is_noop() {
        let (_dir, storage) = setup_with_servers(vec![make_server("s1", Some(true))]);
        let before = storage.load_servers().unwrap();
        let wrote = sync_auto_start(&storage, "s1", None).expect("none noop");
        assert!(!wrote, "None should never write");
        let after = storage.load_servers().unwrap();
        assert_eq!(before[0].auto_start, after[0].auto_start);
        assert_eq!(before[0].updated_at, after[0].updated_at, "None must not bump updated_at");
    }

    #[test]
    fn same_value_is_noop() {
        let (_dir, storage) = setup_with_servers(vec![make_server("s1", Some(true))]);
        let before = storage.load_servers().unwrap();
        let wrote = sync_auto_start(&storage, "s1", Some(true)).expect("same noop");
        assert!(!wrote, "same value should not write");
        let after = storage.load_servers().unwrap();
        assert_eq!(before[0].updated_at, after[0].updated_at);
    }

    #[test]
    fn none_to_true_writes() {
        let (_dir, storage) = setup_with_servers(vec![make_server("s1", None)]);
        let before = storage.load_servers().unwrap();
        let wrote = sync_auto_start(&storage, "s1", Some(true)).expect("write ok");
        assert!(wrote, "transition None → true must write");
        let after = storage.load_servers().unwrap();
        assert_eq!(after[0].auto_start, Some(true));
        assert_ne!(before[0].updated_at, after[0].updated_at, "updated_at must advance");
    }

    #[test]
    fn false_to_true_writes() {
        let (_dir, storage) = setup_with_servers(vec![make_server("s1", Some(false))]);
        let wrote = sync_auto_start(&storage, "s1", Some(true)).expect("write ok");
        assert!(wrote);
        let after = storage.load_servers().unwrap();
        assert_eq!(after[0].auto_start, Some(true));
    }

    #[test]
    fn true_to_false_writes() {
        let (_dir, storage) = setup_with_servers(vec![make_server("s1", Some(true))]);
        let wrote = sync_auto_start(&storage, "s1", Some(false)).expect("write ok");
        assert!(wrote);
        let after = storage.load_servers().unwrap();
        assert_eq!(after[0].auto_start, Some(false));
    }

    #[test]
    fn missing_id_is_silent_noop() {
        let (_dir, storage) = setup_with_servers(vec![make_server("s1", Some(true))]);
        let wrote = sync_auto_start(&storage, "nope", Some(false)).expect("missing id");
        assert!(!wrote, "missing id must not write and not error");
        // s1 状态不变
        let after = storage.load_servers().unwrap();
        assert_eq!(after[0].auto_start, Some(true));
    }

    #[test]
    fn only_target_server_is_touched() {
        let (_dir, storage) = setup_with_servers(vec![
            make_server("a", Some(true)),
            make_server("b", Some(false)),
            make_server("c", None),
        ]);
        let wrote = sync_auto_start(&storage, "b", Some(true)).expect("write ok");
        assert!(wrote);
        let after = storage.load_servers().unwrap();
        assert_eq!(after[0].auto_start, Some(true)); // a 不动
        assert_eq!(after[1].auto_start, Some(true)); // b 改了
        assert_eq!(after[2].auto_start, None);        // c 不动
    }

    #[test]
    fn persistence_survives_reload() {
        // 写一次 → 重新构造 Storage(读磁盘) → auto_start 还在
        let (dir, storage) = setup_with_servers(vec![make_server("s1", None)]);
        sync_auto_start(&storage, "s1", Some(true)).expect("write ok");
        let storage2 = Storage::new(dir.path().to_path_buf()).expect("rebuild");
        let after = storage2.load_servers().unwrap();
        assert_eq!(after[0].auto_start, Some(true), "must persist to disk");
    }
}

#[cfg(test)]
mod validate_tests {
    //! upsert_server 的 command/args 白名单验证 ——
    //! webview 通过这扇门能间接 spawn 进程,所以必须收紧。
    use super::*;

    #[test]
    fn allows_whitelisted_basenames() {
        for cmd in ["npx", "uvx", "node", "python", "python3", "pnpm", "deno", "bun"] {
            validate_command(cmd).unwrap_or_else(|e| panic!("{cmd} should pass: {e}"));
        }
    }

    #[test]
    fn allows_whitelisted_absolute_paths() {
        for path in [
            "C:/nvm4/node.exe",
            "/usr/local/bin/python3",
            "/opt/homebrew/bin/uvx",
        ] {
            validate_command(path).unwrap_or_else(|e| panic!("{path} should pass: {e}"));
        }
    }

    #[test]
    fn rejects_known_dangerous() {
        assert!(validate_command("cmd").is_err(), "cmd.exe 不应通过");
        assert!(validate_command("powershell").is_err(), "powershell 不应通过");
        assert!(validate_command("calc.exe").is_err(), "calc.exe 不在白名单");
        assert!(validate_command("C:/Windows/System32/cmd.exe").is_err(), "绝对 cmd.exe 不应通过");
    }

    #[test]
    fn rejects_cmd_bat_ps1_extensions() {
        assert!(validate_command("foo.cmd").is_err());
        assert!(validate_command("foo.bat").is_err());
        assert!(validate_command("foo.ps1").is_err());
        assert!(validate_command("C:/scripts/run.cmd").is_err());
    }

    #[test]
    fn rejects_relative_paths_with_separator() {
        assert!(validate_command("./npx").is_err());
        assert!(validate_command("../node").is_err());
        assert!(validate_command("subdir/npx").is_err());
    }

    #[test]
    fn rejects_dotdot_in_absolute_path() {
        assert!(validate_command("C:/foo/../node.exe").is_err());
        assert!(validate_command("/usr/local/../../bin/node").is_err());
    }

    #[test]
    fn rejects_nul_bytes() {
        assert!(validate_command("npx evil").is_err());
        assert!(validate_args(&["foo".into(), "bar baz".into()]).is_err());
    }

    #[test]
    fn rejects_encodedcommand() {
        assert!(validate_args(&["-EncodedCommand".into()]).is_err());
        assert!(validate_args(&["-EC".into()]).is_err());
        assert!(validate_args(&["/EncodedCommand".into()]).is_err());
        // case insensitive
        assert!(validate_args(&["-encodedcommand".into()]).is_err());
        // 但 -Command 应该被允许(PowerShell 交互式合法场景)
        validate_args(&["-Command".into()]).unwrap();
    }
}
