pub mod db;
pub mod paths;
pub mod pricing;
pub mod rollup;
pub mod session_common;
pub mod session_claude;
pub mod session_codex;
pub mod session_gemini;
pub mod session_grokbuild;
pub mod session_opencode;
pub mod sql_helpers;
pub mod stats;

use anyhow::{Context, Result};
use parking_lot::Mutex;
// TODO(usage): created in later task — PathBuf 由后续模块使用
// use std::path::PathBuf;
use std::sync::Arc;

/// 依次同步所有已支持的数据源，汇总结果。单个数据源失败不影响其它源。
pub fn sync_all(db: &UsageDb, paths: &paths::UsagePaths) -> Result<session_common::SessionSyncResult> {
    let mut result = session_common::SessionSyncResult::default();
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
    // 同步完成后重建日汇总，保证统计查询走 rollup
    if let Err(e) = rollup::rebuild_rollups(db) {
        result.errors.push(format!("rollup 重建失败: {e:#}"));
    }
    Ok(result)
}

/// 全局同步互斥，避免定时任务与手动同步并发
pub fn sync_mutex() -> &'static parking_lot::Mutex<()> {
    use once_cell::sync::OnceCell;
    static LOCK: OnceCell<parking_lot::Mutex<()>> = OnceCell::new();
    LOCK.get_or_init(|| parking_lot::Mutex::new(()))
}

/// usage.db 全局句柄，持有连接。所有 usage 模块共享。
pub struct UsageDb {
    pub conn: Mutex<rusqlite::Connection>,
}

impl UsageDb {
    pub fn open(path: &std::path::Path) -> Result<Arc<Self>> {
        match Self::open_inner(path) {
            Ok(db) => Ok(db),
            Err(_) => {
                // 损坏恢复：改名损坏库 + 重建空库，避免阻塞 App 启动
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let corrupt = std::path::PathBuf::from(format!("{}.corrupt-{}", path.display(), timestamp));
                let _ = std::fs::rename(path, &corrupt);
                let _ = std::fs::remove_file(std::path::PathBuf::from(format!("{}-wal", path.display())));
                let _ = std::fs::remove_file(std::path::PathBuf::from(format!("{}-shm", path.display())));
                eprintln!("[USAGE] usage.db 损坏，已重命名为 {} 并重建", corrupt.display());
                Self::open_inner(path)
            }
        }
    }

    fn open_inner(path: &std::path::Path) -> Result<Arc<Self>> {
        let conn = rusqlite::Connection::open(path)
            .with_context(|| format!("打开 usage.db 失败: {}", path.display()))?;
        let db = Arc::new(UsageDb { conn: Mutex::new(conn) });
        db::init_schema(&db)?;
        Ok(db)
    }
}
