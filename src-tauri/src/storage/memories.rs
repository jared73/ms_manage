//! 全局记忆文件存储与同步。
//!
//! 概念：每个「全局记忆」是一份 Markdown 主副本（存在 memories.json 内联），
//! 关联一个工具真实读取的绝对路径（target_path，如 `~/.config/opencode/AGENTS.md`）。
//! 启用（enabled）后，主副本内容通过**文件复制**同步到 target_path —— 各 AI 工具
//! 不需要知道本应用的存在，只要读自己的路径就能拿到记忆。
//!
//! 同步方向（默认单向，应用是主副本权威）：
//! - 编辑/保存/启用 → 主副本覆盖写 target_path
//! - target_path 被外部（工具/手改）改动 → 标记 drift，用户手动「拉取」或「推送」
//!
//! 安全：首次启用时若目标文件已存在且内容与主副本不同，优先**拉取外部内容**
//! 到主副本，避免覆盖用户已有的真实配置。

use super::atomic_write;
use serde::Serialize;

/// 记忆条目的默认模板（新条目 / 判断「主副本还是初始内容」用）
pub fn default_template(name: &str) -> String {
    format!("# {name}\n\n在此编写该工具的全局记忆与规则。\n")
}

/// 目标文件与主副本的同步状态（前端展示徽章用）
#[derive(Debug, Clone, Serialize)]
pub struct SyncState {
    /// 目标文件是否存在于磁盘
    pub target_exists: bool,
    /// 目标文件内容是否与主副本一致
    pub in_sync: bool,
    /// 目标文件最后修改时间（ISO8601；不存在为 null）
    pub target_mtime: Option<String>,
}

/// 探测 target_path 与主副本的同步状态。
pub fn sync_state(m: &super::StoredMemory) -> SyncState {
    let target = std::path::PathBuf::from(&m.target_path);
    if !target.exists() {
        return SyncState { target_exists: false, in_sync: false, target_mtime: None };
    }
    let external = std::fs::read_to_string(&target).unwrap_or_default();
    let mtime = target.metadata().ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0))
        .map(|dt| dt.to_rfc3339());
    SyncState {
        target_exists: true,
        in_sync: external == m.content,
        target_mtime: mtime,
    }
}

/// 把主副本内容复制到 target_path（创建父目录）。
/// 返回是否真的发生了写入（内容一致则不写，避免无谓刷新 mtime）。
pub fn sync_to_target(m: &super::StoredMemory) -> anyhow::Result<bool> {
    if !m.enabled {
        return Ok(false);
    }
    let target = std::path::PathBuf::from(&m.target_path);
    let current = std::fs::read_to_string(&target).unwrap_or_default();
    if current == m.content {
        return Ok(false);
    }
    atomic_write(&target, m.content.as_bytes())?;
    Ok(true)
}

/// 把 target_path 内容拉回主副本。返回是否发生变更。
pub fn pull_from_target(m: &mut super::StoredMemory) -> anyhow::Result<bool> {
    let target = std::path::PathBuf::from(&m.target_path);
    if !target.exists() {
        return Ok(false);
    }
    let external = std::fs::read_to_string(&target)?;
    if external == m.content {
        return Ok(false);
    }
    m.content = external;
    m.updated_at = chrono::Utc::now().to_rfc3339();
    Ok(true)
}

/// 首次启用时的安全 reconcile：
/// 目标文件已存在且内容与主副本不同 → 当主副本仍是「初始模板」（用户没编辑过）时，
/// 把外部内容拉进主副本（保留用户已有配置），然后再推送，保证两边一致。
/// 若主副本已被用户编辑过，则以主副本为准覆盖目标（应用是权威）。
pub fn enable_with_reconcile(m: &mut super::StoredMemory) -> anyhow::Result<()> {
    let target = std::path::PathBuf::from(&m.target_path);
    if target.exists() {
        let external = std::fs::read_to_string(&target).unwrap_or_default();
        if !external.is_empty() && external != m.content && is_pristine(m) {
            m.content = external;
            m.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }
    sync_to_target(m).map(|_| ())
}

/// 主副本是否仍是「初始模板」：内容为空，或等于默认模板（trim 后比较，容忍换行差异）。
fn is_pristine(m: &super::StoredMemory) -> bool {
    let c = m.content.trim();
    c.is_empty() || c == default_template(&m.name).trim()
}

/// 内置预设的「从磁盘升级」：若目标文件已存在，且主副本仍是初始模板（用户从未编辑过），
/// 则把文件真实内容读进主副本并自动启用 —— 文件本来就存在说明该工具正在使用，
/// 应直接接管当前配置，而不是用预设模板。
/// 返回是否发生变化（需要落盘）。
pub fn upgrade_preset_from_disk(m: &mut super::StoredMemory) -> bool {
    if !m.builtin || m.enabled || !is_pristine(m) {
        return false;
    }
    let target = std::path::PathBuf::from(&m.target_path);
    let Ok(content) = std::fs::read_to_string(&target) else {
        return false;
    };
    if content.is_empty() || content == m.content {
        return false;
    }
    m.content = content;
    m.enabled = true;
    m.updated_at = chrono::Utc::now().to_rfc3339();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StoredMemory;

    fn make(name: &str, content: &str, target: &std::path::Path) -> StoredMemory {
        let now = chrono::Utc::now().to_rfc3339();
        StoredMemory {
            id: name.to_string(),
            name: name.to_string(),
            description: None,
            target_path: target.to_string_lossy().to_string(),
            enabled: true,
            builtin: false,
            content: content.to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[test]
    fn sync_writes_and_reports_change() {
        let tmp = std::env::temp_dir().join(format!("msm-mem-test-{}", uuid::Uuid::new_v4()));
        let target = tmp.join("AGENTS.md");
        let m = make("t", "# hello\n", &target);
        let changed = sync_to_target(&m).unwrap();
        assert!(changed);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# hello\n");
        // 再次同步内容一致 → 不写
        let changed2 = sync_to_target(&m).unwrap();
        assert!(!changed2);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pull_brings_external_content_back() {
        let tmp = std::env::temp_dir().join(format!("msm-mem-test-{}", uuid::Uuid::new_v4()));
        let target = tmp.join("CLAUDE.md");
        std::fs::create_dir_all(&target.parent().unwrap()).unwrap();
        std::fs::write(&target, "# external content\n").unwrap();
        let mut m = make("t", "# 主副本\n", &target);
        let changed = pull_from_target(&mut m).unwrap();
        assert!(changed);
        assert_eq!(m.content, "# external content\n");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn enable_reconcile_keeps_existing_target() {
        let tmp = std::env::temp_dir().join(format!("msm-mem-test-{}", uuid::Uuid::new_v4()));
        let target = tmp.join("AGENTS.md");
        std::fs::create_dir_all(&target.parent().unwrap()).unwrap();
        std::fs::write(&target, "# 用户已有内容\n").unwrap();
        let mut m = make("t", &default_template("t"), &target);
        enable_with_reconcile(&mut m).unwrap();
        // 外部内容被拉进主副本，且目标未被覆盖
        assert_eq!(m.content, "# 用户已有内容\n");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# 用户已有内容\n");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sync_state_reports_drift() {
        let tmp = std::env::temp_dir().join(format!("msm-mem-test-{}", uuid::Uuid::new_v4()));
        let target = tmp.join("x.md");
        let m = make("t", "# 主副本\n", &target);
        let s = sync_state(&m);
        assert!(!s.target_exists);
        // 写一个不同内容 → drift
        std::fs::create_dir_all(&target.parent().unwrap()).unwrap();
        std::fs::write(&target, "# 外部改了\n").unwrap();
        let s2 = sync_state(&m);
        assert!(s2.target_exists);
        assert!(!s2.in_sync);
        assert!(s2.target_mtime.is_some());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn upgrade_preset_reads_existing_file_and_enables() {
        let tmp = std::env::temp_dir().join(format!("msm-mem-test-{}", uuid::Uuid::new_v4()));
        let target = tmp.join("AGENTS.md");
        std::fs::create_dir_all(&target.parent().unwrap()).unwrap();
        std::fs::write(&target, "# 我已有的真实内容\n").unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let mut m = StoredMemory {
            id: "opencode".to_string(),
            name: "opencode".to_string(),
            description: None,
            target_path: target.to_string_lossy().to_string(),
            enabled: false,
            builtin: true,
            content: default_template("opencode"),
            created_at: now.clone(),
            updated_at: now,
        };
        let changed = upgrade_preset_from_disk(&mut m);
        assert!(changed);
        assert_eq!(m.content, "# 我已有的真实内容\n");
        assert!(m.enabled);
        // 二次调用幂等：无变化
        assert!(!upgrade_preset_from_disk(&mut m));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn upgrade_skips_when_user_edited_or_no_file() {
        let tmp = std::env::temp_dir().join(format!("msm-mem-test-{}", uuid::Uuid::new_v4()));
        let now = chrono::Utc::now().to_rfc3339();
        // 无文件 → 不变
        let mut m = StoredMemory {
            id: "x".to_string(),
            name: "x".to_string(),
            description: None,
            target_path: tmp.join("nope.md").to_string_lossy().to_string(),
            enabled: false,
            builtin: true,
            content: default_template("x"),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        assert!(!upgrade_preset_from_disk(&mut m));
        // 用户已编辑过内容 → 即使文件存在也不覆盖
        let target = tmp.join("AGENTS.md");
        std::fs::create_dir_all(&target.parent().unwrap()).unwrap();
        std::fs::write(&target, "# 文件内容\n").unwrap();
        let mut m2 = StoredMemory {
            id: "x".to_string(),
            name: "x".to_string(),
            description: None,
            target_path: target.to_string_lossy().to_string(),
            enabled: false,
            builtin: true,
            content: "# 用户手写的主副本\n".to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        assert!(!upgrade_preset_from_disk(&mut m2));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
