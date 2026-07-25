use super::StoredProject;
use super::atomic_write_json;
use anyhow::{Context, Result};
use std::path::Path;

pub fn load(path: &Path) -> Result<Vec<StoredProject>> {
    if !path.exists() { return Ok(Vec::new()); }
    let raw = std::fs::read_to_string(path).context("read projects.json")?;
    if raw.trim().is_empty() { return Ok(Vec::new()); }
    // 解析失败 → 不能 .unwrap_or_default() —— 同 servers.rs 的考量,
    // 后续 save_projects 会把损坏文件覆盖成空,Project 配置会永久丢失。
    serde_json::from_str(&raw).with_context(|| {
        let corrupt = path.with_extension("corrupt");
        let _ = std::fs::copy(path, &corrupt);
        eprintln!(
            "[storage] projects.json 解析失败, 已备份到 {}, 但仍向上抛错以阻止数据覆盖",
            corrupt.display()
        );
        format!("projects.json 损坏,已备份为 {} — 修复前请勿写盘", corrupt.display())
    })
}

pub fn save(path: &Path, list: &[StoredProject]) -> Result<()> {
    atomic_write_json(path, list).context("write projects.json")?;
    Ok(())
}
