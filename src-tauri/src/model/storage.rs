//! skill 解析结果的独立存储 —— 不写回 SKILL.md
//!
//! 文件:`data_dir/skill_analyses.json`
//! 结构:
//! ```json
//! {
//!   "version": 1,
//!   "analyses": {
//!     "<skill_id>": {
//!       "skillId": "...",
//!       "summaryZh": "...",
//!       "analyzedAt": "2026-08-29T...",
//!       "model": "deepseek-chat"
//!     }
//!   }
//! }
//! ```

use super::SkillAnalysis;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysesFile {
    pub version: u32,
    #[serde(default)]
    pub analyses: HashMap<String, SkillAnalysis>,
}

pub fn analyses_path(data_dir: &Path) -> PathBuf {
    data_dir.join("skill_analyses.json")
}

pub fn load(data_dir: &Path) -> Result<AnalysesFile> {
    let path = analyses_path(data_dir);
    if !path.exists() {
        return Ok(AnalysesFile { version: 1, analyses: HashMap::new() });
    }
    let raw = std::fs::read_to_string(&path).context("读取 skill_analyses.json")?;
    // 容错:文件可能损坏或旧版本 → 走默认,不阻断 UI
    serde_json::from_str(&raw).or_else(|e| {
        eprintln!("[model] 解析 skill_analyses.json 失败:{e}; 用空文件继续");
        Ok(AnalysesFile { version: 1, analyses: HashMap::new() })
    })
}

pub fn save(data_dir: &Path, file: &AnalysesFile) -> Result<()> {
    std::fs::create_dir_all(data_dir).context("create data dir")?;
    let path = analyses_path(data_dir);
    let raw = serde_json::to_string_pretty(file).context("序列化 analyses")?;
    std::fs::write(&path, raw).context("写 skill_analyses.json")?;
    Ok(())
}

/// 单条 upsert
pub fn upsert(data_dir: &Path, analysis: &SkillAnalysis) -> Result<()> {
    let mut file = load(data_dir)?;
    file.analyses.insert(analysis.skill_id.clone(), analysis.clone());
    save(data_dir, &file)
}

/// 批量 upsert(全局更新场景)
pub fn upsert_many(data_dir: &Path, items: &[SkillAnalysis]) -> Result<()> {
    let mut file = load(data_dir)?;
    for a in items {
        file.analyses.insert(a.skill_id.clone(), a.clone());
    }
    save(data_dir, &file)
}

pub fn get(data_dir: &Path, skill_id: &str) -> Result<Option<SkillAnalysis>> {
    Ok(load(data_dir)?.analyses.remove(skill_id))
}