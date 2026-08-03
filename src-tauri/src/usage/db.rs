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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_in_memory() -> crate::usage::UsageDb {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::usage::UsageDb { conn: parking_lot::Mutex::new(conn) }
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
