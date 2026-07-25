use super::{atomic_write_json, StoredServer};
use anyhow::{Context, Result};
use std::path::Path;

pub fn load(path: &Path) -> Result<Vec<StoredServer>> {
    if !path.exists() { return Ok(vec![]); }
    let raw = std::fs::read_to_string(path).context("read servers.json")?;
    if raw.trim().is_empty() { return Ok(vec![]); }
    // 解析失败:不能静默回退到空 list —— 紧接着任何 save_servers 都会用空列表覆盖原文件,
    // 把用户的所有 MCP 配置永久抹掉。
    // 这里跟 storage::load_settings 一样:把损坏副本备份为 .corrupt,返回 Err 让上层(IPC)，
    // 触发 toast 让用户有机会从磁盘上找 .corrupt 副本恢复。
    serde_json::from_str(&raw).with_context(|| {
        let corrupt = path.with_extension("corrupt");
        let _ = std::fs::copy(path, &corrupt);
        eprintln!(
            "[storage] servers.json 解析失败, 已备份到 {}, 但仍向上抛错以阻止数据覆盖",
            corrupt.display()
        );
        format!("servers.json 损坏,已备份为 {} — 修复前请勿写盘", corrupt.display())
    })
}

pub fn save(path: &Path, list: &[StoredServer]) -> Result<()> {
    atomic_write_json(path, list).context("write servers.json")?;
    Ok(())
}
