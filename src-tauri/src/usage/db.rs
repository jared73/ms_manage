use anyhow::{Context, Result};

/// schema 版本号 —— 用于 `PRAGMA user_version` 控制有序迁移。
/// 加新迁移时：在 `apply_migrations` 加一段 `if user_version < N+1 { ... }`，
/// 并把 SCHEMA_VERSION 改成 N+1。SQLite `PRAGMA user_version` 存于 header（64-bit 整数），
/// 启动时读 → 按序号跑未执行的迁移 → 写回最新版本号。
///
/// 变更日志：
/// v1: 初始 schema（request_logs / usage_daily_rollups / model_pricing / app_config / session_log_sync）
/// v2: request_logs 加 input_token_semantics 列（cache_inclusive 模型区分）
/// v3: usage_daily_rollups 加 failed_count 列（失败率统计）
/// v4: session_log_sync 加 byte_offset 列（增量同步按字节跳读，避免大文件 IO 重）
/// v5: request_logs 加 cost_source 列（session_reported / pricing_estimated，保护 opencode 自报 cost 不被 recalculate 覆盖）
/// v7: session_log_sync 加 last_processed_message_ts 列（opencode message 级增量水位线）
const SCHEMA_VERSION: i64 = 7;

pub fn init_schema(db: &crate::usage::UsageDb) -> Result<()> {
    let conn = db.conn.lock();
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        PRAGMA cache_size = -64000;
        "#,
    )
    .context("初始化 PRAGMA 失败")?;
    // mmap 在某些 Windows 版本不支持；失败静默忽略
    let _ = conn.execute_batch("PRAGMA mmap_size = 268435456;");

    conn.execute_batch(
        r#"
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
            input_token_semantics INTEGER NOT NULL DEFAULT 0,
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
            cost_source TEXT NOT NULL DEFAULT 'pricing_estimated',
            created_at INTEGER NOT NULL,
            data_source TEXT NOT NULL DEFAULT 'session'
        );
        CREATE INDEX IF NOT EXISTS idx_request_logs_created_at ON request_logs(created_at);
        CREATE INDEX IF NOT EXISTS idx_request_logs_app_type ON request_logs(app_type);
        CREATE INDEX IF NOT EXISTS idx_request_logs_model ON request_logs(model);
        CREATE INDEX IF NOT EXISTS idx_request_logs_session ON request_logs(session_id);
        -- 复合索引：过滤+排序一次命中，避免 SELECT+ORDER BY 的 filesort
        CREATE INDEX IF NOT EXISTS idx_request_logs_app_created ON request_logs(app_type, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_request_logs_model_created ON request_logs(model, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_request_logs_status_created ON request_logs(status_code, created_at DESC);

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
            failed_count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (date, app_type, model)
        );

        -- 小时粒度汇总：today/hour 视图秒级响应，避免对 request_logs 全表扫
        CREATE TABLE IF NOT EXISTS usage_hourly_rollups (
            date_hour TEXT NOT NULL,
            app_type TEXT NOT NULL,
            model TEXT NOT NULL,
            request_count INTEGER NOT NULL DEFAULT 0,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost_usd TEXT NOT NULL DEFAULT '0',
            failed_count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (date_hour, app_type, model)
        );
        CREATE INDEX IF NOT EXISTS idx_usage_hourly_app ON usage_hourly_rollups(app_type);

        CREATE TABLE IF NOT EXISTS model_pricing (
            model_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            input_cost_per_million TEXT NOT NULL,
            output_cost_per_million TEXT NOT NULL,
            cache_read_cost_per_million TEXT NOT NULL DEFAULT '0',
            cache_creation_cost_per_million TEXT NOT NULL DEFAULT '0'
        );

        CREATE TABLE IF NOT EXISTS app_config (
            app_type TEXT PRIMARY KEY,
            cost_multiplier TEXT NOT NULL DEFAULT '1.0',
            settle_window_seconds INTEGER NOT NULL DEFAULT 0,
            incomplete_tolerance_seconds INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS session_log_sync (
            file_path TEXT PRIMARY KEY,
            last_modified INTEGER NOT NULL,
            last_line_offset INTEGER NOT NULL DEFAULT 0,
            byte_offset INTEGER NOT NULL DEFAULT 0,
            last_processed_message_ts INTEGER NOT NULL DEFAULT 0,
            last_synced_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_session_log_sync_synced_at ON session_log_sync(last_synced_at);
        "#,
    )
    .context("初始化 usage.db schema 失败")?;

    // 迁移：老库补 input_token_semantics 列（默认 0=LEGACY）
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(request_logs)")?
        .query_map([], |r| r.get(1))?
        .filter_map(|c: Result<String, _>| c.ok())
        .collect();
    if !cols.iter().any(|c| c == "input_token_semantics") {
        conn.execute(
            "ALTER TABLE request_logs ADD COLUMN input_token_semantics INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .context("迁移 request_logs 增加 input_token_semantics 列失败")?;
    }

    // 迁移：rollup 表补 failed_count 列（老库无）
    let rollup_cols: Vec<String> = conn
        .prepare("PRAGMA table_info(usage_daily_rollups)")?
        .query_map([], |r| r.get(1))?
        .filter_map(|c: Result<String, _>| c.ok())
        .collect();
    if !rollup_cols.iter().any(|c| c == "failed_count") {
        conn.execute(
            "ALTER TABLE usage_daily_rollups ADD COLUMN failed_count INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .context("迁移 usage_daily_rollups 增加 failed_count 列失败")?;
    }

    // 读 user_version —— 决定要不要跑 v4/v5 迁移
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0);

    // v4: session_log_sync 加 byte_offset 列（增量同步按字节跳读）
    if current < 4 {
        let sync_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(session_log_sync)")?
            .query_map([], |r| r.get(1))?
            .filter_map(|c: Result<String, _>| c.ok())
            .collect();
        if !sync_cols.iter().any(|c| c == "byte_offset") {
            conn.execute(
                "ALTER TABLE session_log_sync ADD COLUMN byte_offset INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("迁移 session_log_sync 增加 byte_offset 列失败")?;
        }
    }

    // v5: request_logs 加 cost_source 列（session_reported / pricing_estimated）
    if current < 5 {
        let req_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(request_logs)")?
            .query_map([], |r| r.get(1))?
            .filter_map(|c: Result<String, _>| c.ok())
            .collect();
        if !req_cols.iter().any(|c| c == "cost_source") {
            conn.execute(
                "ALTER TABLE request_logs ADD COLUMN cost_source TEXT NOT NULL DEFAULT 'pricing_estimated'",
                [],
            )
            .context("迁移 request_logs 增加 cost_source 列失败")?;
        }
    }

    // v6: app_config 加 settle_window_seconds / incomplete_tolerance_seconds 列（per-app 同步窗口可配）
    if current < 6 {
        let ac_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(app_config)")?
            .query_map([], |r| r.get(1))?
            .filter_map(|c: Result<String, _>| c.ok())
            .collect();
        if !ac_cols.iter().any(|c| c == "settle_window_seconds") {
            conn.execute(
                "ALTER TABLE app_config ADD COLUMN settle_window_seconds INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("迁移 app_config 增加 settle_window_seconds 列失败")?;
        }
        if !ac_cols.iter().any(|c| c == "incomplete_tolerance_seconds") {
            conn.execute(
                "ALTER TABLE app_config ADD COLUMN incomplete_tolerance_seconds INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("迁移 app_config 增加 incomplete_tolerance_seconds 列失败")?;
        }
    }

    // v7: session_log_sync 加 last_processed_message_ts 列（opencode message 级增量水位线）
    if current < 7 {
        let sync_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(session_log_sync)")?
            .query_map([], |r| r.get(1))?
            .filter_map(|c: Result<String, _>| c.ok())
            .collect();
        if !sync_cols.iter().any(|c| c == "last_processed_message_ts") {
            conn.execute(
                "ALTER TABLE session_log_sync ADD COLUMN last_processed_message_ts INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("迁移 session_log_sync 增加 last_processed_message_ts 列失败")?;
        }
    }

    // 启动时一次性 ANALYZE 收集统计，让 query planner 选最优索引
    let _ = conn.execute_batch("PRAGMA optimize;");

    // 启动时批量写入内置默认定价（幂等）
    crate::usage::pricing::seed_default_pricing(&conn)?;

    // 末尾把 user_version 推到当前 SCHEMA_VERSION（幂等）。
    // 注意：上面 CREATE/ALTER 路径已对老库做了兼容，新装 db 的 user_version 是 0；
    // 这一步只是把版本号"登记"到位，下次新增迁移时按这个号码决定要不要跑。
    if current < SCHEMA_VERSION {
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
            .context("设置 user_version 失败")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_in_memory() -> crate::usage::UsageDb {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::usage::UsageDb { conn: parking_lot::Mutex::new(conn), recovered_backup: std::sync::Mutex::new(None) }
    }

    /// 老库（无 input_token_semantics 列）运行 init_schema 后应补列，旧行默认 LEGACY(0)
    #[test]
    fn migrates_legacy_table_adding_semantics_column() {
        let db = open_in_memory();
        {
            let conn = db.conn.lock();
            conn.execute_batch(
                "CREATE TABLE request_logs (
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
                INSERT INTO request_logs (request_id, provider_id, app_type, model, input_tokens, created_at)
                VALUES ('legacy-1', 'p', 'codex', 'm', 100, 1);",
            )
            .unwrap();
        }

        init_schema(&db).unwrap();

        {
            let conn = db.conn.lock();
            let cols: Vec<String> = conn
                .prepare("PRAGMA table_info(request_logs)")
                .unwrap()
                .query_map([], |r| r.get(1))
                .unwrap()
                .filter_map(|c: Result<String, _>| c.ok())
                .collect();
            assert!(cols.iter().any(|c| c == "input_token_semantics"));
            let semantics: i64 = conn
                .query_row(
                    "SELECT input_token_semantics FROM request_logs WHERE request_id = 'legacy-1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(semantics, crate::usage::sql_helpers::INPUT_TOKEN_SEMANTICS_LEGACY);
        }

        // 幂等：再次运行不报错
        init_schema(&db).unwrap();
    }
}
