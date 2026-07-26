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

use crate::install::probe::{self, PkgManager, ProbeResult};
use crate::state::AppState;
use crate::storage::StoredServer;
use chrono::Utc;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

/// 安装进度事件 payload，前端通过 `install_progress` 事件订阅
#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub id: String,
    pub stage: String, // "checking" | "installing" | "resolving" | "done" | "error"
    pub message: String,
}

/// 决定一条 server 配置是否需要预安装。需要的话返回包管理器 + 包名
pub fn detect_pkg(server: &StoredServer) -> Option<(PkgManager, String)> {
    if server.transport != "stdio" {
        return None;
    }
    let cmd = server.command.as_deref()?;
    probe::detect_pkg(cmd, &server.args)
}

fn now_iso() -> String { Utc::now().to_rfc3339() }

/// 公共 helper：校验并 schedule 一条 server 的后台安装任务。
/// 被 Tauri command 和 market 安装流程共用。
pub async fn schedule_install(
    app: &AppHandle,
    state: &State<'_, Arc<AppState>>,
    id: &str,
) -> Result<(), String> {
    let list = state.storage.load_servers().map_err(|e| e.to_string())?;
    let s = list
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "not found".to_string())?;
    let app2 = app.clone();
    tokio::spawn(async move {
        run_install(&app2, s).await;
    });
    Ok(())
}

/// 前端调：触发安装。立即返回，真实工作在后台 tokio task 里，
/// 进度通过 `install_progress` 事件推送。
#[tauri::command]
pub async fn install_server(
    id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    schedule_install(&app, &state, &id).await
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
            true
        });
        emit_progress(&app, &id, "done", "无需卸载（直接二进制）".to_string());
        return Ok(());
    };
    emit_progress(&app, &id, "installing", format!("正在卸载 {}…", pkg));
    // 卸载按包名（不带版本/标签）进行
    let name = probe::strip_version(&pkg);
    let cmd_line = match mgr {
        PkgManager::Npm => vec!["npm".to_string(), "uninstall".to_string(), "-g".to_string(), name.to_string()],
        PkgManager::Uv => vec!["uv".to_string(), "tool".to_string(), "uninstall".to_string(), name.to_string()],
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
                true
            });
            emit_progress(&app, &id, "done", format!("已卸载 {}", pkg));
        }
        Err(e) => {
            update_install_state(&state, &id, |f| {
                f.install_error = Some(format!("卸载失败：{}", e));
                true
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
        Ok(Some(ProbeResult { version, path })) => StoredServer {
            installed: true,
            installed_version: Some(version),
            installed_path: Some(path),
            installed_at: Some(now_iso()),
            install_error: None,
            ..s.clone()
        },
        Ok(None) => StoredServer {
            // 「未检测到本地安装」是中性状态而非错误，install_error 置空，前端显示「未安装」。
            installed: false,
            installed_version: None,
            installed_path: None,
            installed_at: None,
            install_error: None,
            ..s.clone()
        },
        Err(e) => StoredServer {
            // 探测出错时清空安装状态（避免残留「已装 v?」），仅记录错误供排查。
            installed: false,
            installed_version: None,
            installed_path: None,
            installed_at: None,
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
            true
        }).await;
        emit_progress(app, &id, "done", format!("{} 不需要预安装（直接二进制）", name));
        return;
    };

    // 入口校验包名，防止用户配置含 shell 元字符的包名触发命令注入。
    if !probe::validate_pkg_name(&pkg) {
        update_install_state_from(app, &s, |f| {
            f.installed = false;
            f.installed_version = None;
            f.installed_path = None;
            f.installed_at = None;
            f.install_error = Some(format!("非法包名：{}", pkg));
            true
        }).await;
        emit_progress(app, &id, "error", format!("非法包名：{}", pkg));
        return;
    }

    emit_progress(app, &id, "checking", format!("检查包 {} 是否已安装…", pkg));

    let already = match mgr {
        PkgManager::Npm => probe_npm_installed(&pkg).await.ok().flatten(),
        PkgManager::Uv => probe_uv_installed(&pkg).await.ok().flatten(),
    };
    if let Some(ProbeResult { version, path }) = already {
        update_install_state_from(app, &s, |f| {
            f.installed = true;
            f.installed_version = Some(version.clone());
            f.installed_path = Some(path);
            f.installed_at = Some(now_iso());
            f.install_error = None;
            true
        }).await;
        emit_progress(app, &id, "done", format!("{} 已安装 (v{})，跳过安装", name, version));
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
            f.installed = false;
            f.installed_version = None;
            f.installed_path = None;
            f.installed_at = None;
            f.install_error = Some(format!("安装失败：{}", e));
            true
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
        Ok(Some(ProbeResult { version, path })) => {
            update_install_state_from(app, &s, |f| {
                f.installed = true;
                f.installed_version = Some(version.clone());
                f.installed_path = Some(path);
                f.installed_at = Some(now_iso());
                f.install_error = None;
            
                true
    }).await;
            emit_progress(app, &id, "done", format!("{} 安装成功 (v{})", name, version));
        }
        Ok(None) => {
            update_install_state_from(app, &s, |f| {
                f.installed = false;
                f.installed_version = None;
                f.installed_path = None;
                f.installed_at = None;
                f.install_error = Some("安装命令成功退出，但探测不到已安装的二进制".to_string());
            
                true
    }).await;
            emit_progress(app, &id, "error", "安装命令成功退出，但探测不到二进制".to_string());
        }
        Err(e) => {
            update_install_state_from(app, &s, |f| {
                f.installed = false;
                f.installed_version = None;
                f.installed_path = None;
                f.installed_at = None;
                f.install_error = Some(format!("解析失败：{}", e));
            
                true
    }).await;
            emit_progress(app, &id, "error", format!("解析失败：{}", e));
        }
    }
}

/// 流式跑一条命令，把 stdout/stderr 逐行打日志。窗口层日志会写到 MS Manage 日志里。
async fn run_streamed(prog: &str, args: &[ String]) -> anyhow::Result<()> {
    crate::process_util::run_streamed(prog, args).await
}

pub(crate) async fn probe_npm_installed(pkg: &str) -> anyhow::Result<Option<ProbeResult>> {
    // pkg 可能带版本/标签（如 `tavily-mcp@0.1.4`），探测匹配时用剥掉版本的包名。
    let name = probe::strip_version(pkg);
    // 注意：`npm ls -g <pkg>` 对「未安装」的包返回退出码 1（stdout 是无 dependencies 的 JSON）。
    // 所以不能用 run_capture（它把非零当错误），要先解析 stdout，再按退出码区分「未安装」与「真错误」。
    let out = crate::process_util::run_capture_output("npm", &["ls", "-g", name, "--json=true", "--depth=0"])
        .await
        .map_err(|e| anyhow::anyhow!("npm ls 失败：{e}"))?;
    let ver = probe::parse_npm_ls(&out.stdout, name);
    let ver = match ver {
        Some(v) => v,
        None => {
            // 解析不到版本：退出码 0 或 1 都视为「未安装」；其它非零才是真错误。
            let code = out.status.code().unwrap_or(0);
            if code == 0 || code == 1 {
                return Ok(None);
            }
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("npm ls 失败：npm 退出码 {:?}: {}", out.status.code(), stderr.trim());
        }
    };

    let prefix_out = crate::process_util::run_capture("npm", &["prefix", "-g"])
        .await
        .map_err(|e| anyhow::anyhow!("npm prefix 失败：{e}"))?;
    let prefix = probe::parse_npm_prefix(&prefix_out).unwrap_or_default();
    let path = if prefix.as_os_str().is_empty() {
        String::new()
    } else {
        probe::resolve_npm_bin(&prefix, name)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    };

    Ok(Some(ProbeResult { version: ver, path }))
}

pub(crate) async fn probe_uv_installed(pkg: &str) -> anyhow::Result<Option<ProbeResult>> {
    let out = crate::process_util::run_capture("uv", &["tool", "list", "--show-paths"])
        .await
        .map_err(|e| anyhow::anyhow!("uv tool list 失败：{e}"))?;
    Ok(probe::parse_uv_tool_list(&out, probe::strip_version(pkg)))
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

/// 在 AppState 上原子地修改一条 server 记录。
/// 走 `Storage::mutate_server` —— 读-改-写全程持写锁，
/// 跨后台任务与 GUI 并发安全（不会互相覆盖）。
/// 闭包返回 bool 表示是否真的改了 —— updated_at / 写盘只在 true 时发生。
fn mutate_server<F: FnOnce(&mut StoredServer) -> bool>(
    state: &AppState,
    id: &str,
    f: F,
) -> anyhow::Result<bool> {
    let (_, changed) = state.storage.mutate_server(id, f)?;
    Ok(changed)
}

fn update_install_state<F: FnOnce(&mut StoredServer) -> bool>(
    state: &State<'_, Arc<AppState>>,
    id: &str,
    f: F,
) {
    if let Err(e) = mutate_server(state.inner(), id, f) {
        crate::proxy::log_event("error", "install", format!("update_install_state({id}) 写盘失败：{e}"), None);
    }
}

async fn update_install_state_from<F: FnOnce(&mut StoredServer) -> bool>(
    _app: &AppHandle,
    s: &StoredServer,
    f: F,
) {
    let Some(st) = crate::state::try_current() else { return; };
    if let Err(e) = mutate_server(&st, &s.id, f) {
        crate::proxy::log_event("error", "install", format!("update_install_state_from({}) 写盘失败：{e}", s.id), None);
    }
}

fn write_back(state: &State<'_, Arc<AppState>>, id: &str, updated: &StoredServer) {
    let st = state.inner().clone();
    let updated = updated.clone();
    if let Err(e) = mutate_server(&st, id, |slot| {
        // 保留并发任务可能刚更新的安装字段之外的全部字段
        let keep_updated_at = slot.updated_at.clone();
        *slot = updated.clone();
        slot.updated_at = keep_updated_at;
        true
    }) {
        crate::proxy::log_event("error", "install", format!("write_back({id}) 写盘失败：{e}"), None);
    }
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