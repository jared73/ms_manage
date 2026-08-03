use dirs::home_dir;
use std::path::PathBuf;

/// 各工具数据目录探测。后续 Task 15 会加覆盖配置。
pub struct UsagePaths {
    pub opencode_db: PathBuf,
    pub claude_projects: PathBuf,
    pub codex_sessions: PathBuf,
    pub gemini_dir: PathBuf,
    pub grok_dir: PathBuf,
}

impl UsagePaths {
    pub fn detect() -> Self {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));

        // OpenCode: OPENCODE_DB env > XDG_DATA_HOME > ~/.local/share/opencode
        let opencode_data = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));
        let opencode_db = std::env::var("OPENCODE_DB")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| opencode_data.join("opencode").join("opencode.db"));

        UsagePaths {
            opencode_db,
            claude_projects: home.join(".claude").join("projects"),
            codex_sessions: home.join(".codex"),
            gemini_dir: home.join(".gemini"),
            grok_dir: home.join(".grok"),
        }
    }
}
