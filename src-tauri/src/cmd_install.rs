//! 预安装 / 卸载命令
//!
//! 设计目标：用户写好一条 server 配置（如 `npx -y @foo/bar`）后，需要先点"安装"
//! 把包拉到本地（`npm i -g` / `uv tool install`），启动时才能跑。
//! 安装成功后把版本号 + 二进制绝对路径写回 servers.json，启动时直接用这个路径。
//!
//! 需要安装的 server 类型：
//!   - `npx / npm / pnpm / yarn / bunx / pnpx <pkg>`  → `npm install -g <pkg>`
//!   - `uvx / uv tool run <pkg>`                     → `uv tool install <pkg>`
//! 其它情况（直接填绝对路径的 .exe）不需要安装，直接标 installed=true。

use crate::state::AppState;
use crate::storage::StoredServer;
use chrono::Utc;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// 安装进度事件 payload，前端通过 `install_progress` 事件订阅
#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub id: String,
    pub stage: String, // "checking" | "installing" | "resolving" | "done" | "error"
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PkgManager {
    Npm,
    Uv,
}

/// 决定一条 server 配置是否需要预安装。需要的话返回包管理器 + 包名
pub fn detect_pkg(server: &StoredServer) -> Option<(PkgManager, String)> {
    if server.transport != "stdio" {
        return None;
    }
    let cmd = server.command.as_deref()?.to_ascii_lowercase();
    // 跳过像 --yes / -y / -p 这种 flag，取第一个非 flag 参数作为包名
    let pkg = server
        .args
        .iter()
        .find(|a| !a.starts_with('-'))?
        .clone();
    // 去掉末尾的 @version（npx -y @scope/pkg@1.0.0 → @scope/pkg）
    let pkg = strip_version(&pkg);
    match cmd.as_str() {
        "npx" | "npm" | "pnpm" | "pnpx" | "yarn" | "bun" | "bunx" => {
            Some((PkgManager::Npm, pkg))
        }
        "uvx" | "uv" => Some((PkgManager::Uv, pkg)),
        _ => None,
    }
}

/// `@scope/foo@1.0.0` → `@scope/foo`，`foo@latest` → `foo`
fn strip_version(pkg: &str) -> String {
    if pkg.starts_with('@') {
        if let Some(idx) = pkg[1..].find('@') {
            return pkg[..=idx].to_string();
        }
        return pkg.to_string();
    }
    pkg.split('@').next().unwrap_or(pkg).to_string()
}

/// 包名 → 候选可执行文件名（去掉 scope，取最后一段）
fn pkg_basename(pkg: &str) -> String {
    let no_scope = pkg.strip_prefix('@').unwrap_or(pkg);
    let name = no_scope.split('/').last().unwrap_or(no_scope);
    name.to_string()
}

fn now_iso() -> String { Utc::now().to_rfc3339() }

/// 前端调：触发安装。立即返回，真实工作在后台 tokio task 里，
/// 进度通过 `install_progress` 事件推送。
#[tauri::command]
pub async fn install_server(
    id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    let app2 = app.clone();
    tokio::spawn(async move {
        run_install(&app2, s).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn uninstall_server(
    id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.iter().find(|s| s.id == id).cloned().ok_or_else(|| "not found".to_string())?;
    let Some((mgr, pkg)) = detect_pkg(&s) else {
        update_install_state(&state, &id, |f| {
            f.installed = false;
            f.installed_version = None;
            f.installed_path = None;
            f.installed_at = None;
            f.install_error = None;
        });
        emit_progress(&app, &id, "done", "无需卸载（直接二进制）".to_string());
        return Ok(());
    };
    emit_progress(&app, &id, "installing", format!("正在卸载 {}…", pkg));
    let cmd_line = match mgr {
        PkgManager::Npm => vec!["npm".to_string(), "uninstall".to_string(), "-g".to_string(), pkg.clone()],
        PkgManager::Uv => vec!["uv".to_string(), "tool".to_string(), "uninstall".to_string(), pkg.clone()],
    };
    let res = run_streamed(&cmd_line[0], &cmd_line[1..]).await;
    match res {
        Ok(_) => {
            update_install_state(&state, &id, |f| {
                f.installed = false;
                f.installed_version = None;
                f.installed_path = None;
                f.installed_at = None;
                f.install_error = None;
            });
            emit_progress(&app, &id, "done", format!("已卸载 {}", pkg));
        }
        Err(e) => {
            update_install_state(&state, &id, |f| {
                f.install_error = Some(format!("卸载失败：{}", e));
            });
            emit_progress(&app, &id, "error", format!("卸载失败：{}", e));
        }
    }
    Ok(())
}

/// 重新探测已安装状态（点 "刷新" 按钮用，比如手动 npm i -g 之后想同步进来）
#[tauri::command]
pub async fn refresh_install_status(
    id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    let Some((mgr, pkg)) = detect_pkg(&s) else {
        let updated = StoredServer {
            installed: true,
            installed_version: None,
            installed_path: s.command.clone(),
            installed_at: Some(now_iso()),
            install_error: None,
            ..s.clone()
        };
        write_back(&state, &id, &updated);
        return Ok(serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null));
    };
    emit_progress(&app, &id, "checking", format!("正在探测 {}…", pkg));
    let probe = match mgr {
        PkgManager::Npm => probe_npm_installed(&pkg).await,
        PkgManager::Uv => probe_uv_installed(&pkg).await,
    };
    let updated = match probe {
        Ok(Some((ver, path))) => StoredServer {
            installed: true,
            installed_version: Some(ver),
            installed_path: Some(path),
            installed_at: Some(now_iso()),
            install_error: None,
            ..s.clone()
        },
        Ok(None) => StoredServer {
            installed: false,
            installed_version: None,
            installed_path: None,
            installed_at: None,
            install_error: Some("未检测到本地安装".to_string()),
            ..s.clone()
        },
        Err(e) => StoredServer {
            installed: false,
            install_error: Some(format!("探测失败：{}", e)),
            ..s.clone()
        },
    };
    write_back(&state, &id, &updated);
    emit_progress(&app, &id, "done", "状态已刷新".to_string());
    Ok(serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null))
}

// ---------- 内部：实际安装流程 ----------

async fn run_install(app: &AppHandle, s: StoredServer) {
    let id = s.id.clone();
    let name = s.name.clone();
    let Some((mgr, pkg)) = detect_pkg(&s) else {
        update_install_state_from(app, &s, |f| {
            f.installed = true;
            f.installed_path = s.command.clone();
            f.installed_at = Some(now_iso());
            f.install_error = None;
        }).await;
        emit_progress(app, &id, "done", format!("{} 不需要预安装（直接二进制）", name));
        return;
    };

    emit_progress(app, &id, "checking", format!("检查包 {} 是否已安装…", pkg));

    let already = match mgr {
        PkgManager::Npm => probe_npm_installed(&pkg).await.ok().flatten(),
        PkgManager::Uv => probe_uv_installed(&pkg).await.ok().flatten(),
    };
    if let Some((ver, path)) = already {
        update_install_state_from(app, &s, |f| {
            f.installed = true;
            f.installed_version = Some(ver.clone());
            f.installed_path = Some(path);
            f.installed_at = Some(now_iso());
            f.install_error = None;
        }).await;
        emit_progress(app, &id, "done", format!("{} 已安装 (v{})，跳过安装", name, ver));
        return;
    }

    emit_progress(app, &id, "installing", format!("正在安装 {} …", pkg));
    let cmd_line = match mgr {
        PkgManager::Npm => vec!["npm".to_string(), "install".to_string(), "-g".to_string(), pkg.clone()],
        PkgManager::Uv => vec!["uv".to_string(), "tool".to_string(), "install".to_string(), pkg.clone()],
    };
    let res = run_streamed(&cmd_line[0], &cmd_line[1..]).await;
    if let Err(e) = res {
        update_install_state_from(app, &s, |f| {
            f.install_error = Some(format!("安装失败：{}", e));
        }).await;
        emit_progress(app, &id, "error", format!("安装失败：{}", e));
        return;
    }

    emit_progress(app, &id, "resolving", "解析安装结果…".to_string());
    let probed = match mgr {
        PkgManager::Npm => probe_npm_installed(&pkg).await,
        PkgManager::Uv => probe_uv_installed(&pkg).await,
    };
    match probed {
        Ok(Some((ver, path))) => {
            update_install_state_from(app, &s, |f| {
                f.installed = true;
                f.installed_version = Some(ver.clone());
                f.installed_path = Some(path);
                f.installed_at = Some(now_iso());
                f.install_error = None;
            }).await;
            emit_progress(app, &id, "done", format!("{} 安装成功 (v{})", name, ver));
        }
        Ok(None) => {
            update_install_state_from(app, &s, |f| {
                f.install_error = Some("安装命令成功退出，但探测不到已安装的二进制".to_string());
            }).await;
            emit_progress(app, &id, "error", "安装命令成功退出，但探测不到二进制".to_string());
        }
        Err(e) => {
            update_install_state_from(app, &s, |f| {
                f.install_error = Some(format!("解析失败：{}", e));
            }).await;
            emit_progress(app, &id, "error", format!("解析失败：{}", e));
        }
    }
}

/// 流式跑一条命令，把 stdout/stderr 逐行打日志。窗口层日志会写到 MS Manage 日志里。
/// Windows 上 `.cmd/.bat` 必须用 `cmd.exe /c` 包装（tokio 不走 PATHEXT）。
async fn run_streamed(prog: &str, args: &[String]) -> anyhow::Result<()> {
    let resolved = crate::process_util::resolve_cmd(prog);
    let mut c = if cfg!(windows) && resolved.extension()
        .map(|e| e.to_ascii_lowercase())
        .map_or(false, |e| e == "cmd" || e == "bat" || e == "ps1")
    {
        let mut c = Command::new("cmd.exe");
        c.arg("/c").arg(&resolved);
        c.args(args);
        c
    } else {
        let mut c = Command::new(&resolved);
        c.args(args);
        c
    };
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        c.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = c.spawn().map_err(|e| anyhow::anyhow!("spawn {prog} 失败：{e}"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let stderr = child.stderr.take();
    let mut out_reader = BufReader::new(stdout).lines();
    let mut err_reader = if let Some(e) = stderr { Some(BufReader::new(e).lines()) } else { None };
    loop {
        let mut progressed = false;
        if let Some(line) = out_reader.next_line().await.map_err(|e| anyhow::anyhow!("stdout: {e}"))? {
            if !line.is_empty() {
                eprintln!("[install] {prog}: {line}");
            }
            progressed = true;
        }
        if let Some(ref mut r) = err_reader {
            if let Some(line) = r.next_line().await.map_err(|e| anyhow::anyhow!("stderr: {e}"))? {
                if !line.is_empty() {
                    eprintln!("[install] {prog}!: {line}");
                }
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    let status = child.wait().await.map_err(|e| anyhow::anyhow!("wait: {e}"))?;
    if !status.success() {
        anyhow::bail!("{} 退出码 {:?}", prog, status.code());
    }
    Ok(())
}

async fn probe_npm_installed(pkg: &str) -> anyhow::Result<Option<(String, String)>> {
    // 1. 拿版本
    let out = run_capture("npm", &["ls".into(), "-g".into(), pkg.into(), "--json=true".into(), "--depth=0".into()]).await
        .map_err(|e| anyhow::anyhow!("npm ls 失败：{e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap_or(serde_json::Value::Null);
    let ver = v.get("dependencies")
        .and_then(|d| d.get(pkg))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let Some(ver) = ver else { return Ok(None); };

    // 2. 拿全局 prefix（npm prefix -g）
    let prefix_out = run_capture("npm", &["prefix".into(), "-g".into()]).await
        .map_err(|e| anyhow::anyhow!("npm prefix 失败：{e}"))?;
    let prefix = String::from_utf8_lossy(&prefix_out).trim().to_string();
    if prefix.is_empty() {
        return Ok(Some((ver, String::new())));
    }

    // 3. 读包 package.json 的 bin 字段拿真实入口
    let pkg_json_path = std::path::Path::new(&prefix).join("node_modules").join(pkg).join("package.json");
    let bin_path = std::fs::read_to_string(&pkg_json_path).ok()
        .and_then(|j| {
            let v: serde_json::Value = serde_json::from_str(&j).ok()?;
            v.get("bin").and_then(|b| match b {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(o) => o.values().next().and_then(|x| x.as_str().map(String::from)),
                _ => None,
            })
        })
        .map(|rel| {
            let p = std::path::Path::new(&rel);
            if p.is_absolute() {
                rel
            } else {
                std::path::Path::new(&prefix).join("node_modules").join(pkg).join(&rel).to_string_lossy().to_string()
            }
        });

    // 4. fallback：扫 <prefix>/node_modules/.bin/ 找匹配 basename
    let path = if let Some(p) = bin_path {
        p
    } else {
        let bin_name = pkg_basename(pkg);
        let mut candidates = Vec::new();
        let bin_dir = std::path::Path::new(&prefix).join("node_modules").join(".bin");
        for ext in ["", ".cmd", ".bat", ".ps1", ".exe"] {
            let cand = bin_dir.join(format!("{bin_name}{ext}"));
            if cand.is_file() {
                candidates.push(cand.to_string_lossy().to_string());
            }
        }
        candidates.into_iter().next().unwrap_or_default()
    };
    Ok(Some((ver, path)))
}

async fn probe_uv_installed(pkg: &str) -> anyhow::Result<Option<(String, String)>> {
    let out = run_capture("uv", &["tool".into(), "list".into(), "--show-paths".into()]).await
        .map_err(|e| anyhow::anyhow!("uv tool list 失败：{e}"))?;
    let text = String::from_utf8_lossy(&out);
    // 输出格式：
    //   <pkg> v<ver> (<env-dir>)
    //   - <exe-name> (<abs-path>)
    let basename = pkg_basename(pkg);
    let mut pkg_ver: Option<String> = None;
    let mut exe_paths: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(rest) = line.strip_prefix('-') {
            let rest = rest.trim();
            if let Some(idx) = rest.rfind(" (") {
                let name = rest[..idx].trim();
                let path = rest[idx+2..rest.len()-1].trim();
                if name.eq_ignore_ascii_case(&basename) {
                    exe_paths.push(path.to_string());
                }
            }
        } else if line.to_ascii_lowercase().starts_with(&pkg.to_ascii_lowercase()) {
            if let Some(v_idx) = line.find(" v") {
                let rest = &line[v_idx+2..];
                let ver = rest.split_whitespace().next().unwrap_or("").trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
                if !ver.is_empty() {
                    pkg_ver = Some(ver.to_string());
                }
            }
        }
    }
    if pkg_ver.is_none() && exe_paths.is_empty() {
        return Ok(None);
    }
    let path = exe_paths.into_iter().next().unwrap_or_default();
    Ok(Some((pkg_ver.unwrap_or_else(|| "?".to_string()), path)))
}

/// `where <bin>` (Windows) / `which <bin>` (Unix)
async fn which(bin: &str) -> Option<String> {
    let (prog, arg) = if cfg!(windows) { ("where", bin) } else { ("which", bin) };
    let out = run_capture(prog, &[arg.to_string()]).await.ok()?;
    let s = String::from_utf8_lossy(&out);
    s.lines().next().map(|l| l.trim().to_string()).filter(|s| !s.is_empty())
}

/// Windows 上 npm/uv 都是 .cmd，tokio Command 不走 PATHEXT。
/// 统一走这个 helper：resolve + cmd.exe /c 包装。
async fn run_capture(prog: &str, args: &[String]) -> anyhow::Result<Vec<u8>> {
    let resolved = crate::process_util::resolve_cmd(prog);
    let mut c = if cfg!(windows) && resolved.extension()
        .map(|e| e.to_ascii_lowercase())
        .map_or(false, |e| e == "cmd" || e == "bat" || e == "ps1")
    {
        let mut c = Command::new("cmd.exe");
        c.arg("/c").arg(&resolved);
        c.args(args);
        c
    } else {
        let mut c = Command::new(&resolved);
        c.args(args);
        c
    };
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    { c.creation_flags(0x08000000); }
    let out = c.output().await.map_err(|e| anyhow::anyhow!("spawn {prog} 失败：{e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("{} 退出码 {:?}: {}", prog, out.status.code(), stderr.trim());
    }
    Ok(out.stdout)
}

fn emit_progress(app: &AppHandle, id: &str, stage: &str, message: String) {
    let _ = app.emit(
        "install_progress",
        InstallProgress {
            id: id.to_string(),
            stage: stage.to_string(),
            message,
        },
    );
}

fn update_install_state<F: FnOnce(&mut StoredServer)>(
    state: &State<'_, Arc<AppState>>,
    id: &str,
    f: F,
) {
    let Ok(mut list) = state.storage.load_servers() else { return; };
    let Some(s) = list.iter_mut().find(|s| s.id == id) else { return; };
    f(s);
    s.updated_at = now_iso();
    let _ = state.storage.save_servers(&list);
}

async fn update_install_state_from<F: FnOnce(&mut StoredServer)>(
    app: &AppHandle,
    s: &StoredServer,
    f: F,
) {
    let Some(st) = crate::state::try_current() else { return; };
    let Ok(mut list) = st.storage.load_servers() else { return; };
    let Some(slot) = list.iter_mut().find(|x| x.id == s.id) else { return; };
    f(slot);
    slot.updated_at = now_iso();
    let _ = st.storage.save_servers(&list);
    let _ = app; // 保留 hook 方便后续加事件
}

fn write_back(state: &State<'_, Arc<AppState>>, id: &str, updated: &StoredServer) {
    let Ok(mut list) = state.storage.load_servers() else { return; };
    if let Some(slot) = list.iter_mut().find(|s| s.id == id) {
        *slot = updated.clone();
        slot.updated_at = now_iso();
    }
    let _ = state.storage.save_servers(&list);
}

/// 启动时优先用 installed_path 解析命令。
/// 如果 server 标记 installed 但 installed_path 仍指向原 command（如直接二进制），原样返回。
pub fn resolve_installed_command(server: &StoredServer) -> Option<(PathBuf, Vec<String>)> {
    if !server.installed { return None; }
    let cmd_str = server.installed_path.clone()
        .or_else(|| server.command.clone())?;
    if cmd_str.is_empty() { return None; }
    let cmd_path = PathBuf::from(&cmd_str);
    let (final_cmd, final_args) = crate::process_util::resolve_spawn_command(&cmd_path, &server.args);
    Some((final_cmd, final_args))
}