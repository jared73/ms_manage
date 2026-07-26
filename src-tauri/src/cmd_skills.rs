use crate::state::AppState;
use crate::storage::StoredSkill;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Emitter, State};

fn to_stored(s: serde_json::Value) -> Result<StoredSkill, String> {
    serde_json::from_value(s).map_err(|e| e.to_string())
}

/// 校验 skill target / skill 的 `path` 字段:必须是绝对路径,
/// 不含 `..` 段,且落在一个安全的"用户可写"区域下。
///
/// 安全白名单:
/// - 用户 HOME 下的任何目录(主目录、子目录均可)
/// - Windows 上 C:\Program Files\Common Files 之类公共目录(扩展)
/// - Windows 上 C:\Users\Public 公共用户目录
///
/// 反例: `C:\Windows\System32`、`C:\`、`C:\Program Files\Microsoft Office` 这种
/// 显然不该让 webview 通过 IPC 注入创建 symlink 的位置。
pub(crate) fn validate_skill_path(path_str: &str) -> Result<PathBuf, String> {
    if path_str.is_empty() {
        return Err("path 不能为空".to_string());
    }
    if path_str.contains('\0') {
        return Err("path 含 NUL 字节".to_string());
    }
    let p = Path::new(path_str);
    if !p.is_absolute() {
        return Err(format!("path 必须是绝对路径,收到:{path_str}"));
    }
    // 拒绝任何 .. 段
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(format!("path 不允许 .. 段,收到:{path_str}"));
    }
    // 拒绝 Windows 根目录 / 系统目录
    // 系统盘硬编码 "C:" 之前能让用户装 D:/Windows 绕过 —— 用 SystemRoot / SystemDrive
    // 等 env 变量(或 dirs::system_dir())动态发现,跨盘符也安全。
    let lower = path_str.to_lowercase();
    let mut denied_prefixes: Vec<String> = vec![
        "/etc".into(), "/usr".into(), "/var".into(),
        "/sys".into(), "/proc".into(), "/boot".into(),
    ];
    #[cfg(windows)]
    {
        // Windows 系统目录来自环境变量,而不是硬编码 C:。
        // 优先级: SystemRoot > SystemDrive > dirs::system_dir()
        if let Ok(sw) = std::env::var("SystemRoot") {
            denied_prefixes.push(sw.to_lowercase());
        }
        if let Ok(sd) = std::env::var("SystemDrive") {
            // SystemDrive 是 "C:" 这种形式,补全一个 ":\windows" 兜底
            denied_prefixes.push(format!("{}\\windows", sd.to_lowercase()));
        }
        denied_prefixes.extend([
            r"c:\program files\microsoft".to_string(),
            r"c:\program files (x86)\microsoft".to_string(),
            r"c:\programdata".to_string(),
            r"c:\system volume information".to_string(),
        ]);
    }
    for p in &denied_prefixes {
        if lower.starts_with(p) {
            return Err(format!("path 落在系统目录,不允许:{path_str}"));
        }
    }
    // 必须在 HOME 下 —— Windows 上 HOME 不一定有,但 USERPROFILE 一定有
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").unwrap_or_default()
    } else {
        std::env::var("HOME").unwrap_or_default()
    };
    if home.is_empty() {
        return Err("无法定位用户 HOME 目录".to_string());
    }
    // 简化:路径必须以 home 或 user profile 开头
    // 用原大小写比对(Windows 路径大小写不敏感但 USERPROFILE 大小写通常一致)
    // 必须追加路径分隔符防止兄弟目录绕过(如 HOME=C:\Users\foo 时 C:\Users\foobar 也通过)
    let home_lower = home.to_lowercase();
    let sep = std::path::MAIN_SEPARATOR;
    let home_prefix = format!("{}{}", home_lower, sep);
    let home_matches = lower.starts_with(&home_prefix) || lower == home_lower;
    if !home_matches {
        return Err(format!("path 必须在用户目录下($HOME),收到:{path_str}"));
    }
    Ok(p.to_path_buf())
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
    // 验证 dir_path 在用户目录下 —— 防御 webview 注入
    if !s.dir_path.is_empty() {
        validate_skill_path(&s.dir_path)?;
    }
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

/// 解析 SKILL.md 风格的 frontmatter:
/// - 用 `---` 分隔 YAML + Markdown body
/// - 完整字段经 serde_yaml 还原成对象,而不是 JS regex 一行行 split
/// - 返回 `{ front: {key: value}, name, description, body }`
///
/// 之前前端用 regex /^([\w-]+)\s*:\s*(.*)$/ 一行行 split 时遇到
/// `"Use this: for code review"` 这种含 `:` 的 description 会截断,
/// 或多行 YAML 列表 `- item` 也漏掉 —— YAML 嵌套/多行/特殊字符必须交给 serde_yaml。
#[tauri::command]
pub fn parse_skill_frontmatter(content: String) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let (front_yaml, body) = crate::storage::skills::split_frontmatter(&content);
    let fm: serde_yaml::Value = serde_yaml::from_str(&front_yaml).unwrap_or(serde_yaml::Value::Null);
    // 把 serde_yaml::Value 拍平到顶层字符串/数组/布尔/数字,便于前端 JSON 取用。
    let mut front_out: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    if let serde_yaml::Value::Mapping(map) = fm {
        for (k, v) in map {
            let key_str = match k {
                serde_yaml::Value::String(s) => s,
                other => format!("{:?}", other),
            };
            let json_v = serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
            front_out.insert(key_str, json_v);
        }
    }
    let name = front_out.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let description = front_out.get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(json!({
        "front": front_out,
        "name": name,
        "description": description,
        "body": body,
    }))
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
    // 验证 path 在用户目录下,不在系统目录 —— 防御 webview 注入创建 symlink 到
    // 任意位置(C:\Windows\System32 等)
    let safe_path = validate_skill_path(&path)?;
    let mut settings = state.storage.load_settings().map_err(|e| e.to_string())?;
    let id = format!("custom-{}", uuid::Uuid::new_v4());
    let target = crate::storage::SkillTarget {
        id: id.clone(),
        name,
        path: safe_path.to_string_lossy().into_owned(),
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
    // 如果提供新 path,先验证 —— webview 不能改 path 到系统目录
    if let Some(ref p) = path {
        if !p.is_empty() {
            validate_skill_path(p)?;
        }
    }
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
///
/// 进度反馈:通过 `skill_import_progress` 事件向 webview 推送三阶段(stage):
///   - "extracting": 大文件解压中
///   - "scanning": 找 SKILL.md 中
///   - "installing": 复制到 skills_dir
///   - "done" / "error": 终态
///
/// 这样前端 ImportModal 能展示「解压中…」「扫描中…」而不是只有一个 spinner:
/// 100 MB zip 用户等了 10s 没动静就不知道是 UI 卡了还是真在解压。
#[tauri::command]
pub async fn import_skill_from_path(
    path: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ImportResult, String> {
    let skills_dir = state.storage.skills_dir();
    std::fs::create_dir_all(&skills_dir).map_err(|e| format!("create skills dir: {e}"))?;

    let p = std::path::PathBuf::from(&path);

    if !p.exists() {
        return Err(format!("路径不存在: {path}"));
    }

    let path_for_events = path.clone();
    let path_for_closure = path_for_events.clone();
    let result = tauri::async_runtime::spawn_blocking({
        let app_handle = app.clone();
        move || -> Result<ImportResult, String> {
            let p_inner = std::path::PathBuf::from(&path_for_closure);
            let mut result = ImportResult { imported: vec![], errors: vec![], skipped: vec![] };

            let lower_name = p_inner.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            // 进度反馈:开始解压
            let _ = app_handle.emit("skill_import_progress", serde_json::json!({
                "path": path_for_closure, "stage": "extracting", "message": "正在解压..."
            }));

            let extracted_dirs: Vec<std::path::PathBuf> = if lower_name.ends_with(".zip") {
                extract_zip_to_temp(&p_inner).map_err(|e| format!("解压 zip 失败: {e}"))?
            } else if lower_name.ends_with(".tar.gz") || lower_name.ends_with(".tgz") {
                extract_tar_gz_to_temp(&p_inner).map_err(|e| format!("解压 tar.gz 失败: {e}"))?
            } else if p_inner.is_dir() {
                vec![p_inner.clone()]
            } else {
                return Err(format!("不支持的文件类型: {path_for_closure}（仅支持 .zip / .tar.gz / 文件夹）"));
            };

            // 进度反馈:开始扫描
            let _ = app_handle.emit("skill_import_progress", serde_json::json!({
                "path": path_for_closure, "stage": "scanning", "message": "正在扫描 SKILL.md..."
            }));

            let total_dirs = extracted_dirs.len();
            for (dir_idx, dir) in extracted_dirs.iter().enumerate() {
                let skill_dirs = find_skill_dirs(dir);
                if skill_dirs.is_empty() {
                    result.skipped.push(format!("{:?} 里没找到 SKILL.md", dir));
                    continue;
                }
                for sd in skill_dirs {
                    let name_hint = sd.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let _ = app_handle.emit("skill_import_progress", serde_json::json!({
                        "path": path_for_closure,
                        "stage": "installing",
                        "message": format!(
                            "({}/{}) 安装 {}",
                            dir_idx + 1, total_dirs, name_hint
                        ),
                    }));
                    match install_skill_dir(sd.clone(), &skills_dir) {
                        Ok(name) => result.imported.push(name),
                        Err(e) => result.errors.push(format!("{name_hint}: {e}")),
                    }
                }
            }

            Ok(result)
        }
    }).await.map_err(|e| format!("spawn_blocking join error: {e}"))??;

    let _ = app.emit("skill_import_progress", serde_json::json!({
        "path": path_for_events, "stage": "done", "message": "导入完成"
    }));

    // reconcile symlinks(在 async 上下文里执行,不阻塞)
    let storage_ref = state.inner().clone();
    let _ = crate::storage::skills::reconcile_after_change(&storage_ref.storage);
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

    // 手动遍历 entries 并校验路径，防止 tar 里的 .. 或绝对路径写到 temp 目录外
    // (tar::Archive::unpack 不做路径清理，跟 zip 的 enclosed_name 不同)
    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?;
        // 拒绝绝对路径
        if entry_path.is_absolute() {
            continue;
        }
        // 拒绝含 .. 的路径
        if entry_path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            continue;
        }
        let outpath = tmp.join(&entry_path);
        if entry.header().entry_type() == tar::EntryType::Directory {
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

#[cfg(test)]
mod validate_skill_path_tests {
    //! `validate_skill_path` 是 webview 写入 SkillTarget 前的最后一道门:
    //! 绝对路径、无 `..` 段、不落在系统目录里,且必须落在用户 HOME 下。
    //!
    //! 这些测试不写盘,也不依赖 Storage —— 纯函数验证。
    use super::validate_skill_path;

    /// 跨平台 helper:返回"在当前平台上,理应通过的 user 目录下绝对路径"。
    fn a_user_path() -> &'static str {
        if cfg!(windows) {
            // 真正的 USERPROFILE 值不可知,但 /tmp 不到这一层
            // 我们用 pattern 让单测不对 home 做强校验 —— 这里只展示参数形态
            "C:\\Users\\tester\\AppData\\Roaming\\Trae\\skills"
        } else {
            "/home/tester/.config/trae/skills"
        }
    }

    #[test]
    fn rejects_empty_and_nul() {
        assert!(validate_skill_path("").is_err());
        assert!(validate_skill_path("C:\\foo\0bar").is_err());
    }

    #[test]
    fn rejects_non_absolute() {
        assert!(validate_skill_path("relative/path").is_err());
        assert!(validate_skill_path("./foo").is_err());
    }

    #[test]
    fn rejects_dotdot_segments() {
        assert!(validate_skill_path("C:\\Users\\tester\\..\\Windows\\System32").is_err());
        assert!(validate_skill_path("/home/tester/../../etc/passwd").is_err());
    }

    #[test]
    fn rejects_known_system_dirs() {
        // OS-specific 把柄
        if cfg!(windows) {
            assert!(validate_skill_path("C:\\Windows\\System32\\drivers").is_err());
            assert!(validate_skill_path("C:\\Program Files\\Microsoft Office\\root").is_err());
            assert!(validate_skill_path("C:\\ProgramData\\Microsoft").is_err());
        } else {
            assert!(validate_skill_path("/etc/passwd").is_err());
            assert!(validate_skill_path("/usr/bin/node").is_err());
            assert!(validate_skill_path("/var/log/syslog").is_err());
        }
    }

    #[test]
    fn accepts_path_shape_without_home_check() {
        // a_user_path() 只在概念上"看起来像"用户目录,实际 HOME 校验由生产代码用 env 解析;
        // 这里只确认它能进入"通过形态校验"的阶段(不会被前面几个 if 拒)。
        // 如果跑测试的机器 USERPROFILE 正好等于这个值,会通过 HOME 校验 → Ok
        // 否则返回 Err("path 必须在用户目录下")也是预期行为 —— 我们区分两种退出路径。
        let r = validate_skill_path(a_user_path());
        if let Err(e) = r {
            // 允许的两种错误:home 校验未通过;其它都不是
            assert!(e.contains("用户目录") || e.contains("HOME"), "unexpected err: {e}");
        }
    }
}
