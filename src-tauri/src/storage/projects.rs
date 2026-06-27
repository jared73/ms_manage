use super::StoredProject;
use anyhow::{Context, Result};
use std::path::Path;

pub fn load(path: &Path) -> Result<Vec<StoredProject>> {
    if !path.exists() { return Ok(Vec::new()); }
    let raw = std::fs::read_to_string(path).context("read projects.json")?;
    if raw.trim().is_empty() { return Ok(Vec::new()); }
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save(path: &Path, list: &[StoredProject]) -> Result<()> {
    let json = serde_json::to_string_pretty(list)?;
    std::fs::write(path, json).context("write projects.json")?;
    Ok(())
}
