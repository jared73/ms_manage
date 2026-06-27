//! Skill 存储 —— 目录制：每个 skill 是一个子目录，里面有 SKILL.md。
//!
//! 目录结构：
//! ```
//! skills/
//!   code-review/
//!     SKILL.md
//!     (可选) helper.py / template.json 等附件
//! ```
//!
//! symlink 目标（默认）：
//! - Windows: %USERPROFILE%\.claude\skills
//! - Linux:   ~/.claude/skills
//! - macOS:   ~/.claude/skills
//!
//! 启用 skill → 在每个目标目录下创建 symlink / junction
//! 禁用 skill → 移除 symlink / junction
//! 启动时 → reconcile：根据文件系统状态自愈

use super::{SkillTarget, StoredSkill};
use crate::symlink;
use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const SKILL_FILE: &str = "SKILL.md";

/// 默认 symlink 目标（agent skills 目录）
pub fn default_symlink_targets() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join(".claude").join("skills"),
        home.join(".codex").join("skills"),
        home.join(".copilot").join("skills"),
    ]
}

/// 把老的单 `.md` 文件迁移到目录制
pub fn migrate_legacy_flat_files(dir: &Path) -> Result<()> {
    if !dir.exists() { return Ok(()); }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() { continue; }
        if path.extension().and_then(|s| s.to_str()) != Some("md") { continue; }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if stem.is_empty() || stem.starts_with('.') { continue; }
        let new_dir = dir.join(&stem);
        if new_dir.exists() { continue; }
        std::fs::create_dir_all(&new_dir)?;
        let dest = new_dir.join(SKILL_FILE);
        std::fs::rename(&path, &dest).ok();
    }
    Ok(())
}

/// 扫描 skills/ 下的所有 skill 目录
pub fn load_all(dir: &Path) -> Result<Vec<StoredSkill>> {
    if !dir.exists() { return Ok(Vec::new()); }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        // 跳过以 . 开头的目录
        if path.file_name().and_then(|s| s.to_str()).map(|s| s.starts_with('.')).unwrap_or(true) {
            continue;
        }
        match parse_dir(&path) {
            Ok(s) => out.push(s),
            Err(e) => eprintln!("skill parse {:?}: {e}", path),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn parse_dir(path: &Path) -> Result<StoredSkill> {
    let skill_file = path.join(SKILL_FILE);
    if !skill_file.exists() {
        anyhow::bail!("no SKILL.md in {:?}", path);
    }
    let raw = std::fs::read_to_string(&skill_file).context("read SKILL.md")?;
    let (front, body) = split_frontmatter(&raw);
    let fm: serde_yaml::Mapping = serde_yaml::from_str(&front).unwrap_or_default();
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("skill").to_string();
    let display_name = fm.get("name").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
    let description = fm.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
    let id = fm.get("id").and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.clone());
    let enabled = fm.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let mut frontmatter: HashMap<String, serde_yaml::Value> = HashMap::new();
    for (k, v) in fm {
        let key = match k {
            serde_yaml::Value::String(s) => s,
            other => format!("{:?}", other),
        };
        frontmatter.insert(key, v);
    }
    let meta = std::fs::metadata(&skill_file)?;
    let mtime = meta.modified().ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0))
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default();
    Ok(StoredSkill {
        id,
        name: display_name,
        description,
        content: body.to_string(),
        frontmatter,
        dir_path: path.to_string_lossy().to_string(),
        enabled,
        created_at: mtime.clone(),
        updated_at: mtime,
    })
}

pub fn split_frontmatter(raw: &str) -> (String, String) {
    if let Some(stripped) = raw.strip_prefix("---") {
        if let Some(end) = stripped.find("\n---") {
            let front = stripped[..end].trim().to_string();
            let rest = &stripped[end + 4..];
            let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();
            return (front, body);
        }
    }
    ("".to_string(), raw.to_string())
}

/// 保存 skill（写到 skills/<name>/SKILL.md）
pub fn save(dir: &Path, skill: &StoredSkill) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let skill_dir = dir.join(sanitize(&skill.id));
    std::fs::create_dir_all(&skill_dir)?;
    let file = skill_dir.join(SKILL_FILE);

    let mut front = serde_yaml::Mapping::new();
    front.insert("id".into(), serde_yaml::Value::String(skill.id.clone()));
    front.insert("name".into(), serde_yaml::Value::String(skill.name.clone()));
    if let Some(d) = &skill.description {
        front.insert("description".into(), serde_yaml::Value::String(d.clone()));
    }
    front.insert("enabled".into(), serde_yaml::Value::Bool(skill.enabled));
    for (k, v) in &skill.frontmatter {
        if k == "id" || k == "name" || k == "description" || k == "enabled" { continue; }
        front.insert(serde_yaml::Value::String(k.clone()), v.clone());
    }
    let yaml = serde_yaml::to_string(&front)?;
    let content = format!("---\n{}---\n\n{}", yaml, skill.content);
    std::fs::write(&file, content).context("write SKILL.md")?;
    Ok(())
}

pub fn delete(dir: &Path, id: &str) -> Result<()> {
    let skill_dir = dir.join(sanitize(id));
    if skill_dir.exists() {
        std::fs::remove_dir_all(&skill_dir).context("delete skill dir")?;
    }
    Ok(())
}

/// 同步 symlink：让所有 enabled skill 在每个 target 目录下都有 link；
/// 不存在源目录的 link 自动清除。
/// 只对 enabled 的 target 起作用，关闭的 target 跳过。
pub fn reconcile_symlinks(skills: &[StoredSkill], targets: &[SkillTarget]) -> Result<()> {
    // 收集所有 enabled skill 的 (源目录绝对路径, link 名)
    let active: Vec<(PathBuf, String)> = skills.iter()
        .filter(|s| s.enabled)
        .filter_map(|s| {
            let src = PathBuf::from(&s.dir_path);
            if !src.exists() { return None; }
            let src_abs = std::fs::canonicalize(&src).ok().unwrap_or(src);
            Some((src_abs, sanitize(&s.id)))
        })
        .collect();

    for target in targets {
        // 关闭的 target 跳过
        if !target.enabled { continue; }
        let target_path = PathBuf::from(&target.path);
        // 跳过不存在的目标目录（避免无谓创建）
        if !target_path.exists() {
            continue;
        }
        let entries: Vec<(String, PathBuf)> = match std::fs::read_dir(&target_path) {
            Ok(rd) => rd.filter_map(|e| e.ok()).filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let p = e.path();
                if symlink::is_link(&p) { Some((name, p)) } else { None }
            }).collect(),
            Err(_) => continue,
        };
        // 删除孤儿 link（没有对应 enabled skill）
        for (name, path) in &entries {
            if !active.iter().any(|(_, link_name)| link_name == name) {
                let _ = symlink::remove_link(path);
            }
        }
        // 创建缺失的 link
        for (src, link_name) in &active {
            let link = target_path.join(link_name);
            if link.exists() || symlink::is_link(&link) {
                // 已存在则验证指向是否一致（不一致则重建）
                if let Ok(current) = std::fs::read_link(&link) {
                    if &current == src { continue; }
                    let _ = symlink::remove_link(&link);
                }
            }
            if let Err(e) = symlink::create_dir_link(&link, src) {
                eprintln!("[skills] create link {:?} -> {:?} failed: {e}", link, src);
            }
        }
    }
    Ok(())
}

/// 包装 Storage 上的 reconcile 入口（处理锁）
pub fn reconcile_after_change(storage: &super::Storage) -> Result<()> {
    let skills = storage.load_skills()?;
    let settings = storage.load_settings()?;
    let targets: Vec<SkillTarget> = settings.skill_targets;
    reconcile_symlinks(&skills, &targets)
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// （旧版 API，保留兼容）
pub fn set_targets(targets_lock: &Arc<RwLock<Vec<PathBuf>>>, new_targets: Vec<PathBuf>) {
    *targets_lock.write() = new_targets;
}
