use crate::storage::Storage;
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

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
    /// 启动时缓存的 expected token,配合 generation 支持 token 热重载。
    /// 用单个 Mutex 同时绑 (token, generation) —— 之前是 `Mutex<String> + AtomicU64`,
    /// Relaxed ordering 下 handler 可能在「gen 已 +1」与「token 已写入」之间读到旧 token,
    /// 放过旧 token 仍然可以接受,但反过来(读到新 gen 时 cached_token 还是旧值)会让
    /// 鉴权 check 在比较 generation 时使用旧 token 做 ct_eq,造成短时间误判。
    /// 把两者绑在一起写/读,锁内一次性 clone `(token, generation)`,消除这个 race。
    pub cached_token: Mutex<(String, u64)>,
    /// 使用统计 SQLite 数据库句柄
    pub usage_db: Arc<crate::usage::UsageDb>,
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
        let initial_token = storage.load_settings()?.mcp_token.unwrap_or_default();
        let usage_db = crate::usage::UsageDb::open(&data_dir.join("usage.db"))?;
        Ok(Self {
            storage, data_dir, log_dir,
            proxy_starting: Arc::new(AtomicBool::new(false)),
            cached_token: Mutex::new((initial_token, 0)),
            usage_db,
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
