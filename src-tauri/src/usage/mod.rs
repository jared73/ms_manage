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
use std::sync::Arc;

/// 依次同步所有已支持的数据源，汇总结果。单个数据源失败不影响其它源。
/// 同步完成后，若有新数据导入（imported > 0），走增量版 rollup 重建；
/// 改价重算场景走全量版（`rollup::rebuild_rollups`）。
pub fn sync_all(db: &UsageDb, paths: &paths::UsagePaths) -> Result<session_common::SessionSyncResult> {
    // 记录本次同步前 session_log_sync 的最大 last_synced_at，作为增量 rebuild 的下界。
    // 注：实际受影响的 date 集合可能更广（session_log 回填历史 created_at），
    // 增量函数用 `since_ts - 86400` 留出一天 buffer，但仍可能漏掉更深历史桶。
    // 现阶段保守做法：本次同步前用 last_synced_at_min 作为下界，未匹配上的极小概率 case 由下一次 sync 兜底。
    let since_ts = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT COALESCE(MAX(last_synced_at), 0) FROM session_log_sync",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
    };

    let mut result = session_common::SessionSyncResult::default();
    for (name, step) in [
        ("OpenCode", session_opencode::sync_opencode_usage(db, paths, since_ts)),
        ("Claude", session_claude::sync_claude_session_logs(db, paths)),
        ("Codex", session_codex::sync_codex_usage(db, paths)),
        ("Gemini", session_gemini::sync_gemini_usage(db, paths)),
        ("Grok Build", session_grokbuild::sync_grokbuild_usage(db, paths)),
    ] {
        match step {
            Ok(r) => {
                eprintln!(
                    "[USAGE-SYNC] {name} done: imported={} skipped={}",
                    r.imported, r.skipped
                );
                result.merge(r);
            }
            Err(e) => {
                eprintln!("[USAGE-SYNC] {name} 失败: {e:#}");
                result.errors.push(format!("{name} 同步失败: {e:#}"));
            }
        }
    }

    // 同步完成后重建 rollup（修复后 always 跑：之前 if imported>0 短路会让"sync 写 request_logs 但
    // mtime 没变 → imported=0 → rebuild 不跑"场景漏掉老 hour/daily 桶。daily 76 行 / hourly 248 行，
    // 重 rebuild 几毫秒可接受）。
    eprintln!(
        "[USAGE-SYNC] sync_all done: imported={} errors={}",
        result.imported,
        result.errors.len()
    );
    if let Err(e) = rollup::rebuild_rollups_incremental(db, since_ts) {
        result.errors.push(format!("rollup 增量重建失败: {e:#}"));
    }
    if let Err(e) = rollup::rebuild_hourly_rollups_incremental(db, since_ts) {
        result.errors.push(format!("hourly rollup 增量重建失败: {e:#}"));
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
    /// 若启动时发生过损坏恢复,存损坏文件的备份路径;前端启动时查询一次并 toast 告知用户。
    pub recovered_backup: std::sync::Mutex<Option<std::path::PathBuf>>,
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
                let db = Self::open_inner(path)?;
                if let Ok(mut guard) = db.recovered_backup.lock() {
                    *guard = Some(corrupt);
                }
                Ok(db)
            }
        }
    }

    fn open_inner(path: &std::path::Path) -> Result<Arc<Self>> {
        let conn = rusqlite::Connection::open(path)
            .with_context(|| format!("打开 usage.db 失败: {}", path.display()))?;
        let db = Arc::new(UsageDb {
            conn: Mutex::new(conn),
            recovered_backup: std::sync::Mutex::new(None),
        });
        db::init_schema(&db)?;
        Ok(db)
    }
}
