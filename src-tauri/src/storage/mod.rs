use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub mod servers;
pub mod skills;
pub mod logs;
pub mod projects;

pub use logs::LogRecord;

pub const UNASSIGNED_PROJECT_ID: &str = "__unassigned__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredServer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub transport: String, // "stdio" | "sse" | "http"
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_start: Option<bool>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub project_id: Option<String>, // None = UNASSIGNED
    /// 用户在这个 server 上**关闭**的工具名列表（被关闭的不出现在 tools/list 响应里）
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredSkill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub content: String,
    #[serde(default)]
    pub frontmatter: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub dir_path: String,         // skill 目录（包含 SKILL.md）
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProject {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSettings {
    pub language: String,
    pub theme: String,
    pub auto_launch: bool,
    pub proxy_mode: String,
    pub proxy_http_port: u16,
    pub registry_url: Option<String>,
    pub logs_max_entries: usize,
    #[serde(default)]
    pub auto_start_proxy: Option<bool>,
    #[serde(default = "default_npm_registry")]
    pub npm_registry: String,
    #[serde(default)]
    pub auto_inject_npm_registry: bool,
    #[serde(default)]
    pub mcp_token: Option<String>,
    #[serde(default)]
    pub github_token: Option<String>,
    /// 技能同步目标（每个编辑器一个），含 builtin 预设 + 用户自加
    #[serde(default)]
    pub skill_targets: Vec<SkillTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTarget {
    pub id: String,
    pub name: String,
    pub path: String,   // 字符串存，序列化跨平台 OK
    pub enabled: bool,
    #[serde(default)]
    pub builtin: bool,  // 预设不能删，只能 toggle；用户自加的可以删
}

fn default_npm_registry() -> String { "https://registry.npmmirror.com".to_string() }

pub fn generate_token() -> String {
    use rand::Rng;
    let bytes: [u8; 24] = rand::thread_rng().gen();
    format!("mcpr_{}", hex_encode(&bytes))
}

fn hex_encode(b: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push(HEX[(x >> 4) as usize] as char);
        s.push(HEX[(x & 0x0f) as usize] as char);
    }
    s
}

impl Default for StoredSettings {
    fn default() -> Self {
        Self {
            language: "zh".into(),
            theme: "purple".into(),
            auto_launch: false,
            proxy_mode: "stdio".into(),
            proxy_http_port: 39231,
            registry_url: None,
            logs_max_entries: 5000,
            auto_start_proxy: Some(true),
            npm_registry: default_npm_registry(),
            auto_inject_npm_registry: true,
            mcp_token: Some(generate_token()),
            github_token: None,
            skill_targets: default_skill_targets(),
        }
    }
}

/// 内置的编辑器/IDE skill 同步目标预设
pub fn default_skill_targets() -> Vec<SkillTarget> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut out = Vec::new();
    let add = |out: &mut Vec<SkillTarget>, id: &str, name: &str, path: std::path::PathBuf, enabled: bool| {
        out.push(SkillTarget {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            enabled,
            builtin: true,
        });
    };
    // 默认开启：用户最常用的 3 个
    add(&mut out, "claude-code", "Claude Code", home.join(".claude").join("skills"), true);
    add(&mut out, "agents",      "通用",         home.join(".agents").join("skills"), true);
    add(&mut out, "opencode",    "opencode",    home.join(".config").join("opencode").join("skill"), true);
    // 默认关闭：装了就开
    add(&mut out, "openclaw",    "openclaw",    home.join(".openclaw").join("skills"), false);
    add(&mut out, "cline",       "Cline",       home.join(".cline").join("skills"), false);
    add(&mut out, "codex",       "Codex",       home.join(".codex").join("skills"), false);
    add(&mut out, "copilot",     "GitHub Copilot", home.join(".copilot").join("skills"), false);
    out
}

/// 合并缺失的 builtin 目标到已有设置里（用户没删的、没改名的都补上）
pub fn merge_builtin_skill_targets(existing: &mut Vec<SkillTarget>) {
    let defaults = default_skill_targets();
    for d in defaults {
        if !existing.iter().any(|e| e.id == d.id) {
            existing.push(d);
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxyStats {
    pub running: bool,
    pub started_at: Option<String>,
    pub connected_clients: u32,
    pub total_requests: u64,
    pub last_request_at: Option<String>,
}

pub type LiveStatesMap = HashMap<String, String>; // id -> status str

pub struct Storage {
    pub root: PathBuf,
    pub live_states: Arc<RwLock<LiveStatesMap>>,
    pub logs: Arc<RwLock<Vec<LogRecord>>>,
    pub proxy_stats: Arc<RwLock<ProxyStats>>,
}

impl Storage {
    pub fn new(root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root).context("create data dir")?;
        std::fs::create_dir_all(root.join("skills")).context("create skills dir")?;
        let storage = Self {
            root,
            live_states: Arc::new(RwLock::new(HashMap::new())),
            logs: Arc::new(RwLock::new(Vec::new())),
            proxy_stats: Arc::new(RwLock::new(ProxyStats::default())),
        };
        // 一次性把老的单 .md skill 迁移到目录制
        skills::migrate_legacy_flat_files(&storage.skills_dir()).ok();
        // 启动时同步一遍 symlink（保证 enabled 状态与文件系统一致）
        if let Ok(all) = storage.load_skills() {
            if let Ok(settings) = storage.load_settings() {
                let _ = skills::reconcile_symlinks(&all, &settings.skill_targets);
            }
        }
        Ok(storage)
    }

    pub fn servers_file(&self) -> PathBuf { self.root.join("servers.json") }
    pub fn skills_dir(&self) -> PathBuf { self.root.join("skills") }
    pub fn projects_file(&self) -> PathBuf { self.root.join("projects.json") }
    pub fn settings_file(&self) -> PathBuf { self.root.join("settings.json") }
    pub fn log_file(&self) -> PathBuf { self.root.join("logs.json") }

    pub fn load_servers(&self) -> Result<Vec<StoredServer>> {
        servers::load(&self.servers_file())
    }
    pub fn save_servers(&self, list: &[StoredServer]) -> Result<()> {
        servers::save(&self.servers_file(), list)
    }
    pub fn load_skills(&self) -> Result<Vec<StoredSkill>> {
        skills::load_all(&self.skills_dir())
    }
    pub fn save_skill(&self, skill: &StoredSkill) -> Result<()> {
        skills::save(&self.skills_dir(), skill)
    }
    pub fn delete_skill(&self, id: &str) -> Result<()> {
        skills::delete(&self.skills_dir(), id)
    }
    pub fn load_projects(&self) -> Result<Vec<StoredProject>> {
        projects::load(&self.projects_file())
    }
    pub fn save_projects(&self, list: &[StoredProject]) -> Result<()> {
        projects::save(&self.projects_file(), list)
    }

    /// 从 settings 拿当前配置的 HTTP 端口（用于 get_proxy_status）
    pub fn proxy_stats_loaded_port(&self) -> Option<u16> {
        self.load_settings().ok().map(|s| s.proxy_http_port)
    }
    pub fn load_settings(&self) -> Result<StoredSettings> {
        let p = self.settings_file();
        if !p.exists() {
            let d = StoredSettings::default();
            let json = serde_json::to_string_pretty(&d)?;
            std::fs::write(&p, json)?;
            return Ok(d);
        }
        let raw = std::fs::read_to_string(&p)?;
        let mut s: StoredSettings = serde_json::from_str(&raw).unwrap_or_default();
        // 主题向后兼容：dark/system/未知 → purple，light → light
        s.theme = match s.theme.as_str() {
            "purple" | "midnight" | "forest" | "light" => s.theme.clone(),
            _ => "purple".into(),
        };
        if s.mcp_token.is_none() {
            s.mcp_token = Some(StoredSettings::default().mcp_token.unwrap_or_default());
            let _ = std::fs::write(&p, serde_json::to_string_pretty(&s)?);
        }
        // 合并缺失的 builtin 目标（用户升级后能自动拿到新加的预设）
        let before = s.skill_targets.len();
        merge_builtin_skill_targets(&mut s.skill_targets);
        if s.skill_targets.len() != before {
            let _ = std::fs::write(&p, serde_json::to_string_pretty(&s)?);
        }
        Ok(s)
    }
    pub fn save_settings(&self, s: &StoredSettings) -> Result<()> {
        let json = serde_json::to_string_pretty(s)?;
        std::fs::write(self.settings_file(), json)?;
        Ok(())
    }
}
