use super::{atomic_write_json, StoredServer};
use anyhow::{Context, Result};
use std::path::Path;

pub fn load(path: &Path) -> Result<Vec<StoredServer>> {
    if !path.exists() { return Ok(vec![]); }
    let raw = std::fs::read_to_string(path).context("read servers.json")?;
    if raw.trim().is_empty() { return Ok(vec![]); }
    let list: Vec<StoredServer> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(list)
}

pub fn save(path: &Path, list: &[StoredServer]) -> Result<()> {
    atomic_write_json(path, list).context("write servers.json")?;
    Ok(())
}
