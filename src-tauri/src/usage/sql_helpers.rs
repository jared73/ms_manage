//! SQL 片段辅助：把存储的 input_tokens 归一化为 fresh input 语义。
//! codex/gemini/grokbuild 的 input 含 cache，需扣除；opencode/claude 不含，原样保留。

use crate::usage::pricing::CACHE_INCLUSIVE_APP_TYPES;

pub const INPUT_TOKEN_SEMANTICS_LEGACY: i64 = 0;
pub const INPUT_TOKEN_SEMANTICS_TOTAL: i64 = 1;
pub const INPUT_TOKEN_SEMANTICS_FRESH: i64 = 2;

/// 构建 SQL 表达式：返回单行归一化后的 fresh input tokens。
/// alias 传 "" 表示无前缀列名，传 "l" 等表别名生成 l.input_tokens 形式。
pub fn fresh_input_sql(alias: &str) -> String {
    let prefix = if alias.is_empty() {
        String::new()
    } else {
        format!("{alias}.")
    };
    let app_type_list = CACHE_INCLUSIVE_APP_TYPES
        .iter()
        .map(|t| format!("'{t}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CASE \
             WHEN {prefix}input_token_semantics = {FRESH} THEN {prefix}input_tokens \
             WHEN {prefix}input_token_semantics = {TOTAL} \
                  AND {prefix}input_tokens >= ({prefix}cache_read_tokens + {prefix}cache_creation_tokens) \
             THEN ({prefix}input_tokens - {prefix}cache_read_tokens - {prefix}cache_creation_tokens) \
             WHEN {prefix}input_token_semantics = {LEGACY} \
                  AND {prefix}app_type IN ({app_type_list}) \
                  AND {prefix}input_tokens >= {prefix}cache_read_tokens \
             THEN ({prefix}input_tokens - {prefix}cache_read_tokens) \
             ELSE {prefix}input_tokens END",
        FRESH = INPUT_TOKEN_SEMANTICS_FRESH,
        TOTAL = INPUT_TOKEN_SEMANTICS_TOTAL,
        LEGACY = INPUT_TOKEN_SEMANTICS_LEGACY,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE request_logs (
                request_id TEXT PRIMARY KEY,
                app_type TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
                input_token_semantics INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn fresh_input_with_alias_emits_prefixed_columns() {
        let sql = fresh_input_sql("l");
        assert!(sql.contains("l.input_tokens"));
        assert!(sql.contains("l.cache_read_tokens"));
        assert!(sql.contains("l.cache_creation_tokens"));
        assert!(sql.contains("l.input_token_semantics"));
    }

    #[test]
    fn fresh_input_without_alias_uses_bare_columns() {
        let sql = fresh_input_sql("");
        assert!(!sql.contains("."));
        assert!(sql.contains("'codex'"));
        assert!(sql.contains("'gemini'"));
        assert!(sql.contains("'grokbuild'"));
    }

    #[test]
    fn fresh_input_total_subtracts_both_caches() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO request_logs (
                request_id, app_type, input_tokens, cache_read_tokens,
                cache_creation_tokens, input_token_semantics
             ) VALUES ('total-1', 'codex', 1000, 300, 200, ?1)",
            [INPUT_TOKEN_SEMANTICS_TOTAL],
        )
        .unwrap();
        let expr = fresh_input_sql("l");
        let sql = format!("SELECT {expr} FROM request_logs l");
        let value: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(value, 500);
    }

    #[test]
    fn fresh_input_fresh_unchanged() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO request_logs (
                request_id, app_type, input_tokens, cache_read_tokens,
                cache_creation_tokens, input_token_semantics
             ) VALUES ('fresh-1', 'claude', 500, 300, 200, ?1)",
            [INPUT_TOKEN_SEMANTICS_FRESH],
        )
        .unwrap();
        let expr = fresh_input_sql("l");
        let sql = format!("SELECT {expr} FROM request_logs l");
        let value: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(value, 500);
    }

    #[test]
    fn fresh_input_legacy_subtracts_read_only() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO request_logs (
                request_id, app_type, input_tokens, cache_read_tokens
             ) VALUES ('legacy-1', 'gemini', 800, 300)",
            [],
        )
        .unwrap();
        let expr = fresh_input_sql("l");
        let sql = format!("SELECT {expr} FROM request_logs l");
        let value: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(value, 500);
    }

    #[test]
    fn fresh_input_legacy_non_cache_inclusive_unchanged() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO request_logs (
                request_id, app_type, input_tokens, cache_read_tokens
             ) VALUES ('legacy-claude', 'claude', 500, 300)",
            [],
        )
        .unwrap();
        let expr = fresh_input_sql("l");
        let sql = format!("SELECT {expr} FROM request_logs l");
        let value: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(value, 500);
    }

    #[test]
    fn fresh_input_legacy_codex_subtracts_read() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO request_logs (
                request_id, app_type, input_tokens, cache_read_tokens
             ) VALUES ('legacy-codex', 'codex', 800, 300)",
            [],
        )
        .unwrap();
        let expr = fresh_input_sql("l");
        let sql = format!("SELECT {expr} FROM request_logs l");
        let value: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(value, 500);
    }

    #[test]
    fn fresh_input_keeps_value_when_cache_exceeds_input() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO request_logs (
                request_id, app_type, input_tokens, cache_read_tokens,
                cache_creation_tokens, input_token_semantics
             ) VALUES ('total-broken', 'codex', 100, 999, 0, ?1)",
            [INPUT_TOKEN_SEMANTICS_TOTAL],
        )
        .unwrap();
        let expr = fresh_input_sql("l");
        let sql = format!("SELECT {expr} FROM request_logs l");
        let value: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(value, 100);
    }
}
