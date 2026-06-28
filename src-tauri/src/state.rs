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
