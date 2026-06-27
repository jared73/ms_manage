#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    use ai_manage_lib::cli::{parse, Command};
    match parse() {
        Command::Gui => ai_manage_lib::run(),
        Command::Help => ai_manage_lib::cli::print_help(),
        Command::ProxyStdio | Command::Serve => {
            let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
            rt.block_on(async {
                if let Err(e) = ai_manage_lib::proxy::run_stdio().await {
                    eprintln!("[ms-manage] stdio proxy error: {e}");
                    std::process::exit(1);
                }
            });
        }
        Command::ProxyHttp { port } => {
            let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
            rt.block_on(async {
                let state = match ai_manage_lib::state::AppState::initialize() {
                    Ok(s) => std::sync::Arc::new(s),
                    Err(e) => { eprintln!("init failed: {e}"); std::process::exit(1); }
                };
                if let Err(e) = ai_manage_lib::proxy::connect::run_http(state, port).await {
                    eprintln!("http proxy error: {e}");
                    std::process::exit(1);
                }
            });
        }
        Command::Connect { target } => {
            ai_manage_lib::cli::connect(target);
        }
        Command::MarketInstall { id } => {
            ai_manage_lib::cmd_market::install_from_cli(&id);
        }
        Command::SkillsList => {
            ai_manage_lib::cli::skills_list();
        }
        Command::SkillsReconcile => {
            ai_manage_lib::cli::skills_reconcile();
        }
        Command::ProjectsList => {
            ai_manage_lib::cli::projects_list();
        }
        Command::Diagnose => {
            ai_manage_lib::cli::diagnose();
        }
        Command::ExportMcp { path, mode } => {
            ai_manage_lib::cli::export_mcp(path, mode);
        }
    }
}
