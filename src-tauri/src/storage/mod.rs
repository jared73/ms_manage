use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod servers;
pub mod skills;
pub mod logs;
pub mod memories;

/// 把 `contents` 原子写到 `path`：先写临时文件 + fsync，再 rename 到目标。
/// 避免写入中途崩溃/断电导致文件截断（之前直接 `std::fs::write` 容易出现 0 字节或半截文件）。
///
/// 设计取舍:**不**保留 .bak 历史副本。
/// - 旧实现每次写都 .bak 一份 → 重置 token、改 settings 这类高频路径会无限堆积旧文件,
///   而且 .bak 里也含 token 等敏感字段,是隐性泄漏。
/// - 解析失败时仍然在 load 入口单独备份为 .corrupt(那里是定位问题真正需要的);
///   写盘过程本身原子性已由 tmp + rename 保证,不再额外留 .bak。
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("atomic_write: path has no parent")?;
    std::fs::create_dir_all(parent).context("atomic_write: create parent")?;
    // 用 UUID 后缀的 tmp 文件名 —— 多任务同时写同一文件时,tmp 名不会撞。
    // 之前固定 `.tmp` 时,两个任务 A/B:
    //   A: write tmp + remove_file(target) + rename(tmp, target)
    //   B: write tmp(覆盖 A 的 tmp!) + remove_file(target) + rename(tmp, target)
    // B 的 write 会冲掉 A 的写入,触发数据丢失。现在用 UUID 后缀,A/B 各自独立。
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("file"),
        generate_tmp_id()
    ));
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).context("atomic_write: create tmp")?;
        f.write_all(contents).context("atomic_write: write tmp")?;
        f.sync_all().ok();
    }
    // rename 在同一文件系统上是原子的。
    // Windows 下若目标已存在,需要先 remove;这里用 ignore_errors 容错(路径被并发持有等),
    // 之后 rename 真出错就 return,不会留下半截文件。
    #[cfg(windows)]
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path).context("atomic_write: rename")?;
    // 防御性清理:rename 成功落地后,tmp 文件已被 mv 走,这里再加一道保险也无害。
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

/// 生成 12 字符的 tmp id(UUID-like 简版,够用避免命名冲突)。
fn generate_tmp_id() -> String {
    use rand::Rng;
    let bytes: [u8; 6] = rand::thread_rng().gen();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(12);
    for &x in &bytes {
        s.push(HEX[(x >> 4) as usize] as char);
        s.push(HEX[(x & 0x0f) as usize] as char);
    }
    s
}

pub fn atomic_write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context("serialize json")?;
    atomic_write(path, &bytes)
}
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

    // ---- 预安装（包管理）状态 ----
    /// 是否已预装到本地（全局 npm / uv tool）。非 stdio 类型或直接二进制类型无需安装。
    #[serde(default)]
    pub installed: bool,
    /// 已安装的版本号字符串（如 "1.2.3" 或 "2026.6.20"）
    #[serde(default)]
    pub installed_version: Option<String>,
    /// 已安装的可执行文件绝对路径（启动时优先用这个，避免每次 npx 下载）
    #[serde(default)]
    pub installed_path: Option<String>,
    /// 安装时间（ISO8601）
    #[serde(default)]
    pub installed_at: Option<String>,
    /// 上次安装失败原因（None = 上次成功或从未尝试）
    #[serde(default)]
    pub install_error: Option<String>,
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
    /// skill 目录下相对 SKILL.md 的所有文件路径（用于前端展示目录结构）
    #[serde(default)]
    pub files: Vec<String>,
}

/// 全局记忆文件条目：Markdown 主副本内联存储，target_path 是工具真实读取的绝对路径。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMemory {
    pub id: String,
    pub name: String,                  // 工具显示名（opencode / Claude Code / …）
    #[serde(default)]
    pub description: Option<String>,
    pub target_path: String,           // 同步目标绝对路径（如 ~/.config/opencode/AGENTS.md）
    #[serde(default = "default_true")]
    pub enabled: bool,                 // 启用时主副本复制到 target_path
    #[serde(default)]
    pub builtin: bool,                 // 预设条目不能删（只能 toggle / 编辑内容与路径）
    pub content: String,               // 主副本 Markdown 内容
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
    /// 使用统计各数据源目录覆盖。key: opencode/claude/codex/gemini/grokbuild，value: 覆盖路径
    #[serde(default)]
    pub usage_overrides: HashMap<String, String>,
    /// LLM 模型配置(用于 skill 中文摘要解析)。None = 未配置。
    #[serde(default)]
    pub model_config: Option<crate::model::ModelConfig>,
    /// 各供应商的 API Key 备份(per-provider)。用户切来切去时不要丢。
    /// key = provider id,value = 该供应商的 key。
    /// 不放在 model_config 里是因为:切换供应商时保留所有 key,但 model_config
    /// 只装当前激活的那一份。
    #[serde(default)]
    pub provider_keys: HashMap<String, String>,
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

/// 从损坏的 settings.json raw 文本里启发式挖 `mcp_token` 字段。
///
/// 设计取舍：**不引入 regex crate**，手写一个最小字符串扫描：
/// 1. 找到 `"mcp_token"` 字段名（允许引号内侧带/不带空格）；
/// 2. 跳过 `:` 和空白；
/// 3. 找到第一个 `"` 作为值起点；
/// 4. 处理 `\"` 转义,扫描到下一个未转义的 `"` 作为值终点。
///
/// 返回 `Some(token)` 表示挖到了非空 token;`None` 表示找不到或 token 为空。
///
/// 即使文件后半段被截断,只要 `mcp_token` 字段本体没坏,就能抢救。
fn recover_mcp_token(raw: &str) -> Option<String> {
    // 定位字段名 —— 兼容 `"mcp_token"` 与 `"mcp_token" ` 之类写法
    let key = "\"mcp_token\"";
    let key_pos = raw.find(key)?;
    let mut i = key_pos + key.len();
    let bytes = raw.as_bytes();
    // 跳过 : 与空白
    while i < bytes.len() && (bytes[i] == b':' || bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    // 期望下一个非空白字符是 `"`,否则当作结构异常,放弃
    if i >= bytes.len() || bytes[i] != b'"' {
        return None;
    }
    i += 1;
    let value_start = i;
    let mut out = String::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && i + 1 < bytes.len() {
            // 处理 `\"` 与 `\\` 两种最小集(我们的 token 只含 ASCII,实际不会触达,
            // 但写出来保证扫描不溢出)。其他转义保留字面下一个字符。
            let next = bytes[i + 1];
            out.push(next as char);
            i += 2;
            continue;
        }
        if b == b'"' {
            // 字段值结束
            if out.is_empty() {
                return None;
            }
            return Some(out);
        }
        out.push(b as char);
        i += 1;
    }
    // 跑到 raw 末尾都没找到收尾 `"` —— 文件被截断,接受已收集的部分,
    // 只要非空也返回(优先抢救,避免截断场景下白白重置 token)。
    if value_start < i && !out.is_empty() {
        Some(out)
    } else {
        None
    }
}

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
            theme: "light".into(),
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
            usage_overrides: HashMap::new(),
            model_config: None,
            provider_keys: HashMap::new(),
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

/// 内置的「全局记忆文件」预设（各 AI 工具的全局指令/规则文件路径）。
/// 默认全部 disabled —— 首次启用时才写入目标文件，且优先拉取已有内容。
pub fn default_memory_presets() -> Vec<StoredMemory> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut out = Vec::new();
    let now = String::new();
    let add = |out: &mut Vec<StoredMemory>, id: &str, name: &str, desc: &str, rel: std::path::PathBuf| {
        let content = memories::default_template(name);
        out.push(StoredMemory {
            id: id.to_string(),
            name: name.to_string(),
            description: Some(desc.to_string()),
            target_path: rel.to_string_lossy().to_string(),
            enabled: false,
            builtin: true,
            content,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    };
    add(&mut out, "opencode", "opencode", "全局 AGENTS.md", home.join(".config").join("opencode").join("AGENTS.md"));
    add(&mut out, "claude-code", "Claude Code", "全局 CLAUDE.md", home.join(".claude").join("CLAUDE.md"));
    add(&mut out, "codex", "Codex", "全局 AGENTS.md", home.join(".codex").join("AGENTS.md"));
    add(&mut out, "copilot", "GitHub Copilot", "全局自定义指令", home.join(".github").join("copilot-instructions.md"));
    out
}

/// 合并缺失的 builtin 记忆预设到已有列表（升级后自动补新预设，不重复、不覆盖用户数据）
pub fn merge_builtin_memories(existing: &mut Vec<StoredMemory>) {
    let defaults = default_memory_presets();
    for d in defaults {
        if existing.iter().any(|e| e.id == d.id) {
            // 已存在：仅当用户从未改过目标路径（还是旧预设路径）时，允许预设路径漂移修正
            continue;
        }
        existing.push(d);
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

#[derive(Clone)]
pub struct Storage {
    pub root: PathBuf,
    pub live_states: Arc<RwLock<LiveStatesMap>>,
    pub logs: Arc<RwLock<Vec<LogRecord>>>,
    pub proxy_stats: Arc<RwLock<ProxyStats>>,
    /// servers.json 文件读写互斥锁。
    /// 保护所有对 servers.json 的整体读-改-写，避免后台迁移与 GUI 写回互相覆盖。
    pub servers_lock: Arc<RwLock<()>>,
    /// memories.json 文件读写互斥锁。
    pub memories_lock: Arc<RwLock<()>>,
    /// projects.json 文件读写互斥锁。
    pub projects_lock: Arc<RwLock<()>>,
    /// settings.json 内存缓存 —— proxy 每个请求都要比对 token,
    /// 每次都 fs::read+serde_json::from_str+可能的原子写,既慢又有并发风险。
    /// 第一次 load 时落盘,后续命中缓存;save 时 invalidate 一次。
    pub settings_cache: Arc<RwLock<Option<StoredSettings>>>,
    /// memories.json 内存缓存 —— 与 settings_cache 同模式。
    pub memories_cache: Arc<RwLock<Option<Vec<StoredMemory>>>>,
    /// servers.json 内存缓存 —— `tools/list` 热路径每次都调 `load_servers()`,
    /// Claude Desktop / Cursor / Cline 这类客户端秒级高频调用,每次 fs::read+from_str
    /// 会成为瓶颈。跟 settings_cache 同模式:命中直接 clone,没命中才读盘 + 解析,
    /// mutate/upsert/remove 写盘后 invalidate + 重 load 回填。
    pub servers_cache: Arc<RwLock<Option<Vec<StoredServer>>>>,
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
            servers_lock: Arc::new(RwLock::new(())),
            memories_lock: Arc::new(RwLock::new(())),
            projects_lock: Arc::new(RwLock::new(())),
            settings_cache: Arc::new(RwLock::new(None)),
            servers_cache: Arc::new(RwLock::new(None)),
            memories_cache: Arc::new(RwLock::new(None)),
        };
        // 一次性把老的单 .md skill 迁移到目录制
        skills::migrate_legacy_flat_files(&storage.skills_dir()).ok();
        // 预热 memories_cache（冷启动竞态防护，同 settings_cache）
        let _ = storage.load_memories();
        // 启动时同步一遍 symlink（保证 enabled 状态与文件系统一致）
        // 同时预热 settings_cache:
        // - 让冷启动 race(UI get_settings 与 proxy first request 同时命中慢路径各自落盘)
        //   不再发生 —— 后续读全部命中缓存。
        // - 失败容错:load_settings() 出错也不影响 Storage 创建,后续调用会自己 retry。
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
    pub fn memories_file(&self) -> PathBuf { self.root.join("memories.json") }

    pub fn load_servers(&self) -> Result<Vec<StoredServer>> {
        // 命中缓存就直接 clone 返回 —— 代理 tools/list 热路径(秒级高频)不再走 IO。
        {
            let cache = self.servers_cache.read();
            if let Some(list) = cache.as_ref() {
                return Ok(list.clone());
            }
        }
        let _guard = self.servers_lock.read();
        let list = servers::load(&self.servers_file())?;
        *self.servers_cache.write() = Some(list.clone());
        Ok(list)
    }
    pub fn save_servers(&self, list: &[StoredServer]) -> Result<()> {
        let _guard = self.servers_lock.write();
        servers::save(&self.servers_file(), list)?;
        // 写盘后同步缓存,保持单一真相 —— 后续 load_servers 直接命中缓存,
        // 避免下一次 tools/list 又重新读盘。
        *self.servers_cache.write() = Some(list.to_vec());
        Ok(())
    }

    /// 原子地 读-改-写 单条 server 记录：整个过程持有写锁，
    /// 避免 load→mutate→save 三步之间被另一个任务覆盖（丢失更新）。
    /// 返回 `(found, changed)`:
    /// - `found=false` —— 找不到对应 id
    /// - `found=true, changed=false` —— 找到了但闭包未做修改(updated_at 不动)
    /// - `found=true, changed=true`  —— 闭包做了修改(已写盘并 bump updated_at)
    pub fn mutate_server<F: FnOnce(&mut StoredServer) -> bool>(
        &self,
        id: &str,
        f: F,
    ) -> Result<(bool, bool)> {
        let _guard = self.servers_lock.write();
        let mut list = servers::load(&self.servers_file())?;
        let Some(slot) = list.iter_mut().find(|s| s.id == id) else {
            return Ok((false, false));
        };
        let changed = f(slot);
        if changed {
            slot.updated_at = chrono::Utc::now().to_rfc3339();
            servers::save(&self.servers_file(), &list)?;
        }
        // 写盘后写穿缓存 —— 即使 changed=false 也重置,保证缓存与磁盘一致
        // (changed=false 时 list 是从磁盘 load 的最新值,缓存里可能是旧值,
        // 会与其他写路径冲突),此处保持单一真相。
        *self.servers_cache.write() = Some(list);
        Ok((true, changed))
    }

    /// 原子地 upsert（更新或插入）一条 server 记录。
    /// 持有写锁，避免并发 IPC 覆盖彼此。
    /// 返回 true 表示更新了已有记录，false 表示新插入。
    pub fn upsert_server(&self, server: &StoredServer) -> Result<bool> {
        let _guard = self.servers_lock.write();
        let mut list = servers::load(&self.servers_file())?;
        let updated;
        if let Some(slot) = list.iter_mut().find(|s| s.id == server.id) {
            *slot = server.clone();
            slot.updated_at = chrono::Utc::now().to_rfc3339();
            updated = true;
        } else {
            list.push(server.clone());
            updated = false;
        }
        servers::save(&self.servers_file(), &list)?;
        *self.servers_cache.write() = Some(list);
        Ok(updated)
    }

    /// 原子地删除一条 server 记录。
    pub fn remove_server(&self, id: &str) -> Result<()> {
        let _guard = self.servers_lock.write();
        let mut list = servers::load(&self.servers_file())?;
        list.retain(|s| s.id != id);
        servers::save(&self.servers_file(), &list)?;
        *self.servers_cache.write() = Some(list);
        Ok(())
    }

    pub fn load_memories(&self) -> Result<Vec<StoredMemory>> {
        {
            let cache = self.memories_cache.read();
            if let Some(list) = cache.as_ref() {
                return Ok(list.clone());
            }
        }
        let _guard = self.memories_lock.read();
        let mut list = if self.memories_file().exists() {
            let raw = std::fs::read_to_string(self.memories_file())?;
            serde_json::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("[storage] memories.json 解析失败（{e}），使用空列表");
                Vec::new()
            })
        } else {
            Vec::new()
        };
        // 合并缺失的 builtin 预设（升级后自动补新工具），落盘 + 填缓存
        let before = list.len();
        merge_builtin_memories(&mut list);
        let mut changed = list.len() != before;
        // 内置预设：目标文件已存在且主副本还是初始模板 → 读入真实内容并启用
        for m in list.iter_mut() {
            if memories::upgrade_preset_from_disk(m) {
                changed = true;
            }
        }
        if changed {
            let _ = atomic_write_json(&self.memories_file(), &list);
        }
        *self.memories_cache.write() = Some(list.clone());
        Ok(list)
    }
    pub fn save_memories(&self, list: &[StoredMemory]) -> Result<()> {
        let _guard = self.memories_lock.write();
        atomic_write_json(&self.memories_file(), list)?;
        *self.memories_cache.write() = Some(list.to_vec());
        Ok(())
    }
    /// 原子地 upsert（更新或插入）一条记忆条目。返回 true 表示更新已有，false 表示新增。
    pub fn upsert_memory(&self, m: &StoredMemory) -> Result<bool> {
        let _guard = self.memories_lock.write();
        let mut list: Vec<StoredMemory> = if self.memories_file().exists() {
            let raw = std::fs::read_to_string(self.memories_file())?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            Vec::new()
        };
        let updated;
        if let Some(slot) = list.iter_mut().find(|x| x.id == m.id) {
            *slot = m.clone();
            updated = true;
        } else {
            list.push(m.clone());
            updated = false;
        }
        atomic_write_json(&self.memories_file(), &list)?;
        *self.memories_cache.write() = Some(list);
        Ok(updated)
    }
    /// 原子地删除一条记忆条目。
    pub fn remove_memory(&self, id: &str) -> Result<()> {
        let _guard = self.memories_lock.write();
        let mut list: Vec<StoredMemory> = if self.memories_file().exists() {
            let raw = std::fs::read_to_string(self.memories_file())?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            Vec::new()
        };
        list.retain(|x| x.id != id);
        atomic_write_json(&self.memories_file(), &list)?;
        *self.memories_cache.write() = Some(list);
        Ok(())
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

    /// 原子地 upsert（更新或插入）一条 project 记录。
    pub fn upsert_project(&self, project: &StoredProject) -> Result<()> {
        let _guard = self.projects_lock.write();
        let mut list = projects::load(&self.projects_file())?;
        if let Some(slot) = list.iter_mut().find(|p| p.id == project.id) {
            *slot = project.clone();
        } else {
            list.push(project.clone());
        }
        projects::save(&self.projects_file(), &list)
    }

    /// 原子地删除一条 project 记录。
    pub fn remove_project(&self, id: &str) -> Result<()> {
        let _guard = self.projects_lock.write();
        let mut list = projects::load(&self.projects_file())?;
        list.retain(|p| p.id != id);
        projects::save(&self.projects_file(), &list)
    }

    /// 从 settings 拿当前配置的 HTTP 端口（用于 get_proxy_status）
    pub fn proxy_stats_loaded_port(&self) -> Option<u16> {
        self.load_settings().ok().map(|s| s.proxy_http_port)
    }
    pub fn load_settings(&self) -> Result<StoredSettings> {
        // 读路径:命中缓存就直接返回 —— proxy 每个请求都过一次,避免每次 fs::read。
        {
            let cache = self.settings_cache.read();
            if let Some(s) = cache.as_ref() {
                return Ok(s.clone());
            }
        }
        // 慢路径:读盘 + 解析 + 必要的自愈落盘,落盘后塞进缓存。
        // 冷启动并发:多个线程同时看到空缓存时,可能各自生成 token 并写盘,
        // 但只会有第一个写入生效(原子写 rename),后续的覆盖它。
        // 影响仅限冷启动阶段,实际场景只有一个 GUI 线程,不需要加锁。
        let p = self.settings_file();
        if !p.exists() {
            let d = StoredSettings::default();
            atomic_write_json(&p, &d)?;
            *self.settings_cache.write() = Some(d.clone());
            return Ok(d);
        }
        let raw = std::fs::read_to_string(&p)?;
        // 解析失败：不要静默回退默认（会重新生成 mcp_token，让客户端全部失联）。
        // 备份损坏文件为 .corrupt，然后用默认值起步，mcp_token 保留为 None
        // 让下面的补生成逻辑只在"真没有 token"时触发。
        let mut s: StoredSettings = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                let corrupt = p.with_extension("corrupt");
                let _ = std::fs::copy(&p, &corrupt);
                eprintln!(
                    "[storage] settings.json 解析失败（{}），已备份到 {}，使用默认设置重新生成",
                    e, corrupt.display()
                );
                // 尝试从损坏的 raw 文本里把 mcp_token 挖出来 —— 解析失败的常见场景
                // 是文件被截断/部分损坏,但 mcp_token 字段本身还在。如果能挖到,
                // 就保留它,避免所有客户端失联 401。
                let recovered = recover_mcp_token(&raw);
                if let Some(tok) = recovered.as_deref() {
                    crate::proxy::log_event(
                        "info",
                        "storage",
                        "recovered mcp_token from corrupt settings.json".to_string(),
                        None,
                    );
                    StoredSettings {
                        mcp_token: Some(tok.to_string()),
                        ..StoredSettings::default()
                    }
                } else {
                    crate::proxy::log_event(
                        "error",
                        "storage",
                        "mcp_token was reset due to corrupt settings.json; all clients must update their config".to_string(),
                        None,
                    );
                    StoredSettings {
                        mcp_token: None,
                        ..StoredSettings::default()
                    }
                }
            }
        };
        // 主题向后兼容：dark/system/未知 → purple，light → light
        s.theme = match s.theme.as_str() {
            "purple" | "midnight" | "forest" | "light" => s.theme.clone(),
            _ => "purple".into(),
        };
        if s.mcp_token.as_deref().map(|t| t.is_empty()).unwrap_or(true) {
            s.mcp_token = Some(generate_token());
            let _ = atomic_write_json(&p, &s);
        }
        // 合并缺失的 builtin 目标（用户升级后能自动拿到新加的预设）
        let before = s.skill_targets.len();
        merge_builtin_skill_targets(&mut s.skill_targets);
        if s.skill_targets.len() != before {
            let _ = atomic_write_json(&p, &s);
        }
        *self.settings_cache.write() = Some(s.clone());
        Ok(s)
    }
    pub fn save_settings(&self, s: &StoredSettings) -> Result<()> {
        atomic_write_json(&self.settings_file(), s)?;
        // 写盘后立刻刷缓存 —— 保持单一真相,下次 load_settings 命中刚写的副本。
        *self.settings_cache.write() = Some(s.clone());
        Ok(())
    }
}
