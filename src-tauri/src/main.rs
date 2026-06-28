#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    use ms_manage_lib::cli::{parse, Command};
    match parse() {
        Command::Gui => ms_manage_lib::run(),
        Command::Help => ms_manage_lib::cli::print_help(),
        Command::ProxyStdio | Command::Serve => {
            let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
            rt.block_on(async {
                if let Err(e) = ms_manage_lib::proxy::run_stdio().await {
                    eprintln!("[ms-manage] stdio proxy error: {e}");
                    std::process::exit(1);
                }
            });
        }
        Command::ProxyHttp { port } => {
            let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
            rt.block_on(async {
                let state = match ms_manage_lib::state::AppState::initialize() {
                    Ok(s) => std::sync::Arc::new(s),
                    Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
                };
                if let Err(e) = ms_manage_lib::proxy::connect::run_http(state, port).await {
                    eprintln!("http proxy error: {e}");
                    std::process::exit(1);
                }
            });
        }
        Command::Connect { target } => {
            ms_manage_lib::cli::connect(target);
        }
        Command::MarketInstall { id } => {
            ms_manage_lib::cmd_market::install_from_cli(&id);
        }
        Command::SkillsList => {
            ms_manage_lib::cli::skills_list();
        }
        Command::SkillsReconcile => {
            ms_manage_lib::cli::skills_reconcile();
        }
        Command::ProjectsList => {
            ms_manage_lib::cli::projects_list();
        }
        Command::Diagnose => {
            ms_manage_lib::cli::diagnose();
        }
        Command::ExportMcp { path, mode } => {
            ms_manage_lib::cli::export_mcp(path, mode);
        }
    }
}
