use crate::storage::Storage;
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};

static CURRENT: OnceCell<Arc<AppState>> = OnceCell::new();

pub struct AppState {
    pub storage: Arc<Storage>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    /// proxy::start() 的并发互斥。
    /// 之前的实现只查 proxy_stats.running + 后续 spawn 是非原子的:
    /// 两个调用同时通过 running==false 检查、各自 spawn bind 同一端口,
    /// 第二个 bind 失败但第一个任务照跑,导致 stats 显示未运行、实际上 listen 已挂。
    /// 用 compare_exchange 保证只有首个调用者能进入临界区,
    /// 败者直接 bail("代理正在启动")。
    pub proxy_starting: Arc<AtomicBool>,
}

impl AppState {
    pub fn initialize() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ms-manage");
        std::fs::create_dir_all(&data_dir)?;
        let log_dir = data_dir.join("logs");
        std::fs::create_dir_all(&log_dir)?;
        let storage = Arc::new(Storage::new(data_dir.clone())?);
        Ok(Self {
            storage, data_dir, log_dir,
            proxy_starting: Arc::new(AtomicBool::new(false)),
        })
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
