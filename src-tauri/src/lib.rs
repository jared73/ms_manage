// MS Manage — Tauri 2 backend library entry point
//
// 设计思路：把 Tauri 命令按模块拆分到 cmd_*.rs，
// 通过 mod 统一暴露给 generate_handler!。
// 业务核心（代理、server、skill 等）在子模块里。

pub mod autostart;
pub mod cli;
pub mod cmd_app;
pub mod cmd_doctor;
pub mod cmd_install;
pub mod cmd_logs;
pub mod cmd_market;
pub mod cmd_projects;
pub mod cmd_servers;
pub mod cmd_settings;
pub mod cmd_skills;
pub mod diagnose;
pub mod install;
pub(crate) mod process_util;
pub mod proxy;
pub mod proxy_endpoint;
pub mod single_instance;
pub mod state;
pub mod storage;
pub mod symlink;

use state::AppState;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let start_minimized = std::env::args().any(|arg| arg == "--minimized");
    let state = Arc::new(AppState::initialize().expect("failed to init app state"));
    state.clone().install_as_current();

    // 单实例守卫：GUI 启动时先把上次还活着的进程踢掉，再写自己的 PID。
    // 防止双开导致端口冲突、数据文件被两个进程同时改。
    if single_instance::kill_previous_instance(&state.data_dir) {
        eprintln!("[ms-manage] detected a previous instance running, killed it.");
    }
    single_instance::write_pid_file(&state.data_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(state.clone())
        .setup(move |app| {
            // 跟 mcp-router 一样：GUI 起来 = 代理起来，关 GUI = 代理死。
            // 不论设置里 auto_start_proxy 是不是开着，都起 —— 这就是本应用的核心功能。
            // 失败要打日志，不能静默吞。
            let st = state.clone();
            let handle = app.handle().clone();
            let proxy_started = tauri::async_runtime::block_on(proxy::start(&handle, &st));
            if let Err(e) = &proxy_started {
                eprintln!("[ms-manage] auto-start proxy failed: {e}");
            }

            tauri::async_runtime::spawn(async move {
                if proxy_started.is_ok() {
                    if let Ok(servers) = st.storage.load_servers() {
                        for srv in servers {
                            if srv.enabled && srv.auto_start.unwrap_or(false) {
                                if let Err(e) = proxy::start_one(&srv).await {
                                    eprintln!("[auto-start] failed to start {}: {}", srv.name, e);
                                }
                            }
                        }
                    }
                }
            });

            // 系统托盘：点关闭按钮时不真关，缩到托盘；托盘菜单可显窗口 / 真退出
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().cloned().expect("window icon must be set"))
                .tooltip("MS Manage")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app_handle, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app_handle.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键单击托盘 = 显示/聚焦窗口
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            if start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    window.hide()?;
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // 主窗口的关闭按钮 → 缩到托盘，不真退出
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            cmd_app::app_info,
            cmd_app::open_data_dir,
            cmd_app::open_external,
            cmd_doctor::doctor_check,
            cmd_servers::list_servers,
            cmd_servers::list_server_states,
            cmd_servers::get_server_state,
            cmd_servers::upsert_server,
            cmd_servers::delete_server,
            cmd_servers::start_server,
            cmd_servers::stop_server,
            cmd_servers::test_server,
            cmd_servers::fetch_server_tools,
            cmd_servers::set_tool_enabled,
            cmd_install::install_server,
            cmd_install::uninstall_server,
            cmd_install::refresh_install_status,
            cmd_projects::list_projects,
            cmd_projects::upsert_project,
            cmd_projects::delete_project,
            cmd_skills::list_skills,
            cmd_skills::get_skill,
            cmd_skills::upsert_skill,
            cmd_skills::delete_skill,
            cmd_skills::toggle_skill,
            cmd_skills::parse_skill_frontmatter,
            cmd_skills::list_skill_targets,
            cmd_skills::toggle_skill_target,
            cmd_skills::add_skill_target,
            cmd_skills::remove_skill_target,
            cmd_skills::update_skill_target,
            cmd_skills::import_skill_from_path,
            cmd_logs::list_logs,
            cmd_logs::clear_logs,
            cmd_market::fetch_market,
            cmd_market::install_market_item,
            cmd_settings::get_settings,
            cmd_settings::update_settings,
            cmd_settings::regenerate_token,
            cmd_settings::reveal_token,
            cmd_settings::rewrite_client_configs,
            cmd_settings::is_autostart_enabled,
            cmd_settings::enable_autostart,
            cmd_settings::disable_autostart,
            cmd_settings::get_auto_launch,
            cmd_settings::set_auto_launch,
            proxy_endpoint::get_proxy_status,
            proxy_endpoint::get_router_config,
            proxy_endpoint::start_proxy,
            proxy_endpoint::stop_proxy,
            proxy_endpoint::get_tools_total,
            proxy_endpoint::get_exposed_tools,
            proxy_endpoint::log_event,
        ])
        .build(tauri::generate_context!())
        .expect("error while building MS Manage")
        .run(|_app, event| {
            // 关 GUI = 关一切：杀掉所有 spawn 出去的 MCP 子进程，
            // 否则它们会变成孤儿继续占着端口/资源，外部仍然能感觉到它们在跑。
            // HTTP 代理是 GUI 进程内的 tokio task，进程退出会自动死，不用管。
            if matches!(event, tauri::RunEvent::Exit) {
                let _ = tauri::async_runtime::block_on(async {
                    crate::proxy::stop_all().await;
                });
            }
        });
}
