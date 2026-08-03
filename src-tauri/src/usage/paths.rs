use dirs::home_dir;
use std::path::PathBuf;

/// 各工具数据目录探测。
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

    /// 应用覆盖配置后的路径（未配置的保持默认探测结果）
    pub fn apply_overrides(mut self, overrides: &std::collections::HashMap<String, String>) -> Self {
        if let Some(p) = overrides.get("opencode") {
            if !p.is_empty() { self.opencode_db = std::path::PathBuf::from(p); }
        }
        if let Some(p) = overrides.get("claude") {
            if !p.is_empty() { self.claude_projects = std::path::PathBuf::from(p); }
        }
        if let Some(p) = overrides.get("codex") {
            if !p.is_empty() { self.codex_sessions = std::path::PathBuf::from(p); }
        }
        if let Some(p) = overrides.get("gemini") {
            if !p.is_empty() { self.gemini_dir = std::path::PathBuf::from(p); }
        }
        if let Some(p) = overrides.get("grokbuild") {
            if !p.is_empty() { self.grok_dir = std::path::PathBuf::from(p); }
        }
        self
    }
}
