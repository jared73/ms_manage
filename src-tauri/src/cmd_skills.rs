use crate::state::AppState;
use crate::storage::StoredSkill;
use chrono::Utc;
use std::sync::Arc;
use tauri::State;

fn to_stored(s: serde_json::Value) -> Result<StoredSkill, String> {
    serde_json::from_value(s).map_err(|e| e.to_string())
}

fn from_stored(s: &StoredSkill) -> serde_json::Value {
    serde_json::to_value(s).unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
pub fn list_skills(state: State<'_, Arc<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let list = state.storage.load_skills().map_err(|e| e.to_string())?;
    Ok(list.iter().map(from_stored).collect())
}

#[tauri::command]
pub fn get_skill(id: String, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let list = state.storage.load_skills().map_err(|e| e.to_string())?;
    let s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    Ok(from_stored(&s))
}

#[tauri::command]
pub fn upsert_skill(skill: serde_json::Value, state: State<'_, Arc<AppState>>) -> Result<serde_json::Value, String> {
    let mut s: StoredSkill = to_stored(skill)?;
    if s.id.is_empty() { s.id = uuid::Uuid::new_v4().to_string(); }
    let now = Utc::now().to_rfc3339();
    if s.created_at.is_empty() { s.created_at = now.clone(); }
    s.updated_at = now;
    state.storage.save_skill(&s).map_err(|e| e.to_string())?;
    // 保存后立即 reconcile symlink
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(from_stored(&s))
}

#[tauri::command]
pub fn delete_skill(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.storage.delete_skill(&id).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

#[tauri::command]
pub fn toggle_skill(id: String, enabled: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let list = state.storage.load_skills().map_err(|e| e.to_string())?;
    let mut s = list.into_iter().find(|s| s.id == id).ok_or_else(|| "not found".to_string())?;
    s.enabled = enabled;
    s.updated_at = Utc::now().to_rfc3339();
    state.storage.save_skill(&s).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

// ---- Skill integration targets (mcp-router 风格：每个编辑器一个目标) ----

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillTargetDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub builtin: bool,
}

#[tauri::command]
pub fn list_skill_targets(state: State<'_, Arc<AppState>>) -> Result<Vec<SkillTargetDto>, String> {
    let settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    Ok(settings.skill_targets.into_iter().map(|t| SkillTargetDto {
        id: t.id, name: t.name, path: t.path, enabled: t.enabled, builtin: t.builtin,
    }).collect())
}

#[tauri::command]
pub fn toggle_skill_target(id: String, enabled: bool, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let mut found = false;
    for t in settings.skill_targets.iter_mut() {
        if t.id == id { t.enabled = enabled; found = true; break; }
    }
    if !found { return Err(format!("target {id} not found")); }
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

#[tauri::command]
pub fn add_skill_target(name: String, path: String, state: State<'_, Arc<AppState>>) -> Result<SkillTargetDto, String> {
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let id = format!("custom-{}", uuid::Uuid::new_v4());
    let target = crate::storage::SkillTarget {
        id: id.clone(),
        name,
        path,
        enabled: true,
        builtin: false,
    };
    settings.skill_targets.push(target.clone());
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(SkillTargetDto { id: target.id, name: target.name, path: target.path, enabled: target.enabled, builtin: target.builtin })
}

#[tauri::command]
pub fn remove_skill_target(id: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let before = settings.skill_targets.len();
    settings.skill_targets.retain(|t| t.id != id);
    if settings.skill_targets.len() == before {
        return Err(format!("target {id} not found"));
    }
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

#[tauri::command]
pub fn update_skill_target(id: String, name: Option<String>, path: Option<String>, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let mut found = false;
    for t in settings.skill_targets.iter_mut() {
        if t.id == id {
            if let Some(n) = name { t.name = n; }
            if let Some(p) = path { t.path = p; }
            found = true;
            break;
        }
    }
    if !found { return Err(format!("target {id} not found")); }
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

// ---- Skill 导入：支持 .zip / .tar.gz 压缩包，或拖入文件夹 ----

/// 导入结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportResult {
    pub imported: Vec<String>,        // 成功导入的 skill 名称列表
    pub errors: Vec<String>,          // 失败描述
    pub skipped: Vec<String>,         // 跳过的原因（顶层没有 SKILL.md 等）
}

/// 从给定路径导入 skill。
/// 路径可以是：
///   - .zip 压缩包（含 SKILL.md 的文件夹压缩）
///   - .tar.gz 同上
///   - 文件夹路径（含 SKILL.md）
/// 压缩包/文件夹顶层可能含多个 skill 子目录，逐个识别导入。
#[tauri::command]
pub async fn import_skill_from_path(path: String, state: State<'_, Arc<AppState>>) -> Result<ImportResult, String> {
    let skills_dir = state.storage.skills_dir();
    std::fs::create_dir_all(&skills_dir).map_err(|e| format!("create skills dir: {e}"))?;

    let mut result = ImportResult { imported: vec![], errors: vec![], skipped: vec![] };
    let p = std::path::PathBuf::from(&path);

    if !p.exists() {
        return Err(format!("路径不存在: {path}"));
    }

    let lower_name = p.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let extracted_dirs: Vec<std::path::PathBuf> = if lower_name.ends_with(".zip") {
        extract_zip_to_temp(&p).map_err(|e| format!("解压 zip 失败: {e}"))?
    } else if lower_name.ends_with(".tar.gz") || lower_name.ends_with(".tgz") {
        extract_tar_gz_to_temp(&p).map_err(|e| format!("解压 tar.gz 失败: {e}"))?
    } else if p.is_dir() {
        vec![p.clone()]
    } else {
        return Err(format!("不支持的文件类型: {path}（仅支持 .zip / .tar.gz / 文件夹）"));
    };

    // 扫描：每个 extracted dir 里找 SKILL.md，可能在根或一级子目录
    for dir in &extracted_dirs {
        let skill_dirs = find_skill_dirs(dir);
        if skill_dirs.is_empty() {
            result.skipped.push(format!("{:?} 里没找到 SKILL.md", dir));
            continue;
        }
        for sd in skill_dirs {
            let name_hint = sd.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            match install_skill_dir(sd, &skills_dir) {
                Ok(name) => result.imported.push(name),
                Err(e) => result.errors.push(format!("{name_hint}: {e}")),
            }
        }
    }

    // reconcile symlinks
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(result)
}

fn extract_zip_to_temp(zip_path: &std::path::Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let tmp = std::env::temp_dir().join(format!("msmanage-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = match entry.enclosed_name() {
            Some(p) => tmp.join(p),
            None => continue,
        };
        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(vec![tmp])
}

fn extract_tar_gz_to_temp(tar_gz_path: &std::path::Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let file = std::fs::File::open(tar_gz_path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    let tmp = std::env::temp_dir().join(format!("msmanage-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp)?;
    archive.unpack(&tmp)?;
    Ok(vec![tmp])
}

/// 在 dir 里递归找所有含 SKILL.md 的目录（找到就停，不再下挖 —— 那是 skill 自己的内容）
fn find_skill_dirs(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if d.join("SKILL.md").is_file() {
            out.push(d.to_path_buf());
            return; // 是 skill 自身，不再递归它的子目录
        }
        let Ok(rd) = std::fs::read_dir(d) else { return; };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

/// 把 skill 文件夹复制到 skills/{name}/，并写元数据
fn install_skill_dir(src: std::path::PathBuf, dest_root: &std::path::Path) -> anyhow::Result<String> {
    let raw_name = src.file_name().and_then(|s| s.to_str()).unwrap_or("skill").to_string();
    let safe_name = sanitize_for_filename(&raw_name);
    let dest = dest_root.join(&safe_name);
    if dest.exists() {
        // 已存在：覆盖（先删旧的）
        std::fs::remove_dir_all(&dest).ok();
    }
    copy_dir_recursive(&src, &dest)?;
    Ok(safe_name)
}

fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}
