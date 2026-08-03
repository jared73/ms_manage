pub mod db;
pub mod paths;
// TODO(usage): created in later task
// pub mod pricing;
// TODO(usage): created in later task
// pub mod session_claude;
// TODO(usage): created in later task
// pub mod session_codex;
// TODO(usage): created in later task
// pub mod session_gemini;
// TODO(usage): created in later task
// pub mod session_grokbuild;
// TODO(usage): created in later task
// pub mod session_opencode;
// TODO(usage): created in later task
// pub mod stats;

use anyhow::{Context, Result};
use parking_lot::Mutex;
// TODO(usage): created in later task — PathBuf 由后续模块使用
// use std::path::PathBuf;
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
