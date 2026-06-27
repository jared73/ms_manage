use crate::storage::Storage;
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::Arc;

static CURRENT: OnceCell<Arc<AppState>> = OnceCell::new();

pub struct AppState {
    pub storage: Arc<Storage>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppState {
    pub fn initialize() -> Result<Self> {
        // 一次性数据迁移：旧目录 com.aimanage.desktop / ai-manage → 新目录 com.msmanage.desktop / ms-manage
        // 跑过就在 %LOCALAPPDATA%\ms-manage\.migrated 留标记，避免重复跑
        run_one_time_migration();

        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ms-manage");
        std::fs::create_dir_all(&data_dir)?;
        let log_dir = data_dir.join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let storage = Arc::new(Storage::new(data_dir.clone())?);
        Ok(Self { storage, data_dir, log_dir })
    }

    pub fn install_as_current(self: Arc<Self>) {
        let _ = CURRENT.set(self);
    }

    pub fn get_settings(&self) -> Result<crate::cmd_settings::AppSettings> {
        Ok(self.storage.load_settings()?.into())
    }
}

pub fn try_current() -> Option<Arc<AppState>> {
    CURRENT.get().cloned()
}

/// 旧版本（ai-manage / com.aimanage.desktop）→ 新版本（ms-manage / com.msmanage.desktop）
/// 跑过一次就写标记文件，避免每次启动都搬一遍（也防止覆盖用户后续的修改）
fn run_one_time_migration() {
    use std::fs;

    let local = match dirs::data_local_dir() {
        Some(p) => p,
        None => return,
    };

    let new_dir = local.join("ms-manage");
    let flag = new_dir.join(".migrated-from-aimanage");
    if flag.exists() {
        return;
    }

    // 尝试建新目录（如果不存在）作为搬运动作的目标
    if let Err(e) = fs::create_dir_all(&new_dir) {
        eprintln!("[ms-manage] migration: cannot create {:?}: {e}", new_dir);
        return;
    }

    // 候选旧位置（按优先级：先 com.aimanage.desktop，再 ai-manage）
    let candidates = [
        local.join("com.aimanage.desktop"),
        local.join("ai-manage"),
    ];

    let mut moved_any = false;
    for old in &candidates {
        if !old.exists() { continue; }
        match move_dir_contents(old, &new_dir) {
            Ok(n) => {
                eprintln!("[ms-manage] migration: moved {} entries from {:?} → {:?}", n, old, new_dir);
                moved_any = true;
            }
            Err(e) => eprintln!("[ms-manage] migration: move {:?} → {:?} failed: {e}", old, new_dir),
        }
    }

    if moved_any {
        // 写标记，下次启动跳过
        let _ = fs::write(&flag, b"migrated at ");
    }
}

/// 把 src 目录里所有顶层条目搬到 dest 目录（不递归覆盖已有同名条目）
fn move_dir_contents(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<usize> {
    use std::fs;
    let mut count = 0;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let from = entry.path();
        let to = dest.join(&name);

        // 跳过已经在目标里存在的（用户后来手动加的，保留）
        if to.exists() {
            continue;
        }
        // 先 try rename（快、原子），失败再 copy + remove
        match fs::rename(&from, &to) {
            Ok(()) => count += 1,
            Err(_) => {
                if from.is_dir() {
                    copy_dir_all(&from, &to)?;
                    fs::remove_dir_all(&from)?;
                } else {
                    fs::copy(&from, &to)?;
                    fs::remove_file(&from)?;
                }
                count += 1;
            }
        }
    }
    Ok(count)
}

fn copy_dir_all(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    use std::fs;
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
