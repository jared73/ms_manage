# 使用统计模块 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 ms_manage 添加"使用统计"模块：解析 5 种 AI 工具（OpenCode/Claude Code/Codex/Gemini/Grok Build）的本地会话数据，聚合展示 token 用量与费用，支持按应用/模型/日期趋势查看，含可编辑的模型定价表。

**Architecture:** Rust 后端新增 `usage/` 模块，用 SQLite（`rusqlite` bundled）存储 `request_logs`/`usage_daily_rollups`/`model_pricing`/`session_log_sync` 四张表；5 个数据源解析器各自把工具本地的 SQLite/JSONL 数据增量同步进来；`cmd_usage.rs` 暴露 IPC 命令；前端新增 `/usage` 页面用 recharts 展示。后台每 60 秒自动增量同步。

**Tech Stack:** Rust + rusqlite(bundled) + rust_decimal + Tauri 2 IPC；React + TypeScript + recharts + react-i18next + Tailwind。

参考实现：`D:\Jared\plan\cc-switch`（farion1231/cc-switch）的 `src-tauri/src/services/session_usage*.rs`、`src-tauri/src/proxy/usage/calculator.rs`、`src-tauri/src/services/usage_stats.rs`、`src-tauri/src/database/schema.rs`。

---

### Task 1: 添加依赖（rusqlite、rust_decimal）

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`（前端 recharts 在 Task 10 加，此任务只加 Rust 依赖）

- [ ] **Step 1: 在 Cargo.toml 添加依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中 `url = "2"` 之后添加：

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
rust_decimal = "1"
```

- [ ] **Step 2: 验证编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 编译通过，无报错。bundled SQLite 首次编译需要 1-2 分钟。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(deps): add rusqlite + rust_decimal for usage stats"
```

---

### Task 2: 数据库层（usage/db.rs）

**Files:**
- Create: `src-tauri/src/usage/db.rs`
- Create: `src-tauri/src/usage/mod.rs`

- [ ] **Step 1: 写数据库打开与建表代码**

创建 `src-tauri/src/usage/mod.rs`：

```rust
pub mod db;
pub mod paths;
pub mod pricing;
pub mod session_claude;
pub mod session_codex;
pub mod session_gemini;
pub mod session_grokbuild;
pub mod session_opencode;
pub mod stats;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

/// usage.db 全局句柄，持有连接。所有 usage 模块共享。
pub struct UsageDb {
    pub conn: Mutex<rusqlite::Connection>,
}

impl UsageDb {
    pub fn open(path: &std::path::Path) -> Result<Arc<Self>> {
        let conn = rusqlite::Connection::open(path)
            .with_context(|| format!("打开 usage.db 失败: {}", path.display()))?;
        let db = Arc::new(UsageDb { conn: Mutex::new(conn) });
        db::init_schema(&db)?;
        Ok(db)
    }
}
```

创建 `src-tauri/src/usage/db.rs`：

```rust
use anyhow::{Context, Result};

pub fn init_schema(db: &crate::usage::UsageDb) -> Result<()> {
    let conn = db.conn.lock();
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS request_logs (
            request_id TEXT PRIMARY KEY,
            provider_id TEXT NOT NULL,
            app_type TEXT NOT NULL,
            model TEXT NOT NULL,
            request_model TEXT,
            pricing_model TEXT,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            input_cost_usd TEXT NOT NULL DEFAULT '0',
            output_cost_usd TEXT NOT NULL DEFAULT '0',
            cache_read_cost_usd TEXT NOT NULL DEFAULT '0',
            cache_creation_cost_usd TEXT NOT NULL DEFAULT '0',
            total_cost_usd TEXT NOT NULL DEFAULT '0',
            latency_ms INTEGER NOT NULL DEFAULT 0,
            first_token_ms INTEGER,
            duration_ms INTEGER,
            status_code INTEGER NOT NULL DEFAULT 200,
            error_message TEXT,
            session_id TEXT,
            provider_type TEXT,
            is_streaming INTEGER NOT NULL DEFAULT 1,
            cost_multiplier TEXT NOT NULL DEFAULT '1.0',
            created_at INTEGER NOT NULL,
            data_source TEXT NOT NULL DEFAULT 'session'
        );
        CREATE INDEX IF NOT EXISTS idx_request_logs_created_at ON request_logs(created_at);
        CREATE INDEX IF NOT EXISTS idx_request_logs_app_type ON request_logs(app_type);
        CREATE INDEX IF NOT EXISTS idx_request_logs_model ON request_logs(model);
        CREATE INDEX IF NOT EXISTS idx_request_logs_session ON request_logs(session_id);

        CREATE TABLE IF NOT EXISTS usage_daily_rollups (
            date TEXT NOT NULL,
            app_type TEXT NOT NULL,
            model TEXT NOT NULL,
            request_count INTEGER NOT NULL DEFAULT 0,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost_usd TEXT NOT NULL DEFAULT '0',
            PRIMARY KEY (date, app_type, model)
        );

        CREATE TABLE IF NOT EXISTS model_pricing (
            model_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            input_cost_per_million TEXT NOT NULL,
            output_cost_per_million TEXT NOT NULL,
            cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
            cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
        );

        CREATE TABLE IF NOT EXISTS session_log_sync (
            file_path TEXT PRIMARY KEY,
            last_modified INTEGER NOT NULL,
            last_line_offset INTEGER NOT NULL DEFAULT 0,
            last_synced_at INTEGER NOT NULL
        );
        "#,
    )
    .context("初始化 usage.db schema 失败")
}
```

- [ ] **Step 2: 注册模块到 lib.rs**

在 `src-tauri/src/lib.rs` 的 `pub mod symlink;` 之后加：

```rust
pub mod usage;
```

- [ ] **Step 3: 编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 编译通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/usage src-tauri/src/lib.rs
git commit -m "feat(usage): usage.db schema + UsageDb connection"
```

---

### Task 3: 工具目录探测（usage/paths.rs）

**Files:**
- Create: `src-tauri/src/usage/paths.rs`

- [ ] **Step 1: 写路径探测逻辑**

```rust
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
```

- [ ] **Step 2: 编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/paths.rs
git commit -m "feat(usage): tool data directory detection"
```

---

### Task 4: 模型定价 + 费用计算（usage/pricing.rs）

**Files:**
- Create: `src-tauri/src/usage/pricing.rs`

- [ ] **Step 1: 写定价结构与费用计算器（含单测）**

```rust
use anyhow::{Context, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromStr;
use std::str::FromStr;

/// 模型定价（每百万 token 的 USD 价格）
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_million: Decimal,
    pub output_cost_per_million: Decimal,
    pub cache_read_cost_per_million: Decimal,
    pub cache_creation_cost_per_million: Decimal,
}

impl ModelPricing {
    pub fn from_strings(
        input: &str,
        output: &str,
        cache_read: &str,
        cache_creation: &str,
    ) -> Result<Self> {
        Ok(Self {
            input_cost_per_million: Decimal::from_str(input).context("input 定价非法")?,
            output_cost_per_million: Decimal::from_str(output).context("output 定价非法")?,
            cache_read_cost_per_million: Decimal::from_str(cache_read).context("cache_read 定价非法")?,
            cache_creation_cost_per_million: Decimal::from_str(cache_creation).context("cache_creation 定价非法")?,
        })
    }
}

/// 一次请求的 token 用量
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CostBreakdown {
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub cache_read_cost: Decimal,
    pub cache_creation_cost: Decimal,
    pub total_cost: Decimal,
}

/// 费用计算器。input_includes_cache_read=true 表示 input_tokens 已含 cache 部分
/// （OpenAI/Codex/Gemini 风格），需扣除后再按输入价计费。
pub fn calculate_cost(
    usage: &TokenUsage,
    pricing: &ModelPricing,
    input_includes_cache_read: bool,
) -> CostBreakdown {
    let million = Decimal::from(1_000_000u64);
    let billable_input = if input_includes_cache_read {
        usage
            .input_tokens
            .saturating_sub(usage.cache_read_tokens)
            .saturating_sub(usage.cache_creation_tokens)
    } else {
        usage.input_tokens
    };
    let mut c = CostBreakdown {
        input_cost: Decimal::from(billable_input) * pricing.input_cost_per_million / million,
        output_cost: Decimal::from(usage.output_tokens) * pricing.output_cost_per_million / million,
        cache_read_cost: Decimal::from(usage.cache_read_tokens)
            * pricing.cache_read_cost_per_million
            / million,
        cache_creation_cost: Decimal::from(usage.cache_creation_tokens)
            * pricing.cache_creation_cost_per_million
            / million,
        ..Default::default()
    };
    c.total_cost = c.input_cost + c.output_cost + c.cache_read_cost + c.cache_creation_cost;
    c
}

/// app_type 的 input_tokens 是否包含 cache read（true: codex/gemini/grokbuild; false: opencode/claude）
pub fn input_includes_cache_read(app_type: &str) -> bool {
    matches!(app_type, "codex" | "gemini" | "grokbuild")
}

/// 从 usage.db 查询模型定价；未找到则从内置 seed 表加载并尝试落库。
pub fn find_model_pricing(
    conn: &rusqlite::Connection,
    model_id: &str,
) -> Option<ModelPricing> {
    let row = conn
        .query_row(
            "SELECT input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million
             FROM model_pricing WHERE model_id = ?1",
            [model_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .ok();
    match row {
        Some((i, o, cr, cc)) => ModelPricing::from_strings(&i, &o, &cr, &cc).ok(),
        None => {
            // 尝试 seed 内置定价
            let seed = seed_pricing(model_id)?;
            let _ = conn.execute(
                "INSERT OR IGNORE INTO model_pricing (model_id, display_name,
                    input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million)
                 VALUES (?1, ?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![model_id, seed.0, seed.1, seed.2, seed.3],
            );
            Some(seed.4)
        }
    }
}

/// 内置定价 seed：model_id -> (input, output, cache_read, cache_creation, ModelPricing)
fn seed_pricing(model_id: &str) -> Option<(&'static str, &'static str, &'static str, &'static str, ModelPricing)> {
    let entry: Option<(&str, &str, &str, &str)> = match model_id {
        m if m.starts_with("claude-3-5-sonnet") => Some(("3.0", "15.0", "0.3", "3.75")),
        m if m.starts_with("claude-sonnet-4") => Some(("3.0", "15.0", "0.3", "3.75")),
        m if m.starts_with("claude-3-7-sonnet") => Some(("3.0", "15.0", "0.3", "3.75")),
        m if m.starts_with("claude-3-5-haiku") => Some(("0.80", "4.0", "0.08", "1.0")),
        m if m.starts_with("claude-3-opus") => Some(("15.0", "75.0", "1.5", "18.75")),
        m if m.starts_with("gpt-4o") => Some(("2.50", "10.0", "1.25", "2.50")),
        m if m.starts_with("gpt-4.1") => Some(("2.0", "8.0", "0.5", "2.0")),
        m if m.starts_with("o3") || m.starts_with("o4") => Some(("2.0", "8.0", "0.5", "2.0")),
        m if m.starts_with("gemini-2.5-pro") => Some(("1.25", "10.0", "0.3125", "1.25")),
        m if m.starts_with("gemini-2.5-flash") => Some(("0.30", "2.50", "0.075", "0.30")),
        m if m.starts_with("deepseek") && m.contains("reasoner") => Some(("0.55", "2.19", "0.14", "0.55")),
        m if m.starts_with("deepseek") => Some(("0.27", "1.10", "0.07", "0.27")),
        m if m.starts_with("grok-4.5-build") => Some(("2.0", "6.0", "0.30", "2.0")),
        m if m.starts_with("grok") => Some(("2.0", "6.0", "0.30", "2.0")),
        m if m.starts_with("qwen") => Some(("0.50", "2.0", "0.10", "0.50")),
        _ => None,
    }?;
    let p = ModelPricing::from_strings(entry.0, entry.1, entry.2, entry.3).ok()?;
    Some((entry.0, entry.1, entry.2, entry.3, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_anthropic_semantics() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            model: None,
        };
        let pricing = ModelPricing::from_strings("3.0", "15.0", "0.3", "3.75").unwrap();
        let c = calculate_cost(&usage, &pricing, false);
        assert_eq!(c.input_cost, Decimal::from_str("0.003").unwrap());
        assert_eq!(c.output_cost, Decimal::from_str("0.0075").unwrap());
        assert_eq!(c.cache_read_cost, Decimal::from_str("0.00006").unwrap());
        assert_eq!(c.cache_creation_cost, Decimal::from_str("0.000375").unwrap());
        assert_eq!(c.total_cost, Decimal::from_str("0.010935").unwrap());
    }

    #[test]
    fn test_calculate_cache_inclusive() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            model: None,
        };
        let pricing = ModelPricing::from_strings("3.0", "15.0", "0.3", "3.75").unwrap();
        let c = calculate_cost(&usage, &pricing, true);
        assert_eq!(c.input_cost, Decimal::from_str("0.0021").unwrap());
    }

    #[test]
    fn test_seed_lookup() {
        assert!(seed_pricing("claude-sonnet-4-20250514").is_some());
        assert!(seed_pricing("grok-4.5-build").is_some());
        assert!(seed_pricing("totally-unknown-model").is_none());
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test pricing` (workdir `src-tauri`)
Expected: 3 个测试通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/pricing.rs
git commit -m "feat(usage): model pricing seed table + cost calculator"
```

---

### Task 5: 同步通用工具（session 状态 + 增量水位线）

**Files:**
- Create: `src-tauri/src/usage/session_common.rs`（在 mod.rs 注册）

- [ ] **Step 1: 写通用同步结构与状态存取**

创建 `src-tauri/src/usage/session_common.rs`：

```rust
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::time::SystemTime;

/// 一次同步的结果汇总
#[derive(Debug, Clone, Default)]
pub struct SessionSyncResult {
    pub imported: u32,
    pub skipped: u32,
    pub files_scanned: u32,
    pub suspected_duplicates: u32,
    pub deferred_files: u32,
    pub errors: Vec<String>,
}

impl SessionSyncResult {
    pub fn merge(&mut self, other: SessionSyncResult) {
        self.imported += other.imported;
        self.skipped += other.skipped;
        self.files_scanned += other.files_scanned;
        self.suspected_duplicates += other.suspected_duplicates;
        self.deferred_files += other.deferred_files;
        self.errors.extend(other.errors);
    }
}

/// 读取 session_log_sync 中的同步进度，返回 (last_modified, last_line_offset)
pub fn get_sync_state(db: &UsageDb, file_path: &str) -> Result<(i64, i64)> {
    let conn = db.conn.lock();
    let result = conn.query_row(
        "SELECT last_modified, last_line_offset FROM session_log_sync WHERE file_path = ?1",
        [file_path],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    );
    Ok(result.unwrap_or((0, 0)))
}

/// 文件 mtime 的纳秒时间戳
pub fn metadata_modified_nanos(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// 更新同步进度
pub fn update_sync_state(db: &UsageDb, file_path: &str, last_modified: i64, line_offset: i64) -> Result<()> {
    let conn = db.conn.lock();
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO session_log_sync (file_path, last_modified, last_line_offset, last_synced_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(file_path) DO UPDATE SET
            last_modified = excluded.last_modified,
            last_line_offset = excluded.last_line_offset,
            last_synced_at = excluded.last_synced_at",
        rusqlite::params![file_path, last_modified, line_offset, now],
    )
    .context("更新同步状态失败")?;
    Ok(())
}

/// 插入一条请求日志。request_id 冲突时跳过（INSERT OR IGNORE）。
/// 返回 true = 新插入，false = 已存在跳过。
pub fn insert_request_log(
    db: &UsageDb,
    request_id: &str,
    app_type: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    costs: &crate::usage::pricing::CostBreakdown,
    total_cost_usd: &str,
    session_id: Option<&str>,
    created_at: i64,
) -> Result<bool> {
    let conn = db.conn.lock();
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO request_logs (
            request_id, provider_id, app_type, model, request_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd, total_cost_usd,
            latency_ms, status_code, session_id, is_streaming, cost_multiplier, created_at, data_source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        rusqlite::params![
            request_id,
            format!("_{app_type}_session"),
            app_type,
            model,
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            total_cost_usd,
            0i64,
            200i64,
            session_id,
            1i64,
            "1.0",
            created_at,
            "session",
        ],
    )
    .context("插入 request_logs 失败")?;
    Ok(inserted > 0)
}
```

在 `src-tauri/src/usage/mod.rs` 顶部加：

```rust
pub mod session_common;
```

- [ ] **Step 2: 编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/session_common.rs src-tauri/src/usage/mod.rs
git commit -m "feat(usage): session sync state + request log insert helper"
```

---

### Task 6: OpenCode 同步（session_opencode.rs）

**Files:**
- Create: `src-tauri/src/usage/session_opencode.rs`

- [ ] **Step 1: 写 OpenCode SQLite 解析（含单测）**

```rust
use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, insert_request_log, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::time::SystemTime;

/// 同步 OpenCode 使用数据（只读打开 ~/.local/share/opencode/opencode.db）
pub fn sync_opencode_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let db_path = &paths.opencode_db;
    let mut result = SessionSyncResult::default();

    if !db_path.exists() {
        return Ok(result);
    }

    let db_path_str = db_path.to_string_lossy().to_string();

    // opencode 是 WAL 模式，需同时考虑 -wal 的 mtime
    let metadata = fs::metadata(db_path).context("无法读取 opencode.db 元数据")?;
    let mut file_modified = metadata_modified_nanos(&metadata);
    let wal_path = db_path.with_extension("db-wal");
    if let Ok(wal_meta) = fs::metadata(&wal_path) {
        file_modified = file_modified.max(metadata_modified_nanos(&wal_meta));
    }

    let (last_modified, _) = get_sync_state(db, &db_path_str)?;
    if file_modified <= last_modified {
        result.files_scanned = 1;
        return Ok(result);
    }

    let opencode_conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .context("无法打开 opencode.db")?;

    result.files_scanned = 1;
    let mut has_sync_errors = false;

    // 查询所有会话及水位线
    let mut stmt = opencode_conn
        .prepare(
            "SELECT s.id,
                    MAX(s.time_updated, COALESCE(MAX(m.time_updated), s.time_updated)) AS sync_watermark
             FROM session s
             LEFT JOIN message m ON m.session_id = s.id
             GROUP BY s.id
             ORDER BY sync_watermark",
        )
        .context("准备会话查询失败")?;
    let sessions: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("查询会话失败")?
        .filter_map(|r| r.ok())
        .collect();

    for (session_id, time_updated) in &sessions {
        let sync_key = format!("{db_path_str}:{session_id}");
        let (sess_last_modified, _) = get_sync_state(db, &sync_key)?;
        if *time_updated <= sess_last_modified {
            continue;
        }

        let mut session_had_error = false;
        let mut session_has_incomplete = false;

        // 查询 assistant 消息
        let mut msg_stmt = opencode_conn
            .prepare("SELECT id, data FROM message WHERE session_id = ?1 ORDER BY time_created")
            .context("准备消息查询失败")?;
        let rows = msg_stmt
            .query_map([session_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .context("查询消息失败")?;

        for row in rows {
            let (message_id, data_json) = match row {
                Ok(r) => r,
                Err(_) => continue,
            };
            let value: serde_json::Value = match serde_json::from_str(&data_json) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if value.get("role").and_then(|r| r.as_str()) != Some("assistant") {
                continue;
            }
            if value.get("tokens").is_none() {
                continue;
            }
            // 跳过未完成消息
            if value.get("time").and_then(|t| t.get("completed")).is_none() {
                session_has_incomplete = true;
                continue;
            }

            let Some(msg) = parse_message_data(&value) else { continue };

            let request_id = format!("opencode_session:{session_id}:{message_id}");
            match insert_opencode_message(db, &request_id, &msg) {
                Ok(true) => result.imported += 1,
                Ok(false) => result.skipped += 1,
                Err(e) => {
                    let m = format!("OpenCode 消息插入失败 {request_id}: {e:#}");
                    log::warn!("[OPENCODE-SYNC] {m}");
                    result.errors.push(m);
                    result.skipped += 1;
                    session_had_error = true;
                }
            }
        }

        if session_had_error {
            has_sync_errors = true;
            continue;
        }
        if session_has_incomplete {
            continue;
        }
        if let Err(e) = update_sync_state(db, &sync_key, *time_updated, 0) {
            log::warn!("[OPENCODE-SYNC] 同步状态更新失败 {session_id}: {e:#}");
            has_sync_errors = true;
        }
    }

    if !has_sync_errors {
        update_sync_state(db, &db_path_str, file_modified, 0)?;
    }

    if result.imported > 0 {
        log::info!("[OPENCODE-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

struct OpenCodeMessageData {
    input_tokens: u32,
    output_tokens: u32,
    reasoning_tokens: u32,
    cache_read_tokens: u32,
    cache_write_tokens: u32,
    cost: f64,
    model_id: String,
    timestamp_ms: i64,
}

fn parse_message_data(value: &serde_json::Value) -> Option<OpenCodeMessageData> {
    let tokens = value.get("tokens")?;
    let input_tokens = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let output_tokens = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let reasoning_tokens = tokens.get("reasoning").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let cache_obj = tokens.get("cache");
    let cache_read_tokens = cache_obj.and_then(|c| c.get("read")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let cache_write_tokens = cache_obj.and_then(|c| c.get("write")).and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    if input_tokens == 0 && output_tokens == 0 && reasoning_tokens == 0
        && cache_read_tokens == 0 && cache_write_tokens == 0
    {
        return None;
    }

    let cost = value.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let model_id = value.get("modelID").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let timestamp_ms = value.get("time").and_then(|t| t.get("created")).and_then(|v| v.as_i64()).unwrap_or(0);

    Some(OpenCodeMessageData {
        input_tokens,
        output_tokens,
        reasoning_tokens,
        cache_read_tokens,
        cache_write_tokens,
        cost,
        model_id,
        timestamp_ms,
    })
}

fn insert_opencode_message(
    db: &UsageDb,
    request_id: &str,
    msg: &OpenCodeMessageData,
) -> Result<bool> {
    let conn = db.conn.lock();
    let created_at = if msg.timestamp_ms > 0 {
        msg.timestamp_ms / 1000
    } else {
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    };

    // opencode: output 含 reasoning；input 是 fresh input
    let output_with_reasoning = msg.output_tokens + msg.reasoning_tokens;
    let app_type = "opencode";
    let model_id = &msg.model_id;

    // 优先用 opencode 自带的 cost，否则定价表估算
    let (costs, total_cost) = if msg.cost > 0.0 {
        let zero = rust_decimal::Decimal::ZERO;
        let total = rust_decimal::Decimal::from_str(&msg.cost.to_string())
            .unwrap_or(rust_decimal::Decimal::ZERO);
        (
            crate::usage::pricing::CostBreakdown {
                input_cost: zero,
                output_cost: zero,
                cache_read_cost: zero,
                cache_creation_cost: zero,
                total_cost: total,
            },
            msg.cost.to_string(),
        )
    } else {
        let usage = TokenUsage {
            input_tokens: msg.input_tokens,
            output_tokens: output_with_reasoning,
            cache_read_tokens: msg.cache_read_tokens,
            cache_creation_tokens: msg.cache_write_tokens,
            model: Some(model_id.clone()),
        };
        match find_model_pricing(&conn, model_id) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(
                    &usage,
                    &pricing,
                    input_includes_cache_read(app_type),
                );
                let total = c.total_cost.to_string();
                (c, total)
            }
            None => {
                let zero = crate::usage::pricing::CostBreakdown::default();
                (zero, "0".to_string())
            }
        }
    };

    let inserted = conn.execute(
        "INSERT OR IGNORE INTO request_logs (
            request_id, provider_id, app_type, model, request_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd, total_cost_usd,
            latency_ms, status_code, session_id, is_streaming, cost_multiplier, created_at, data_source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        rusqlite::params![
            request_id,
            "_opencode_session",
            app_type,
            model_id,
            model_id,
            msg.input_tokens,
            output_with_reasoning,
            msg.cache_read_tokens,
            msg.cache_write_tokens,
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            total_cost,
            0i64,
            200i64,
            Some(request_id),
            1i64,
            "1.0",
            created_at,
            "session",
        ],
    )
    .context("插入 OpenCode 消息失败")?;
    Ok(inserted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_data_full() {
        let json = serde_json::json!({
            "role": "assistant",
            "cost": 0.0023113,
            "tokens": {
                "total": 56554,
                "input": 3272,
                "output": 383,
                "reasoning": 419,
                "cache": { "write": 0, "read": 52480 }
            },
            "modelID": "deepseek-v4-pro",
            "providerID": "deepseek",
            "time": { "created": 1779755333700i64, "completed": 1779755350639i64 }
        });
        let data = parse_message_data(&json).unwrap();
        assert_eq!(data.input_tokens, 3272);
        assert_eq!(data.output_tokens, 383);
        assert_eq!(data.reasoning_tokens, 419);
        assert_eq!(data.cache_read_tokens, 52480);
        assert!((data.cost - 0.0023113).abs() < 1e-10);
        assert_eq!(data.model_id, "deepseek-v4-pro");
    }

    #[test]
    fn test_parse_message_data_missing_cache() {
        let json = serde_json::json!({
            "role": "assistant",
            "cost": 0.0,
            "tokens": { "input": 1000, "output": 200 },
            "modelID": "mimo-v2.5-pro",
            "time": { "created": 1779755333700i64 }
        });
        let data = parse_message_data(&json).unwrap();
        assert_eq!(data.input_tokens, 1000);
        assert_eq!(data.output_tokens, 200);
        assert_eq!(data.reasoning_tokens, 0);
        assert_eq!(data.cache_read_tokens, 0);
    }

    #[test]
    fn test_parse_message_data_skips_zero_tokens() {
        let json = serde_json::json!({
            "role": "assistant",
            "tokens": { "input": 0, "output": 0, "reasoning": 0, "cache": { "read": 0, "write": 0 } },
            "modelID": "test"
        });
        assert!(parse_message_data(&json).is_none());
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test opencode` (workdir `src-tauri`)
Expected: 3 个测试通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/session_opencode.rs
git commit -m "feat(usage): opencode session sync from opencode.db"
```

---

### Task 7: Claude Code 同步（session_claude.rs）

**Files:**
- Create: `src-tauri/src/usage/session_claude.rs`

- [ ] **Step 1: 写 Claude JSONL 解析（含单测）**

```rust
use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// 同步 Claude Code 会话日志（~/.claude/projects/**/*.jsonl）
pub fn sync_claude_session_logs(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let projects_dir = &paths.claude_projects;
    let mut result = SessionSyncResult::default();
    if !projects_dir.exists() {
        return Ok(result);
    }

    let jsonl_files = collect_jsonl_files(projects_dir);
    for file_path in &jsonl_files {
        result.files_scanned += 1;
        match sync_single_file(db, file_path) {
            Ok((imported, skipped)) => {
                result.imported += imported;
                result.skipped += skipped;
            }
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                log::warn!("[CLAUDE-SYNC] 文件解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        log::info!("[CLAUDE-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

/// 收集 .jsonl：项目目录/*.jsonl + subagents/*.jsonl + subagents/workflows/wf_*/*.jsonl
fn collect_jsonl_files(projects_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(projects_dir) else { return files };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(sub_entries) = fs::read_dir(&path) {
            for sub_entry in sub_entries.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    files.push(sub_path);
                } else if sub_path.is_dir() {
                    let subagents_dir = sub_path.join("subagents");
                    if subagents_dir.is_dir() {
                        push_jsonl_children(&subagents_dir, &mut files);
                        let workflows_dir = subagents_dir.join("workflows");
                        if workflows_dir.is_dir() {
                            if let Ok(wf_entries) = fs::read_dir(&workflows_dir) {
                                for wf_entry in wf_entries.flatten() {
                                    if wf_entry.path().is_dir() {
                                        push_jsonl_children(&wf_entry.path(), &mut files);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

fn push_jsonl_children(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
}

struct ParsedAssistantUsage {
    message_id: String,
    model: String,
    input_tokens: u32,
    output_tokens: u32,
    cache_read_tokens: u32,
    cache_creation_tokens: u32,
    stop_reason: Option<String>,
    timestamp: Option<String>,
    session_id: Option<String>,
}

fn sync_single_file(db: &UsageDb, file_path: &Path) -> Result<(u32, u32)> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, last_offset) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let file = fs::File::open(file_path).context("无法打开文件")?;
    let reader = BufReader::new(file);

    let mut line_offset: i64 = 0;
    let mut messages: HashMap<String, ParsedAssistantUsage> = HashMap::new();
    let mut current_session_id: Option<String> = None;

    for line_result in reader.lines() {
        line_offset += 1;
        if line_offset <= last_offset {
            continue;
        }
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if current_session_id.is_none() {
            if let Some(sid) = value.get("sessionId").and_then(|v| v.as_str()) {
                current_session_id = Some(sid.to_string());
            }
        }
        if value.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let Some(message) = value.get("message") else { continue };
        let Some(msg_id) = message.get("id").and_then(|v| v.as_str()).map(str::to_string) else { continue };
        let Some(usage) = message.get("usage") else { continue };

        let parsed = ParsedAssistantUsage {
            message_id: msg_id.clone(),
            model: message.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
            input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            cache_read_tokens: usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            cache_creation_tokens: usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            stop_reason: message.get("stop_reason").and_then(|v| v.as_str()).map(str::to_string),
            timestamp: value.get("timestamp").and_then(|v| v.as_str()).map(str::to_string),
            session_id: current_session_id.clone(),
        };

        let should_replace = match messages.get(&msg_id) {
            None => true,
            Some(existing) => {
                if parsed.stop_reason.is_some() && existing.stop_reason.is_none() {
                    true
                } else if parsed.stop_reason.is_some() == existing.stop_reason.is_some() {
                    parsed.output_tokens > existing.output_tokens
                } else {
                    false
                }
            }
        };
        if should_replace {
            messages.insert(msg_id, parsed);
        }
    }

    let mut imported = 0u32;
    let mut skipped = 0u32;

    for msg in messages.values() {
        let has_billable = msg.input_tokens > 0 || msg.output_tokens > 0
            || msg.cache_read_tokens > 0 || msg.cache_creation_tokens > 0;
        if !has_billable {
            continue;
        }
        let request_id = format!("claude_session:{}", msg.message_id);

        match insert_claude_entry(db, &request_id, msg) {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(e) => {
                log::warn!("[CLAUDE-SYNC] 插入失败 ({}): {e:#}", msg.message_id);
                skipped += 1;
            }
        }
    }

    update_sync_state(db, &file_path_str, file_modified, line_offset)?;
    Ok((imported, skipped))
}

fn insert_claude_entry(db: &UsageDb, request_id: &str, msg: &ParsedAssistantUsage) -> Result<bool> {
    let conn = db.conn.lock();
    let created_at = msg
        .timestamp
        .as_deref()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });

    let usage = TokenUsage {
        input_tokens: msg.input_tokens,
        output_tokens: msg.output_tokens,
        cache_read_tokens: msg.cache_read_tokens,
        cache_creation_tokens: msg.cache_creation_tokens,
        model: Some(msg.model.clone()),
    };
    let (costs, total) = match find_model_pricing(&conn, &msg.model) {
        Some(pricing) => {
            let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("claude"));
            let t = c.total_cost.to_string();
            (c, t)
        }
        None => (Default::default(), "0".to_string()),
    };

    let inserted = conn.execute(
        "INSERT OR IGNORE INTO request_logs (
            request_id, provider_id, app_type, model, request_model,
            input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens,
            input_cost_usd, output_cost_usd, cache_read_cost_usd, cache_creation_cost_usd, total_cost_usd,
            latency_ms, status_code, session_id, is_streaming, cost_multiplier, created_at, data_source
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        rusqlite::params![
            request_id,
            "_claude_session",
            "claude",
            msg.model,
            msg.model,
            msg.input_tokens,
            msg.output_tokens,
            msg.cache_read_tokens,
            msg.cache_creation_tokens,
            costs.input_cost.to_string(),
            costs.output_cost.to_string(),
            costs.cache_read_cost.to_string(),
            costs.cache_creation_cost.to_string(),
            total,
            0i64,
            200i64,
            msg.session_id.as_deref(),
            1i64,
            "1.0",
            created_at,
            "session",
        ],
    )
    .context("插入 Claude 消息失败")?;
    Ok(inserted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_line() {
        let line = r#"{"type":"assistant","sessionId":"sess-1","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg-1","model":"claude-sonnet-4","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5},"stop_reason":"end_turn"}}"#;
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let usage = value.get("message").unwrap().get("usage").unwrap();
        assert_eq!(usage.get("input_tokens").unwrap().as_u64().unwrap(), 100);
        assert_eq!(usage.get("cache_read_input_tokens").unwrap().as_u64().unwrap(), 10);
        assert_eq!(value.get("sessionId").unwrap().as_str().unwrap(), "sess-1");
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test claude` (workdir `src-tauri`)
Expected: 1 个测试通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/session_claude.rs
git commit -m "feat(usage): claude code session sync from jsonl"
```

---

### Task 8: Codex 同步（session_codex.rs）

**Files:**
- Create: `src-tauri/src/usage/session_codex.rs`

- [ ] **Step 1: 写 Codex JSONL 解析（delta 计算，含单测）**

Codex 的 rollout JSONL 中 `event_msg` 携带 `total_token_usage`（累计值），需与前一条做差分。`last_token_usage` 已是单轮值直接使用。

```rust
use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// 同步 Codex 会话日志（~/.codex/sessions/**/rollout-*.jsonl）
pub fn sync_codex_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let codex_dir = &paths.codex_sessions;
    let mut result = SessionSyncResult::default();

    let mut files = Vec::new();
    let sessions_dir = codex_dir.join("sessions");
    if sessions_dir.is_dir() {
        collect_jsonl_recursive(&sessions_dir, &mut files);
    }
    let archived = codex_dir.join("archived_sessions");
    if archived.is_dir() {
        collect_jsonl_recursive(&archived, &mut files);
    }

    for file_path in &files {
        result.files_scanned += 1;
        match sync_single_file(db, file_path) {
            Ok((imported, skipped)) => {
                result.imported += imported;
                result.skipped += skipped;
            }
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                log::warn!("[CODEX-SYNC] 文件解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        log::info!("[CODEX-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

fn collect_jsonl_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_recursive(&path, files);
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
}

#[derive(Debug, Clone, Default)]
struct CumulativeTokens {
    input: u64,
    cached_input: u64,
    output: u64,
}

#[derive(Debug, Clone, Default)]
struct DeltaTokens {
    input: u32,
    cached_input: u32,
    output: u32,
}

impl DeltaTokens {
    fn is_zero(&self) -> bool {
        self.input == 0 && self.cached_input == 0 && self.output == 0
    }
}

fn sync_single_file(db: &UsageDb, file_path: &Path) -> Result<(u32, u32)> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, last_offset) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let file = fs::File::open(file_path).context("无法打开文件")?;
    let reader = BufReader::new(file);

    let mut line_offset = 0i64;
    let mut prev_total: Option<CumulativeTokens> = None;
    let mut event_index = 0u32;
    let mut imported = 0u32;
    let mut skipped = 0u32;

    // session_id = 文件名中的 UUID
    let session_id = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    for line_result in reader.lines() {
        line_offset += 1;
        if line_offset <= last_offset {
            continue;
        }
        let Ok(line) = line_result else { continue };
        if line.trim().is_empty() || !line.contains("token_count") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if value.get("type").and_then(|t| t.as_str()) != Some("event_msg") {
            continue;
        }
        let Some(payload) = value.get("payload") else { continue };
        if payload.get("type").and_then(|t| t.as_str()) != Some("token_count") {
            continue;
        }
        let Some(info) = payload.get("info").filter(|i| !i.is_null()) else { continue };
        let Some(model) = info
            .get("model")
            .or_else(|| info.get("model_name"))
            .or_else(|| payload.get("model"))
            .and_then(|v| v.as_str())
        else {
            continue;
        };

        // 取累计或单轮 token
        let (cumulative, is_total) = if let Some(total) = info.get("total_token_usage") {
            (parse_cumulative(total), true)
        } else if let Some(last) = info.get("last_token_usage") {
            (parse_cumulative(last), false)
        } else {
            continue;
        };
        let Some(cum) = cumulative else { continue };

        let delta = if is_total {
            let d = compute_delta(&prev_total, &cum);
            prev_total = Some(cum);
            d
        } else {
            DeltaTokens {
                input: cum.input as u32,
                cached_input: cum.cached_input as u32,
                output: cum.output as u32,
            }
        };
        let delta = DeltaTokens {
            cached_input: delta.cached_input.min(delta.input),
            ..delta
        };
        if delta.is_zero() {
            continue;
        }

        event_index += 1;
        let created_at = parse_timestamp_secs(value.get("timestamp"));
        let request_id = format!("codex_session:{session_id}:{event_index}");

        let usage = TokenUsage {
            input_tokens: delta.input,
            output_tokens: delta.output,
            cache_read_tokens: delta.cached_input,
            cache_creation_tokens: 0,
            model: Some(model.to_string()),
        };
        let conn = db.conn.lock();
        let (costs, total) = match find_model_pricing(&conn, model) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("codex"));
                let t = c.total_cost.to_string();
                (c, t)
            }
            None => (Default::default(), "0".to_string()),
        };
        drop(conn);

        let inserted = crate::usage::session_common::insert_request_log(
            db,
            &request_id,
            "codex",
            model,
            delta.input,
            delta.output,
            delta.cached_input,
            0,
            &costs,
            &total,
            Some(&session_id),
            created_at,
        )?;
        if inserted {
            imported += 1;
        } else {
            skipped += 1;
        }
    }

    update_sync_state(db, &file_path_str, file_modified, line_offset)?;
    Ok((imported, skipped))
}

fn parse_cumulative(info: &serde_json::Value) -> Option<CumulativeTokens> {
    Some(CumulativeTokens {
        input: info.get("input").and_then(|v| v.as_u64()).unwrap_or(0),
        cached_input: info.get("cached_input").and_then(|v| v.as_u64()).unwrap_or(0),
        output: info.get("output").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

fn compute_delta(prev: &Option<CumulativeTokens>, cur: &CumulativeTokens) -> DeltaTokens {
    match prev {
        None => DeltaTokens {
            input: cur.input as u32,
            cached_input: cur.cached_input as u32,
            output: cur.output as u32,
        },
        Some(p) => DeltaTokens {
            input: cur.input.saturating_sub(p.input) as u32,
            cached_input: cur.cached_input.saturating_sub(p.cached_input) as u32,
            output: cur.output.saturating_sub(p.output) as u32,
        },
    }
}

/// 解析 ISO8601 时间戳为 unix 秒，失败返回 0
fn parse_timestamp_secs(value: Option<&serde_json::Value>) -> i64 {
    let Some(v) = value else { return 0 };
    if let Some(n) = v.as_i64() {
        return n;
    }
    v.as_str()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_delta() {
        let prev = Some(CumulativeTokens { input: 100, cached_input: 50, output: 30 });
        let cur = CumulativeTokens { input: 150, cached_input: 60, output: 55 };
        let d = compute_delta(&prev, &cur);
        assert_eq!(d.input, 50);
        assert_eq!(d.cached_input, 10);
        assert_eq!(d.output, 25);
    }

    #[test]
    fn test_compute_delta_first() {
        let cur = CumulativeTokens { input: 100, cached_input: 0, output: 20 };
        let d = compute_delta(&None, &cur);
        assert_eq!(d.input, 100);
        assert_eq!(d.output, 20);
    }

    #[test]
    fn test_compute_delta_saturating() {
        // 文件滚动或重置导致累计值变小 → saturating 为 0，不产生负 delta
        let prev = Some(CumulativeTokens { input: 200, cached_input: 0, output: 100 });
        let cur = CumulativeTokens { input: 50, cached_input: 0, output: 30 };
        let d = compute_delta(&prev, &cur);
        assert!(d.is_zero());
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test codex` (workdir `src-tauri`)
Expected: 3 个测试通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/session_codex.rs
git commit -m "feat(usage): codex session sync with delta computation"
```

---

### Task 9: Gemini 同步（session_gemini.rs）

**Files:**
- Create: `src-tauri/src/usage/session_gemini.rs`

- [ ] **Step 1: 写 Gemini JSON 解析（单文件 JSON，含单测）**

```rust
use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 同步 Gemini 会话日志（~/.gemini/tmp/*/chats/session-*.json）
pub fn sync_gemini_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let files = collect_gemini_session_files(&paths.gemini_dir);
    let mut result = SessionSyncResult {
        files_scanned: files.len() as u32,
        ..Default::default()
    };

    for file_path in &files {
        match sync_single_gemini_file(db, file_path) {
            Ok((imported, skipped)) => {
                result.imported += imported;
                result.skipped += skipped;
            }
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                log::warn!("[GEMINI-SYNC] 解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        log::info!("[GEMINI-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

fn collect_gemini_session_files(gemini_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let tmp_dir = gemini_dir.join("tmp");
    let Ok(project_dirs) = fs::read_dir(&tmp_dir) else { return files };
    for entry in project_dirs.flatten() {
        let chats_dir = entry.path().join("chats");
        let Ok(chat_files) = fs::read_dir(&chats_dir) else { continue };
        for file_entry in chat_files.flatten() {
            let path = file_entry.path();
            let is_session = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("session-") && n.ends_with(".json"))
                .unwrap_or(false);
            if is_session {
                files.push(path);
            }
        }
    }
    files
}

fn sync_single_gemini_file(db: &UsageDb, file_path: &Path) -> Result<(u32, u32)> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, _) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok((0, 0));
    }

    let content = fs::read_to_string(file_path).context("无法读取文件")?;
    let value: serde_json::Value = serde_json::from_str(&content).context("JSON 解析失败")?;

    let session_id = value.get("sessionId").and_then(|v| v.as_str()).map(str::to_string);
    let Some(messages) = value.get("messages").and_then(|m| m.as_array()) else {
        return Ok((0, 0));
    };

    let mut imported = 0u32;
    let mut skipped = 0u32;

    for msg in messages {
        if msg.get("type").and_then(|t| t.as_str()) != Some("gemini") {
            continue;
        }
        let Some(tokens_obj) = msg.get("tokens").filter(|t| t.is_object()) else { continue };

        let input = tokens_obj.get("input").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let output = tokens_obj.get("output").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let cached = tokens_obj.get("cached").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let thoughts = tokens_obj.get("thoughts").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if input == 0 && output == 0 && cached == 0 && thoughts == 0 {
            continue;
        }

        let message_id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model = msg.get("model").and_then(|v| v.as_str()).unwrap_or("unknown");
        let timestamp = msg.get("timestamp").and_then(|v| v.as_str());

        let created_at = timestamp
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            });

        // thoughts 合并到 output 计费
        let output_tokens = output + thoughts;
        let request_id = format!("gemini_session:{}:{}", session_id.as_deref().unwrap_or("unknown"), message_id);

        let usage = TokenUsage {
            input_tokens: input,
            output_tokens,
            cache_read_tokens: cached,
            cache_creation_tokens: 0,
            model: Some(model.to_string()),
        };
        let conn = db.conn.lock();
        let (costs, total) = match find_model_pricing(&conn, model) {
            Some(pricing) => {
                let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("gemini"));
                let t = c.total_cost.to_string();
                (c, t)
            }
            None => (Default::default(), "0".to_string()),
        };
        drop(conn);

        let inserted = crate::usage::session_common::insert_request_log(
            db,
            &request_id,
            "gemini",
            model,
            input,
            output_tokens,
            cached,
            0,
            &costs,
            &total,
            session_id.as_deref(),
            created_at,
        )?;
        if inserted {
            imported += 1;
        } else {
            skipped += 1;
        }
    }

    update_sync_state(db, &file_path_str, file_modified, messages.len() as i64)?;
    Ok((imported, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_nonexistent() {
        assert!(collect_gemini_session_files(Path::new("/nonexistent")).is_empty());
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test gemini` (workdir `src-tauri`)
Expected: 1 个测试通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/session_gemini.rs
git commit -m "feat(usage): gemini session sync from json files"
```

---

### Task 10: Grok Build 同步（session_grokbuild.rs）

**Files:**
- Create: `src-tauri/src/usage/session_grokbuild.rs`

- [ ] **Step 1: 写 Grok updates.jsonl 解析（turn_completed 事件，含单测）**

```rust
use crate::usage::paths::UsagePaths;
use crate::usage::pricing::{find_model_pricing, TokenUsage, input_includes_cache_read};
use crate::usage::session_common::{
    get_sync_state, metadata_modified_nanos, update_sync_state, SessionSyncResult,
};
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 事件沉降窗：只导入早于「现在 − 2 分钟」的事件，避免与代理双算
const SETTLE_WINDOW_SECONDS: i64 = 120;

/// 同步 Grok Build 使用数据（~/.grok/{sessions,archived_sessions}/**/updates.jsonl）
pub fn sync_grokbuild_usage(db: &UsageDb, paths: &UsagePaths) -> Result<SessionSyncResult> {
    let mut files = Vec::new();
    for root in [paths.grok_dir.join("sessions"), paths.grok_dir.join("archived_sessions")] {
        collect_files_named(&root, "updates.jsonl", &mut files);
    }

    let mut result = SessionSyncResult {
        files_scanned: files.len() as u32,
        ..Default::default()
    };

    for file_path in &files {
        match sync_single_grok_file(db, file_path) {
            Ok(file_result) => result.merge(file_result),
            Err(e) => {
                let m = format!("{}: {e:#}", file_path.display());
                log::warn!("[GROK-SYNC] 解析失败: {m}");
                result.errors.push(m);
            }
        }
    }

    if result.imported > 0 {
        log::info!("[GROK-SYNC] 同步完成: 导入 {} 条", result.imported);
    }
    Ok(result)
}

fn collect_files_named(root: &Path, name: &str, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_named(&path, name, files);
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            files.push(path);
        }
    }
}

fn sync_single_grok_file(db: &UsageDb, file_path: &Path) -> Result<SessionSyncResult> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let metadata = fs::metadata(file_path).context("无法读取文件元数据")?;
    let file_modified = metadata_modified_nanos(&metadata);

    let (last_modified, _) = get_sync_state(db, &file_path_str)?;
    if file_modified <= last_modified {
        return Ok(SessionSyncResult::default());
    }

    let content = fs::read_to_string(file_path).context("无法读取文件")?;
    let events = parse_grok_usage_events(&content);

    let session_id = file_path
        .parent()
        .and_then(|d| d.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut result = SessionSyncResult::default();
    let mut deferred = false;

    for (idx, event) in events.iter().enumerate() {
        if now.saturating_sub(event.created_at) < SETTLE_WINDOW_SECONDS {
            deferred = true;
            break;
        }

        for (model, turn) in &event.per_model {
            if turn.input == 0 && turn.output == 0 && turn.cached == 0 {
                continue;
            }
            let turn_key = if event.prompt_id.is_empty() {
                format!("idx{idx}")
            } else {
                event.prompt_id.clone()
            };
            let request_id = format!("grok_session:{session_id}:{turn_key}:{model}");

            // 有 CLI 自报成本则优先使用，否则定价表估算
            let reported = if event.cost_ticks > 0 {
                rust_decimal::Decimal::from(event.cost_ticks) / rust_decimal::Decimal::from(10_000_000_000u64)
            } else {
                rust_decimal::Decimal::ZERO
            };

            let usage = TokenUsage {
                input_tokens: turn.input,
                output_tokens: turn.output,
                cache_read_tokens: turn.cached,
                cache_creation_tokens: 0,
                model: Some(model.clone()),
            };
            let conn = db.conn.lock();
            let (costs, total) = match find_model_pricing(&conn, model) {
                Some(pricing) if reported == rust_decimal::Decimal::ZERO || event.cost_partial => {
                    let c = crate::usage::pricing::calculate_cost(&usage, &pricing, input_includes_cache_read("grokbuild"));
                    let t = if reported == rust_decimal::Decimal::ZERO {
                        c.total_cost.to_string()
                    } else {
                        reported.to_string()
                    };
                    (c, t)
                }
                Some(_) => (
                    crate::usage::pricing::CostBreakdown::default(),
                    reported.to_string(),
                ),
                None => (
                    crate::usage::pricing::CostBreakdown::default(),
                    if reported == rust_decimal::Decimal::ZERO { "0".to_string() } else { reported.to_string() },
                ),
            };
            drop(conn);

            let inserted = crate::usage::session_common::insert_request_log(
                db,
                &request_id,
                "grokbuild",
                model,
                turn.input,
                turn.output,
                turn.cached,
                0,
                &costs,
                &total,
                Some(&session_id),
                event.created_at,
            )?;
            if inserted {
                result.imported += 1;
            } else {
                result.skipped += 1;
            }
        }
    }

    if deferred {
        result.deferred_files += 1;
    } else {
        update_sync_state(db, &file_path_str, file_modified, events.len() as i64)?;
    }

    Ok(result)
}

struct GrokCounters {
    input: u32,
    output: u32,
    cached: u32,
    cost_ticks: u64,
    cost_partial: bool,
}

struct GrokUsageEvent {
    created_at: i64,
    prompt_id: String,
    cost_ticks: u64,
    cost_partial: bool,
    per_model: Vec<(String, GrokCounters)>,
}

fn parse_grok_usage_events(content: &str) -> Vec<GrokUsageEvent> {
    let mut events = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if record.get("method").and_then(|v| v.as_str()) != Some("_x.ai/session/update") {
            continue;
        }
        let update = record.get("params").and_then(|p| p.get("update"));
        let kind = update.and_then(|u| u.get("sessionUpdate")).and_then(|v| v.as_str());
        if kind.is_some() && kind != Some("turn_completed") {
            continue;
        }
        let Some(usage) = update.and_then(|u| u.get("usage")).filter(|u| u.is_object()) else { continue };
        let Some(created_at) = parse_event_timestamp(record.get("timestamp")) else { continue };

        let prompt_id = update.and_then(|u| u.get("prompt_id")).and_then(|v| v.as_str()).unwrap_or("").to_string();

        let mut per_model: Vec<(String, GrokCounters)> = usage
            .get("modelUsage")
            .and_then(|m| m.as_object())
            .map(|map| {
                map.iter()
                    .map(|(model, c)| (model.clone(), parse_grok_counters(c)))
                    .collect()
            })
            .unwrap_or_default();
        if per_model.is_empty() {
            per_model.push(("unknown".to_string(), parse_grok_counters(usage)));
        }
        per_model.sort_by(|a, b| a.0.cmp(&b.0));

        events.push(GrokUsageEvent {
            created_at,
            prompt_id,
            cost_ticks: usage.get("costUsdTicks").and_then(|v| v.as_u64()).unwrap_or(0),
            cost_partial: usage.get("costIsPartial").and_then(|v| v.as_bool()).unwrap_or(false),
            per_model,
        });
    }
    events
}

fn parse_grok_counters(value: &serde_json::Value) -> GrokCounters {
    let get = |key: &str| value.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
    GrokCounters {
        input: get("inputTokens").min(u32::MAX as u64) as u32,
        output: get("outputTokens").min(u32::MAX as u64) as u32,
        cached: get("cachedReadTokens").min(u32::MAX as u64) as u32,
        cost_ticks: get("costUsdTicks"),
        cost_partial: value.get("costIsPartial").and_then(|v| v.as_bool()).unwrap_or(false),
    }
}

fn parse_event_timestamp(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(if n > 100_000_000_000 { n / 1000 } else { n });
    }
    value
        .as_str()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_EPOCH: i64 = 1_700_000_000;

    fn usage_line(epoch: i64, prompt_id: &str) -> String {
        format!(
            r#"{{"timestamp":{epoch},"method":"_x.ai/session/update","params":{{"update":{{"sessionUpdate":"turn_completed","prompt_id":"{prompt_id}","usage":{{"modelUsage":{{"grok-4.5-build":{{"inputTokens":16632,"outputTokens":104,"cachedReadTokens":0,"reasoningTokens":0,"costUsdTicks":338880000}}}}}}}}}}}}"#
        )
    }

    #[test]
    fn parses_turn_completed_ignores_noise() {
        let content = concat!(
            "not json\n",
            "{\"timestamp\":\"2026-07-20T13:26:20Z\",\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"usage_snapshot\",\"prompt_id\":\"px\",\"usage\":{\"inputTokens\":9999,\"outputTokens\":9,\"cachedReadTokens\":0}}}}\n",
            "{\"timestamp\":\"2026-07-20T13:26:24Z\",\"method\":\"_x.ai/session/update\",\"params\":{\"update\":{\"sessionUpdate\":\"turn_completed\",\"prompt_id\":\"p1\",\"usage\":{\"modelUsage\":{\"grok-4.5-build\":{\"inputTokens\":16632,\"outputTokens\":104,\"cachedReadTokens\":0,\"costUsdTicks\":338880000}}}}}}\n"
        );
        let events = parse_grok_usage_events(content);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].prompt_id, "p1");
        assert_eq!(events[0].per_model[0].0, "grok-4.5-build");
        assert_eq!(events[0].per_model[0].1.input, 16632);
        assert_eq!(events[0].cost_ticks, 338_880_000);
    }

    #[test]
    fn missing_model_usage_falls_back() {
        let line = format!(
            r#"{{"timestamp":"{OLD_EPOCH}","method":"_x.ai/session/update","params":{{"update":{{"prompt_id":"p1","usage":{{"inputTokens":100,"outputTokens":10,"cachedReadTokens":5}}}}}}}}"#
        );
        let events = parse_grok_usage_events(&line);
        assert_eq!(events[0].per_model[0].0, "unknown");
        assert_eq!(events[0].per_model[0].1.input, 100);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test grok` (workdir `src-tauri`)
Expected: 2 个测试通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/session_grokbuild.rs
git commit -m "feat(usage): grok build session sync from updates.jsonl"
```

---

### Task 11: 聚合查询（usage/stats.rs）

**Files:**
- Create: `src-tauri/src/usage/stats.rs`

- [ ] **Step 1: 写聚合查询（summary / by_app / trends / model_stats / logs）**

```rust
use crate::usage::UsageDb;
use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub request_count: u64,
    pub total_cost: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummaryByApp {
    pub app_type: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyStats {
    pub date: String,
    pub request_count: u64,
    pub total_cost: String,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub model: String,
    pub request_count: u64,
    pub total_tokens: u64,
    pub total_cost: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogRow {
    pub request_id: String,
    pub app_type: String,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub total_cost_usd: String,
    pub created_at: i64,
    pub session_id: Option<String>,
}

/// 从明细表聚合（不依赖 rollup 表，数据量小直接聚合）
pub fn get_usage_summary(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
) -> Result<UsageSummary> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    if let Some(a) = app_type {
        conditions.push("app_type = ?");
        params.push(Box::new(a.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let row = conn
        .query_row(
            &format!(
                "SELECT COUNT(*),
                        COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(cache_read_tokens), 0),
                        COALESCE(SUM(cache_creation_tokens), 0)
                 FROM request_logs {where_clause}"
            ),
            rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)),
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            },
        )
        .context("查询 usage summary 失败")?;

    let (req, cost, ti, to, cr, cc) = row;
    Ok(UsageSummary {
        request_count: req as u64,
        total_cost: format!("{:.6}", cost),
        total_input_tokens: ti as u64,
        total_output_tokens: to as u64,
        total_cache_read_tokens: cr as u64,
        total_cache_creation_tokens: cc as u64,
    })
}

pub fn get_usage_summary_by_app(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<UsageSummaryByApp>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT app_type, COUNT(*), COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                    COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0)
             FROM request_logs {where_clause}
             GROUP BY app_type ORDER BY app_type"
        ))
        .context("prepare by_app 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .context("query by_app 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (app, req, cost, tokens) = row.context("读取 by_app 行失败")?;
        out.push(UsageSummaryByApp {
            app_type: app,
            request_count: req as u64,
            total_cost: format!("{:.6}", cost),
            total_tokens: tokens as u64,
        });
    }
    Ok(out)
}

pub fn get_usage_trends(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<DailyStats>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT date(created_at, 'unixepoch', 'localtime') as day,
                    COUNT(*), COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0),
                    COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0)
             FROM request_logs {where_clause}
             GROUP BY day ORDER BY day"
        ))
        .context("prepare trends 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .context("query trends 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (date, req, cost, tokens) = row.context("读取 trends 行失败")?;
        out.push(DailyStats {
            date,
            request_count: req as u64,
            total_cost: format!("{:.6}", cost),
            total_tokens: tokens as u64,
        });
    }
    Ok(out)
}

pub fn get_model_stats(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<ModelStats>> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT model, COUNT(*),
                    COALESCE(SUM(input_tokens + output_tokens + cache_read_tokens + cache_creation_tokens), 0),
                    COALESCE(SUM(CAST(total_cost_usd AS REAL)), 0)
             FROM request_logs {where_clause}
             GROUP BY model ORDER BY SUM(CAST(total_cost_usd AS REAL)) DESC"
        ))
        .context("prepare model_stats 失败")?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })
        .context("query model_stats 失败")?;

    let mut out = Vec::new();
    for row in rows {
        let (model, req, tokens, cost) = row.context("读取 model_stats 行失败")?;
        out.push(ModelStats {
            model,
            request_count: req as u64,
            total_tokens: tokens as u64,
            total_cost: format!("{:.6}", cost),
        });
    }
    Ok(out)
}

/// 数据源状态（路径 + 是否存在）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceStatus {
    pub app_type: String,
    pub label: String,
    pub path: String,
    pub exists: bool,
}

pub fn get_usage_data_sources() -> Vec<DataSourceStatus> {
    let paths = crate::usage::paths::UsagePaths::detect();
    [
        ("opencode", "OpenCode", &paths.opencode_db),
        ("claude", "Claude Code", &paths.claude_projects),
        ("codex", "Codex", &paths.codex_sessions),
        ("gemini", "Gemini", &paths.gemini_dir),
        ("grokbuild", "Grok Build", &paths.grok_dir),
    ]
    .into_iter()
    .map(|(app, label, p)| DataSourceStatus {
        app_type: app.to_string(),
        label: label.to_string(),
        path: p.display().to_string(),
        exists: p.exists(),
    })
    .collect()
}
```

- [ ] **Step 2: 编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 通过。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/usage/stats.rs
git commit -m "feat(usage): aggregation queries for summary/trends/model stats"
```

---

### Task 12: IPC 命令（cmd_usage.rs）

**Files:**
- Create: `src-tauri/src/cmd_usage.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写 IPC 命令**

```rust
use crate::state::AppState;
use crate::usage::session_common::SessionSyncResult;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_usage_summary(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<String>,
) -> Result<crate::usage::stats::UsageSummary, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_usage_summary(&db, start_date, end_date, app_type.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_usage_summary_by_app(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::UsageSummaryByApp>, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_usage_summary_by_app(&db, start_date, end_date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_usage_trends(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::DailyStats>, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_usage_trends(&db, start_date, end_date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_model_stats(
    state: State<'_, Arc<AppState>>,
    start_date: Option<i64>,
    end_date: Option<i64>,
) -> Result<Vec<crate::usage::stats::ModelStats>, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_model_stats(&db, start_date, end_date).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_usage_data_sources() -> Vec<crate::usage::stats::DataSourceStatus> {
    crate::usage::stats::get_usage_data_sources()
}

/// 立即手动同步所有数据源
#[tauri::command]
pub async fn sync_session_usage(state: State<'_, Arc<AppState>>) -> Result<SessionSyncResult, String> {
    let db = state.usage_db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let paths = crate::usage::paths::UsagePaths::detect();
        crate::usage::sync_all(&db, &paths)
    })
    .await
    .map_err(|e| e.to_string())?;
    result.map_err(|e| e.to_string())
}

/// 模型定价管理
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricingInput {
    pub model_id: String,
    pub input_cost_per_million: String,
    pub output_cost_per_million: String,
    pub cache_read_cost_per_million: String,
    pub cache_creation_cost_per_million: String,
}

#[tauri::command]
pub fn get_model_pricing(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.usage_db.clone();
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT model_id, input_cost_per_million, output_cost_per_million,
                    cache_read_cost_per_million, cache_creation_cost_per_million
             FROM model_pricing ORDER BY model_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "modelId": r.get::<_, String>(0)?,
                "inputCostPerMillion": r.get::<_, String>(1)?,
                "outputCostPerMillion": r.get::<_, String>(2)?,
                "cacheReadCostPerMillion": r.get::<_, String>(3)?,
                "cacheCreationCostPerMillion": r.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_model_pricing_batch(
    state: State<'_, Arc<AppState>>,
    items: Vec<ModelPricingInput>,
) -> Result<(), String> {
    let db = state.usage_db.clone();
    let conn = db.conn.lock();
    for item in items {
        conn.execute(
            "INSERT INTO model_pricing (model_id, display_name,
                input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_creation_cost_per_million)
             VALUES (?1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(model_id) DO UPDATE SET
                input_cost_per_million = excluded.input_cost_per_million,
                output_cost_per_million = excluded.output_cost_per_million,
                cache_read_cost_per_million = excluded.cache_read_cost_per_million,
                cache_creation_cost_per_million = excluded.cache_creation_cost_per_million",
            rusqlite::params![
                item.model_id,
                item.input_cost_per_million,
                item.output_cost_per_million,
                item.cache_read_cost_per_million,
                item.cache_creation_cost_per_million,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

- [ ] **Step 2: 在 lib.rs 注册命令与 usage_db**

在 `src-tauri/src/lib.rs` 顶部 mod 声明加 `pub mod cmd_usage;`。

在 `AppState`（`src-tauri/src/state.rs`）加字段并初始化：

```rust
pub struct AppState {
    // ...现有字段...
    pub usage_db: std::sync::Arc<crate::usage::UsageDb>,
}
```

在 `AppState::initialize()` 中初始化：

```rust
let usage_db = crate::usage::UsageDb::open(&data_dir.join("usage.db"))?;
Ok(Self {
    // ...现有字段...
    usage_db,
})
```

在 `lib.rs` 的 `.invoke_handler(tauri::generate_handler![` 中加：

```rust
cmd_usage::get_usage_summary,
cmd_usage::get_usage_summary_by_app,
cmd_usage::get_usage_trends,
cmd_usage::get_model_stats,
cmd_usage::get_usage_data_sources,
cmd_usage::sync_session_usage,
cmd_usage::get_model_pricing,
cmd_usage::update_model_pricing_batch,
```

- [ ] **Step 3: 编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cmd_usage.rs src-tauri/src/lib.rs src-tauri/src/state.rs
git commit -m "feat(usage): IPC commands for usage stats + register in invoke_handler"
```

---

### Task 13: 后台定时同步（sync_all 入口 + 60 秒定时器）

**Files:**
- Modify: `src-tauri/src/usage/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 usage/mod.rs 加 sync_all**

在 `src-tauri/src/usage/mod.rs` 末尾追加：

```rust
use session_common::SessionSyncResult;

/// 同步全部数据源。任何单个源失败不阻塞其他源。
pub fn sync_all(db: &UsageDb, paths: &paths::UsagePaths) -> Result<SessionSyncResult> {
    let mut result = SessionSyncResult::default();
    for (name, step) in [
        ("OpenCode", session_opencode::sync_opencode_usage(db, paths)),
        ("Claude", session_claude::sync_claude_session_logs(db, paths)),
        ("Codex", session_codex::sync_codex_usage(db, paths)),
        ("Gemini", session_gemini::sync_gemini_usage(db, paths)),
        ("Grok Build", session_grokbuild::sync_grokbuild_usage(db, paths)),
    ] {
        match step {
            Ok(r) => result.merge(r),
            Err(e) => result.errors.push(format!("{name} 同步失败: {e:#}")),
        }
    }
    Ok(result)
}

/// 全局同步互斥，避免定时任务与手动同步并发
pub fn sync_mutex() -> &'static parking_lot::Mutex<()> {
    use once_cell::sync::OnceCell;
    static LOCK: OnceCell<parking_lot::Mutex<()>> = OnceCell::new();
    LOCK.get_or_init(|| parking_lot::Mutex::new(()))
}
```

- [ ] **Step 2: 在 lib.rs setup 中启动定时任务**

在 `lib.rs` 的 `.setup(move |app| { ... })` 内、proxy 启动之后添加：

```rust
// usage 统计后台同步：启动立即跑一次，之后每 60 秒增量检查
{
    let db = state.usage_db.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let _guard = crate::usage::sync_mutex().lock();
            let paths = crate::usage::paths::UsagePaths::detect();
            if let Ok(result) = crate::usage::sync_all(&db, &paths) {
                if result.imported > 0 {
                    log::info!("[USAGE-SYNC] 后台同步: 导入 {} 条", result.imported);
                }
                for e in &result.errors {
                    log::warn!("[USAGE-SYNC] {e}");
                }
            }
            drop(_guard);
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}
```

- [ ] **Step 3: 编译**

Run: `cargo check` (workdir `src-tauri`)
Expected: 通过。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/usage/mod.rs src-tauri/src/lib.rs
git commit -m "feat(usage): background sync every 60s + sync_all entry"
```

---

### Task 14: 前端 IPC 封装（ipc.ts 类型 + invoke）

**Files:**
- Modify: `src/lib/ipc.ts`

- [ ] **Step 1: 添加类型与函数**

在 `src/lib/ipc.ts` 末尾追加：

```typescript
// ---- Usage stats ----
export interface UsageSummary {
  requestCount: number;
  totalCost: string;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheCreationTokens: number;
}

export interface UsageSummaryByApp {
  appType: string;
  requestCount: number;
  totalCost: string;
  totalTokens: number;
}

export interface DailyStats {
  date: string;
  requestCount: number;
  totalCost: string;
  totalTokens: number;
}

export interface ModelStats {
  model: string;
  requestCount: number;
  totalTokens: number;
  totalCost: string;
}

export interface DataSourceStatus {
  appType: string;
  label: string;
  path: string;
  exists: boolean;
}

export interface SessionSyncResult {
  imported: number;
  skipped: number;
  filesScanned: number;
  suspectedDuplicates: number;
  deferredFiles: number;
  errors: string[];
}

export interface ModelPricingInput {
  modelId: string;
  inputCostPerMillion: string;
  outputCostPerMillion: string;
  cacheReadCostPerMillion: string;
  cacheCreationCostPerMillion: string;
}

export const getUsageSummary = (
  startDate?: number,
  endDate?: number,
  appType?: string,
) => invoke<UsageSummary>("get_usage_summary", { startDate, endDate, appType });
export const getUsageSummaryByApp = (startDate?: number, endDate?: number) =>
  invoke<UsageSummaryByApp[]>("get_usage_summary_by_app", { startDate, endDate });
export const getUsageTrends = (startDate?: number, endDate?: number) =>
  invoke<DailyStats[]>("get_usage_trends", { startDate, endDate });
export const getModelStats = (startDate?: number, endDate?: number) =>
  invoke<ModelStats[]>("get_model_stats", { startDate, endDate });
export const getUsageDataSources = () => invoke<DataSourceStatus[]>("get_usage_data_sources");
export const syncSessionUsage = () => invoke<SessionSyncResult>("sync_session_usage");
export const getModelPricing = () => invoke<ModelPricingInput[]>("get_model_pricing");
export const updateModelPricingBatch = (items: ModelPricingInput[]) =>
  invoke<void>("update_model_pricing_batch", { items });
```

- [ ] **Step 2: 类型检查**

Run: `npx tsc --noEmit` (workdir 根目录)
Expected: 无类型错误。

- [ ] **Step 3: Commit**

```bash
git add src/lib/ipc.ts
git commit -m "feat(usage): frontend IPC types and functions"
```

---

### Task 15: 前端 Usage 页面（recharts + 汇总 + 趋势 + 明细）

**Files:**
- Modify: `package.json`（加 recharts）
- Create: `src/pages/Usage/index.tsx`
- Create: `src/pages/Usage/UsageHero.tsx`
- Create: `src/pages/Usage/UsageTrendChart.tsx`
- Create: `src/pages/Usage/ModelStatsTable.tsx`
- Create: `src/pages/Usage/DataSourcesPanel.tsx`
- Modify: `src/App.tsx`（路由）
- Modify: `src/components/Layout.tsx`（导航）
- Modify: `src/i18n/zh.ts` / `src/i18n/en.ts` / `src/i18n/schema.ts`

- [ ] **Step 1: 安装 recharts**

Run: `npm install recharts` (workdir 根目录)
Expected: `recharts@^3.x` 写入 package.json。

- [ ] **Step 2: 创建页面组件**

创建 `src/pages/Usage/UsageHero.tsx`：

```tsx
import { useTranslation } from "react-i18next";
import type { UsageSummary } from "@/lib/ipc";

export function UsageHero({ summary }: { summary: UsageSummary | null }) {
  const { t } = useTranslation();
  const cost = summary ? Number(summary.totalCost) : 0;
  const tokens =
    (summary?.totalInputTokens ?? 0) +
    (summary?.totalOutputTokens ?? 0) +
    (summary?.totalCacheReadTokens ?? 0) +
    (summary?.totalCacheCreationTokens ?? 0);

  const cards = [
    { key: "cost", value: `$${cost.toFixed(4)}`, label: t("usage.total_cost") },
    { key: "tokens", value: tokens.toLocaleString(), label: t("usage.total_tokens") },
    { key: "requests", value: (summary?.requestCount ?? 0).toLocaleString(), label: t("usage.total_requests") },
  ];

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
      {cards.map((c) => (
        <div key={c.key} className="rounded-xl border border-border bg-bg-subtle p-4">
          <div className="text-2xs text-fg-muted">{c.label}</div>
          <div className="mt-1 text-2xl font-bold">{c.value}</div>
        </div>
      ))}
    </div>
  );
}
```

创建 `src/pages/Usage/UsageTrendChart.tsx`：

```tsx
import { useTranslation } from "react-i18next";
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { DailyStats } from "@/lib/ipc";

export function UsageTrendChart({ data }: { data: DailyStats[] }) {
  const { t } = useTranslation();
  const chartData = data.map((d) => ({
    date: d.date,
    tokens: d.totalTokens,
    cost: Number(d.totalCost),
  }));

  return (
    <div className="h-64 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={chartData}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
          <XAxis dataKey="date" tick={{ fontSize: 12 }} />
          <YAxis yAxisId="left" tick={{ fontSize: 12 }} tickFormatter={(v) => (v / 1000).toFixed(0) + "k"} />
          <YAxis yAxisId="right" orientation="right" tick={{ fontSize: 12 }} tickFormatter={(v) => "$" + v.toFixed(2)} />
          <Tooltip />
          <Line yAxisId="left" type="monotone" dataKey="tokens" stroke="var(--color-accent)" name={t("usage.total_tokens")} dot={false} />
          <Line yAxisId="right" type="monotone" dataKey="cost" stroke="#f59e0b" name={t("usage.total_cost")} dot={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
```

创建 `src/pages/Usage/ModelStatsTable.tsx`：

```tsx
import { useTranslation } from "react-i18next";
import type { ModelStats } from "@/lib/ipc";

export function ModelStatsTable({ data }: { data: ModelStats[] }) {
  const { t } = useTranslation();
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-border text-left text-2xs text-fg-muted">
          <th className="py-2 pr-3">{t("usage.model")}</th>
          <th className="py-2 pr-3">{t("usage.total_requests")}</th>
          <th className="py-2 pr-3">{t("usage.total_tokens")}</th>
          <th className="py-2 text-right">{t("usage.total_cost")}</th>
        </tr>
      </thead>
      <tbody>
        {data.map((m) => (
          <tr key={m.model} className="border-b border-border">
            <td className="py-2 pr-3 font-mono text-xs">{m.model}</td>
            <td className="py-2 pr-3">{m.requestCount.toLocaleString()}</td>
            <td className="py-2 pr-3">{m.totalTokens.toLocaleString()}</td>
            <td className="py-2 text-right">${Number(m.totalCost).toFixed(4)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
```

创建 `src/pages/Usage/DataSourcesPanel.tsx`：

```tsx
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { getUsageDataSources, syncSessionUsage, type DataSourceStatus, type SessionSyncResult } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function DataSourcesPanel() {
  const { t } = useTranslation();
  const [sources, setSources] = useState<DataSourceStatus[]>([]);
  const [syncing, setSyncing] = useState(false);

  const refresh = async () => {
    try {
      setSources(await getUsageDataSources());
    } catch (e) {
      toast.error(describeError(e));
    }
  };

  void refresh();

  const doSync = async () => {
    setSyncing(true);
    try {
      const r: SessionSyncResult = await syncSessionUsage();
      toast.success(
        t("usage.sync_done", { imported: r.imported, skipped: r.skipped }),
      );
      await refresh();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setSyncing(false);
    }
  };

  return (
    <div className="rounded-xl border border-border bg-bg-subtle p-4">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold">{t("usage.data_sources")}</h3>
        <button
          onClick={doSync}
          disabled={syncing}
          className="flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
        >
          <RefreshCw size={13} className={syncing ? "animate-spin" : ""} />
          {t("usage.sync_now")}
        </button>
      </div>
      <ul className="space-y-2">
        {sources.map((s) => (
          <li key={s.appType} className="flex items-center gap-2 text-xs">
            <span className={`h-2 w-2 rounded-full ${s.exists ? "bg-emerald-500" : "bg-gray-400"}`} />
            <span className="w-24 shrink-0 font-medium">{s.label}</span>
            <code className="truncate font-mono text-fg-muted">{s.path}</code>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

创建 `src/pages/Usage/index.tsx`：

```tsx
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getModelStats, getUsageSummary, getUsageSummaryByApp, getUsageTrends,
  type DailyStats, type ModelStats, type UsageSummary, type UsageSummaryByApp } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { Section } from "@/components/Section";
import { UsageHero } from "./UsageHero";
import { UsageTrendChart } from "./UsageTrendChart";
import { ModelStatsTable } from "./ModelStatsTable";
import { DataSourcesPanel } from "./DataSourcesPanel";

const RANGES = [
  { key: "7d", days: 7 },
  { key: "30d", days: 30 },
  { key: "90d", days: 90 },
  { key: "all", days: 0 },
] as const;

function rangeToEpoch(days: number): number | undefined {
  if (days === 0) return undefined;
  return Math.floor(Date.now() / 1000) - days * 86400;
}

export function Usage() {
  const { t } = useTranslation();
  const [rangeKey, setRangeKey] = useState<(typeof RANGES)[number]["key"]>("30d");
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [byApp, setByApp] = useState<UsageSummaryByApp[]>([]);
  const [trends, setTrends] = useState<DailyStats[]>([]);
  const [models, setModels] = useState<ModelStats[]>([]);
  const [error, setError] = useState<string | null>(null);

  const startDate = useMemo(() => rangeToEpoch(RANGES.find((r) => r.key === rangeKey)!.days), [rangeKey]);

  const load = useCallback(async () => {
    try {
      const [s, b, tr, m] = await Promise.all([
        getUsageSummary(startDate),
        getUsageSummaryByApp(startDate),
        getUsageTrends(startDate),
        getModelStats(startDate),
      ]);
      setSummary(s);
      setByApp(b);
      setTrends(tr);
      setModels(m);
      setError(null);
    } catch (e) {
      setError(describeError(e));
    }
  }, [startDate]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-5 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-bold">{t("nav.usage")}</h1>
        <div className="flex gap-1 rounded-lg border border-border p-0.5">
          {RANGES.map((r) => (
            <button
              key={r.key}
              onClick={() => setRangeKey(r.key)}
              className={`rounded-md px-3 py-1 text-xs ${
                rangeKey === r.key ? "bg-accent text-white" : "text-fg-muted hover:text-fg"
              }`}
            >
              {t(`usage.range.${r.key}`)}
            </button>
          ))}
        </div>
      </div>

      {error && <div className="rounded-lg bg-red-500/10 p-3 text-xs text-red-600">{error}</div>}

      <UsageHero summary={summary} />

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <Section title={t("usage.trend")} className="lg:col-span-2">
          <UsageTrendChart data={trends} />
        </Section>
        <DataSourcesPanel />
      </div>

      {byApp.length > 0 && (
        <Section title={t("usage.by_app")}>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
            {byApp.map((a) => (
              <div key={a.appType} className="rounded-xl border border-border p-3">
                <div className="text-xs font-medium">{a.appType}</div>
                <div className="mt-1 text-lg font-bold">${Number(a.totalCost).toFixed(2)}</div>
                <div className="text-2xs text-fg-muted">{a.requestCount.toLocaleString()} req</div>
              </div>
            ))}
          </div>
        </Section>
      )}

      {models.length > 0 && (
        <Section title={t("usage.by_model")}>
          <ModelStatsTable data={models} />
        </Section>
      )}
    </div>
  );
}
```

- [ ] **Step 3: 加路由与导航**

在 `src/App.tsx` 中：

```tsx
import { Usage } from "@/pages/Usage";
// 在 <Route path="/logs" element={<Logs />} /> 之后加：
<Route path="/usage" element={<Usage />} />
```

在 `src/components/Layout.tsx`：

```tsx
import { ... ChartColumn } from "lucide-react";
// items 数组加：
{ to: "/usage", key: "usage", icon: ChartColumn },
```

- [ ] **Step 4: 加 i18n**

在 `src/i18n/zh.ts` 加：

```ts
usage: {
  total_cost: "总费用",
  total_tokens: "总 Token",
  total_requests: "总请求数",
  total_input_tokens: "输入 Token",
  total_output_tokens: "输出 Token",
  model: "模型",
  trend: "用量趋势",
  by_app: "按应用",
  by_model: "按模型",
  data_sources: "数据源",
  sync_now: "立即同步",
  sync_done: "同步完成: 导入 {{imported}} 条, 跳过 {{skipped}} 条",
  range: { "7d": "近 7 天", "30d": "近 30 天", "90d": "近 90 天", all: "全部" },
},
```

在 `src/i18n/en.ts` 加对应英文：

```ts
usage: {
  total_cost: "Total Cost",
  total_tokens: "Total Tokens",
  total_requests: "Total Requests",
  total_input_tokens: "Input Tokens",
  total_output_tokens: "Output Tokens",
  model: "Model",
  trend: "Usage Trend",
  by_app: "By App",
  by_model: "By Model",
  data_sources: "Data Sources",
  sync_now: "Sync Now",
  sync_done: "Synced: {{imported}} imported, {{skipped}} skipped",
  range: { "7d": "7 days", "30d": "30 days", "90d": "90 days", all: "All" },
},
```

在 `src/i18n/schema.ts` 的 nav 类型中加 `usage: string;`（若 schema 定义 nav）。

- [ ] **Step 5: 验证构建**

Run: `npm run build` (workdir 根目录)
Expected: 构建成功无错误。

- [ ] **Step 6: Commit**

```bash
git add package.json package-lock.json src/pages/Usage src/App.tsx src/components/Layout.tsx src/i18n
git commit -m "feat(usage): frontend usage stats page with recharts"
```

---

### Task 16: 前端请求明细表（get_request_logs + RequestLogTable）

**Files:**
- Modify: `src-tauri/src/cmd_usage.rs`（加 get_request_logs 命令）
- Modify: `src/lib/ipc.ts`（类型 + invoke）
- Create: `src/pages/Usage/RequestLogTable.tsx`

- [ ] **Step 1: 后端 get_request_logs 命令**

在 `src-tauri/src/cmd_usage.rs` 添加：

```rust
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogFilters {
    pub start_date: Option<i64>,
    pub end_date: Option<i64>,
    pub app_type: Option<String>,
    pub model: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedLogs {
    pub rows: Vec<serde_json::Value>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[tauri::command]
pub fn get_request_logs(
    state: State<'_, Arc<AppState>>,
    filters: RequestLogFilters,
) -> Result<PaginatedLogs, String> {
    let db = state.usage_db.clone();
    crate::usage::stats::get_request_logs(
        &db,
        filters.start_date,
        filters.end_date,
        filters.app_type.as_deref(),
        filters.model.as_deref(),
        filters.page.unwrap_or(1).max(1),
        filters.page_size.unwrap_or(50).min(500),
    )
    .map_err(|e| e.to_string())
}
```

在 `src-tauri/src/usage/stats.rs` 添加：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedLogs {
    pub rows: Vec<RequestLogRow>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

impl From<rusqlite::Row<'_>> for RequestLogRow {
    fn from(r: rusqlite::Row) -> Self {
        Self {
            request_id: r.get(0).unwrap_or_default(),
            app_type: r.get(1).unwrap_or_default(),
            model: r.get(2).unwrap_or_default(),
            input_tokens: r.get(3).unwrap_or(0),
            output_tokens: r.get(4).unwrap_or(0),
            cache_read_tokens: r.get(5).unwrap_or(0),
            cache_creation_tokens: r.get(6).unwrap_or(0),
            total_cost_usd: r.get(7).unwrap_or_else(|_| "0".to_string()),
            created_at: r.get(8).unwrap_or(0),
            session_id: r.get(9).ok(),
        }
    }
}

pub fn get_request_logs(
    db: &UsageDb,
    start_date: Option<i64>,
    end_date: Option<i64>,
    app_type: Option<&str>,
    model: Option<&str>,
    page: u64,
    page_size: u64,
) -> Result<PaginatedLogs> {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = start_date {
        conditions.push("created_at >= ?");
        params.push(Box::new(s));
    }
    if let Some(e) = end_date {
        conditions.push("created_at <= ?");
        params.push(Box::new(e));
    }
    if let Some(a) = app_type {
        conditions.push("app_type = ?");
        params.push(Box::new(a.to_string()));
    }
    if let Some(m) = model {
        conditions.push("model = ?");
        params.push(Box::new(m.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let conn = db.conn.lock();
    let total: u64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM request_logs {where_clause}"),
            rusqlite::params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)),
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0) as u64;

    let offset = (page - 1) * page_size;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT request_id, app_type, model, input_tokens, output_tokens,
                    cache_read_tokens, cache_creation_tokens, total_cost_usd, created_at, session_id
             FROM request_logs {where_clause}
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
        ))
        .context("prepare request_logs 失败")?;

    let mut all_params: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    all_params.push(&page_size);
    all_params.push(&offset);

    let rows = stmt
        .query_map(rusqlite::params_from_iter(all_params), RequestLogRow::from)
        .context("query request_logs 失败")?
        .filter_map(Result::ok)
        .collect();

    Ok(PaginatedLogs { rows, total, page, page_size })
}
```

在 `src-tauri/src/lib.rs` 的 invoke_handler 注册 `cmd_usage::get_request_logs`。

- [ ] **Step 2: 前端类型与组件**

在 `src/lib/ipc.ts` 加：

```typescript
export interface RequestLogRow {
  requestId: string;
  appType: string;
  model: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalCostUsd: string;
  createdAt: number;
  sessionId?: string;
}

export interface PaginatedLogs {
  rows: RequestLogRow[];
  total: number;
  page: number;
  pageSize: number;
}

export interface RequestLogFilters {
  startDate?: number;
  endDate?: number;
  appType?: string;
  model?: string;
  page?: number;
  pageSize?: number;
}

export const getRequestLogs = (filters: RequestLogFilters = {}) =>
  invoke<PaginatedLogs>("get_request_logs", { filters });
```

创建 `src/pages/Usage/RequestLogTable.tsx`：

```tsx
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getRequestLogs, type PaginatedLogs } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { toast } from "sonner";

export function RequestLogTable({ startDate }: { startDate?: number }) {
  const { t } = useTranslation();
  const [data, setData] = useState<PaginatedLogs | null>(null);
  const [page, setPage] = useState(1);
  const pageSize = 20;

  const load = useCallback(async () => {
    try {
      setData(await getRequestLogs({ startDate, page, pageSize }));
    } catch (e) {
      toast.error(describeError(e));
    }
  }, [startDate, page]);

  useEffect(() => {
    void load();
  }, [load]);

  const totalPages = data ? Math.max(1, Math.ceil(data.total / pageSize)) : 1;

  return (
    <div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-2xs text-fg-muted">
            <th className="py-2 pr-3">{t("usage.time")}</th>
            <th className="py-2 pr-3">{t("usage.app")}</th>
            <th className="py-2 pr-3">{t("usage.model")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.input_tokens")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.output_tokens")}</th>
            <th className="py-2 text-right">{t("usage.total_cost")}</th>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((r) => (
            <tr key={r.requestId} className="border-b border-border">
              <td className="py-1.5 pr-3 text-xs text-fg-muted">
                {new Date(r.createdAt * 1000).toLocaleString()}
              </td>
              <td className="py-1.5 pr-3 text-xs">{r.appType}</td>
              <td className="py-1.5 pr-3 font-mono text-xs">{r.model}</td>
              <td className="py-1.5 pr-3 text-right text-xs">{r.inputTokens.toLocaleString()}</td>
              <td className="py-1.5 pr-3 text-right text-xs">{r.outputTokens.toLocaleString()}</td>
              <td className="py-1.5 text-right text-xs">${Number(r.totalCostUsd).toFixed(4)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {data && data.total > 0 && (
        <div className="mt-3 flex items-center justify-end gap-2 text-xs">
          <button
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
            className="rounded-lg border border-border px-2.5 py-1 disabled:opacity-40"
          >
            {t("usage.prev")}
          </button>
          <span className="text-fg-muted">
            {page} / {totalPages}
          </span>
          <button
            disabled={page >= totalPages}
            onClick={() => setPage((p) => p + 1)}
            className="rounded-lg border border-border px-2.5 py-1 disabled:opacity-40"
          >
            {t("usage.next")}
          </button>
        </div>
      )}
    </div>
  );
}
```

在 `src/pages/Usage/index.tsx` 中 import 并在 by_model Section 之后加：

```tsx
<Section title={t("usage.request_logs")}>
  <RequestLogTable startDate={startDate} />
</Section>
```

在 `zh.ts` / `en.ts` 加 `time`/`app`/`prev`/`next`/`request_logs` 键。

- [ ] **Step 3: 构建验证**

Run: `npm run build` (workdir 根目录)
Expected: 构建成功。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/cmd_usage.rs src-tauri/src/usage/stats.rs src-tauri/src/lib.rs src/lib/ipc.ts src/pages/Usage src/i18n
git commit -m "feat(usage): paginated request log detail table"
```

---

### Task 17: 模型定价编辑 UI（PricingPanel + Modal）

**Files:**
- Modify: `src/pages/Usage/index.tsx`
- Create: `src/pages/Usage/PricingPanel.tsx`
- Modify: `src/i18n/zh.ts` / `en.ts`

- [ ] **Step 1: 创建 PricingPanel 组件**

```tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { getModelPricing, updateModelPricingBatch, type ModelPricingInput } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function PricingPanel() {
  const { t } = useTranslation();
  const [items, setItems] = useState<ModelPricingInput[]>([]);
  const [editing, setEditing] = useState<ModelPricingInput | null>(null);
  const [saving, setSaving] = useState(false);

  const load = async () => {
    try {
      setItems(await getModelPricing());
    } catch (e) {
      toast.error(describeError(e));
    }
  };

  void load();

  const save = async () => {
    if (!editing) return;
    setSaving(true);
    try {
      await updateModelPricingBatch([editing]);
      toast.success(t("usage.pricing_saved"));
      setEditing(null);
      await load();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-2xs text-fg-muted">
            <th className="py-2 pr-3">{t("usage.model")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.input_price")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.output_price")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.cache_read_price")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.cache_creation_price")}</th>
            <th className="py-2 text-right">{t("usage.edit")}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((p) => (
            <tr key={p.modelId} className="border-b border-border">
              <td className="py-2 pr-3 font-mono text-xs">{p.modelId}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.inputCostPerMillion}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.outputCostPerMillion}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.cacheReadCostPerMillion}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.cacheCreationCostPerMillion}</td>
              <td className="py-2 text-right">
                <button
                  onClick={() => setEditing({ ...p })}
                  className="rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-overlay-soft"
                >
                  {t("usage.edit")}
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {editing && (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4" onClick={() => setEditing(null)}>
          <div className="w-full max-w-md rounded-xl bg-bg p-5 shadow-xl" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-4 text-sm font-semibold">{t("usage.edit_pricing")}</h3>
            <div className="space-y-3">
              {(
                [
                  ["inputCostPerMillion", t("usage.input_price")],
                  ["outputCostPerMillion", t("usage.output_price")],
                  ["cacheReadCostPerMillion", t("usage.cache_read_price")],
                  ["cacheCreationCostPerMillion", t("usage.cache_creation_price")],
                ] as const
              ).map(([field, label]) => (
                <div key={field}>
                  <label className="mb-1 block text-xs text-fg-muted">{label}</label>
                  <input
                    type="text"
                    value={editing[field]}
                    onChange={(e) => setEditing({ ...editing, [field]: e.target.value })}
                    className="w-full rounded-lg border border-border bg-bg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-accent"
                  />
                </div>
              ))}
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <button onClick={() => setEditing(null)} className="rounded-lg border border-border px-3 py-1.5 text-xs">
                {t("common.cancel")}
              </button>
              <button
                onClick={save}
                disabled={saving}
                className="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
              >
                {t("common.save")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 接入 index.tsx**

在 `src/pages/Usage/index.tsx` 中 import 并在 RequestLogTable 之后加：

```tsx
<Section title={t("usage.model_pricing")}>
  <PricingPanel />
</Section>
```

- [ ] **Step 3: 加 i18n 键**

`zh.ts`:
```ts
usage: {
  // ...已有键...
  request_logs: "请求明细",
  model_pricing: "模型定价",
  input_price: "输入价(每百万)",
  output_price: "输出价(每百万)",
  cache_read_price: "缓存读取价",
  cache_creation_price: "缓存写入价",
  edit: "编辑",
  edit_pricing: "编辑定价",
  pricing_saved: "定价已保存",
  time: "时间",
  app: "应用",
  prev: "上一页",
  next: "下一页",
},
```

`en.ts` 对应英文。

- [ ] **Step 4: 构建验证 + Commit**

Run: `npm run build`
Expected: 成功。

```bash
git add src/pages/Usage/PricingPanel.tsx src/pages/Usage/index.tsx src/i18n
git commit -m "feat(usage): model pricing edit UI"
```

---

### Task 18: 数据源目录覆盖配置

**Files:**
- Modify: `src-tauri/src/usage/paths.rs`
- Modify: `src-tauri/src/storage/mod.rs`（StoredSettings 加 usage_overrides 字段）
- Modify: `src-tauri/src/cmd_settings.rs`（AppSettings 暴露）
- Modify: `src/pages/Settings.tsx`（UI）

- [ ] **Step 1: StoredSettings 加 usage_overrides 字段**

在 `src-tauri/src/storage/mod.rs` 的 `StoredSettings` 中加：

```rust
/// 使用统计各数据源目录覆盖。key: opencode/claude/codex/gemini/grokbuild，value: 覆盖路径
#[serde(default)]
pub usage_overrides: std::collections::HashMap<String, String>,
```

在 `impl Default for StoredSettings` 中加 `usage_overrides: HashMap::new()`。

- [ ] **Step 2: paths.rs 应用覆盖**

在 `src-tauri/src/usage/paths.rs` 的 `detect()` 改为接受 overrides 参数：

```rust
pub fn detect() -> Self {
    // ...现有探测逻辑...
}

/// 应用覆盖配置后的路径
pub fn apply_overrides(mut self, overrides: &std::collections::HashMap<String, String>) -> Self {
    if let Some(p) = overrides.get("opencode") {
        self.opencode_db = std::path::PathBuf::from(p);
    }
    if let Some(p) = overrides.get("claude") {
        self.claude_projects = std::path::PathBuf::from(p);
    }
    if let Some(p) = overrides.get("codex") {
        self.codex_sessions = std::path::PathBuf::from(p);
    }
    if let Some(p) = overrides.get("gemini") {
        self.gemini_dir = std::path::PathBuf::from(p);
    }
    if let Some(p) = overrides.get("grokbuild") {
        self.grok_dir = std::path::PathBuf::from(p);
    }
    self
}
```

- [ ] **Step 3: 同步逻辑改用带覆盖的路径**

在 `src-tauri/src/lib.rs` 后台任务和 `cmd_usage.rs::sync_session_usage` 中，改为：

```rust
let paths = crate::usage::paths::UsagePaths::detect()
    .apply_overrides(&state.storage.load_settings()?.usage_overrides);
```

`cmd_usage.rs::get_usage_data_sources` 同样传入 overrides（从 state.storage 读取），返回路径改为覆盖后的值。

- [ ] **Step 4: cmd_settings 暴露 usage_overrides**

在 `cmd_settings.rs` 的 `AppSettings` 和 `SafeAppSettings` 加字段：

```rust
#[serde(default)]
pub usage_overrides: std::collections::HashMap<String, String>,
```

`get_settings` / `update_settings` 映射该字段。

- [ ] **Step 5: Settings 页加覆盖配置 UI**

在 `src/pages/Settings.tsx` 增加"使用统计数据源目录"区块：5 行（应用名 + 路径输入框 + 恢复默认按钮），保存时调 `updateSettings`。

- [ ] **Step 6: 编译 + 构建**

Run: `cargo check` (workdir `src-tauri`) 然后 `npm run build` (根目录)
Expected: 均通过。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/usage/paths.rs src-tauri/src/storage/mod.rs src-tauri/src/cmd_settings.rs src-tauri/src/lib.rs src-tauri/src/cmd_usage.rs src/pages/Settings.tsx src/lib/ipc.ts
git commit -m "feat(usage): data source directory override config in settings"
```

---

### Task 19: 验证与自测

**Files:**
- 无（手动验证）

- [ ] **Step 1: 全量测试**

Run: `cargo test` (workdir `src-tauri`) 和 `npm run build` (根目录)
Expected: 所有 Rust 测试通过，前端构建成功。

- [ ] **Step 2: 手动冒烟验证**

1. `cargo tauri dev` 启动应用
2. 侧边导航点"使用统计"，页面显示汇总卡片（可能初始为 0，等 60 秒后台同步或点"立即同步"）
3. 点"立即同步"，toast 显示导入条数；数据源面板 5 个灯变绿（有数据的源）
4. 切换日期范围，趋势图/模型表刷新
5. 设置页改一个数据源目录为无效路径，回到使用统计页确认该源显示不存在（灰灯）

- [ ] **Step 3: Commit（如有修复）**

```bash
git add -A
git commit -m "fix(usage): post-integration fixes from smoke test"
```

---

## 自审记录

**Spec 覆盖检查：**
- 5 数据源：Task 6-10 ✓
- 总计/按应用/按模型/趋势/请求明细分页：Task 11（聚合）+ Task 16（get_request_logs + RequestLogTable 分页）✓
- 模型定价表内置+可编辑：Task 4（seed + 计算器）+ Task 12（命令）+ Task 17（PricingPanel 编辑 UI）✓
- 目录自动探测+可覆盖：Task 3 + Task 18 ✓
- 启动同步+60s 定时+手动按钮：Task 13 + Task 15 DataSourcesPanel ✓
- 图表 recharts：Task 15 ✓

**类型一致性：** `UsageDb`/`SessionSyncResult`/`insert_request_log` 签名在各 Task 间一致；IPC camelCase 命名与 Tauri 默认一致；前端 `ModelPricingInput`/`RequestLogFilters` 与后端命令参数名对应。
