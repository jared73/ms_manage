use crate::state::AppState;
use crate::storage::StoredServer;
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tauri::State;

const ANTHROPIC_SKILLS_REPO: &str = "anthropics/claude-plugins-official";

fn cache_path(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("market-cache.json")
}

fn builtin_servers() -> Vec<(&'static str, &'static str, &'static str, &'static str, u64)> {
    vec![
        ("fetch",              "server", "@modelcontextprotocol/server-fetch",                "网页抓取（HTML → Markdown）", 8500),
        ("memory",             "server", "@modelcontextprotocol/server-memory",              "基于知识图谱的长期记忆", 6300),
        ("sequentialthinking", "server", "@modelcontextprotocol/server-sequential-thinking",  "分步推理 / 复杂任务分解", 9100),
        ("context7",           "server", "@upstash/context7-mcp",                            "库/框架官方文档查询", 22800),
    ]
}

fn builtin_skills() -> Vec<(&'static str, &'static str, &'static str, &'static str, u64)> {
    vec![
        ("create-skill",   "skill", "alexanderop", "从需求描述创建完整 SKILL.md（alexanderop/claude-code-builder）", 280),
        ("grill-me",       "skill", "mattpocock",  "用尖锐的反向问题审视当前方案（mattpocock/skills）", 740),
        ("superpower",     "skill", "obra",        "通用瑞士军刀 Skill（obra/superpowers）", 1500),
        ("frontend-design","skill", "anthropics",  "前端设计 Skill（来自 Anthropic 官方 plugins 仓库）", 560),
    ]
}

fn fallback_items() -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (id, kind, author, desc, installs) in builtin_servers().into_iter().chain(builtin_skills().into_iter()) {
        out.push(json!({
            "id": id, "kind": kind, "name": id, "description": desc,
            "author": author, "tags": Vec::<String>::new(),
            "installs": installs, "updated_at": "2026-05-01T00:00:00Z",
        }));
    }
    out
}

/// 从 GitHub 拉取 Anthropic Plugins（用 API 读目录 + plugin.json 描述）
async fn fetch_from_github() -> Result<Vec<serde_json::Value>, String> {
    // 读 settings 拿 GitHub token（有 token 限流 5000 次/小时，无 token 只有 60 次/小时）
    let github_token = crate::state::try_current()
        .and_then(|s| s.storage.load_settings().ok())
        .and_then(|s| s.github_token.clone())
        .unwrap_or_default();

    eprintln!("[market] fetch_from_github: listing plugins (token={})...", if github_token.is_empty() { "none" } else { "set" });
    let mut builder = reqwest::Client::builder().user_agent("ms-manage/0.1.0");
    let client = builder.build().map_err(|e| format!("HTTP client failed: {e}"))?;

    let url = format!("https://api.github.com/repos/{}/contents/plugins", ANTHROPIC_SKILLS_REPO);
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json");
    if !github_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {github_token}"));
    }
    let resp = req.send().await
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await
        .map_err(|e| format!("GitHub API response parse failed: {e}"))?;

    let mut items: Vec<serde_json::Value> = Vec::new();

    if let Some(entries) = body.as_array() {
        eprintln!("[market] found {} plugin directories", entries.len());
        // 逐个读取 plugin.json（不用 stream，简单循环避免 async 类型问题）
        for entry in entries {
            let name = entry.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if entry_type != "dir" || name.starts_with('.') {
                continue;
            }
            let desc = fetch_plugin_desc(&client, name).await
                .unwrap_or_else(|| name.replace('-', " "));
            eprintln!("[market]   plugin: {} - {}", name, desc);
            items.push(json!({
                "id": name, "kind": "skill", "name": name,
                "description": desc, "author": "anthropic",
                "source_url": format!("https://github.com/{}/tree/main/plugins/{}", ANTHROPIC_SKILLS_REPO, name),
                "tags": ["anthropic"],
                "installs": 0, "updated_at": Utc::now().to_rfc3339(),
            }));
        }
    }

    for (id, kind, author, desc, installs) in builtin_servers() {
        items.push(json!({
            "id": id, "kind": kind, "name": id, "description": desc,
            "author": author, "tags": Vec::<String>::new(),
            "installs": installs, "updated_at": "2026-05-01T00:00:00Z",
        }));
    }

    for (id, kind, author, desc, installs) in builtin_skills() {
        items.push(json!({
            "id": id, "kind": kind, "name": id, "description": desc,
            "author": author, "tags": Vec::<String>::new(),
            "installs": installs, "updated_at": "2026-05-01T00:00:00Z",
        }));
    }

    eprintln!("[market] fetch_from_github done, total {} items", items.len());
    Ok(items)
}

/// 读取 plugins/<name>/.claude-plugin/plugin.json 的 description
async fn fetch_plugin_desc(client: &reqwest::Client, name: &str) -> Option<String> {
    let github_token = crate::state::try_current()
        .and_then(|s| s.storage.load_settings().ok())
        .and_then(|s| s.github_token.clone())
        .unwrap_or_default();

    let url = format!(
        "https://api.github.com/repos/{}/contents/plugins/{}/.claude-plugin/plugin.json",
        ANTHROPIC_SKILLS_REPO, name
    );
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json");
    if !github_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {github_token}"));
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() { return None; }
    let body: serde_json::Value = resp.json().await.ok()?;
    let content = body.get("content")?.as_str()?;
    let decoded = base64_decode(content).ok()?;
    let json: serde_json::Value = serde_json::from_str(&decoded).ok()?;
    json.get("description")?.as_str().map(|s| s.to_string())
}

/// 简单 base64 解码（不需要额外 crate）
fn base64_decode(s: &str) -> Result<String, String> {
    const TABLE: [u8; 128] = {
        let mut t = [255u8; 128];
        let mut i = 0;
        while i < 26 { t[b'A' as usize + i] = i as u8; i += 1; }
        while i < 52 { t[b'a' as usize + (i - 26)] = i as u8; i += 1; }
        while i < 62 { t[b'0' as usize + (i - 52)] = i as u8; i += 1; }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };
    let clean: Vec<u8> = s.bytes().filter(|&b| b != b'\n' && b != b'\r' && b != b' ').collect();
    if clean.is_empty() { return Ok(String::new()); }
    let mut out = Vec::new();
    for chunk in clean.chunks(4) {
        let mut buf = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            buf[i] = TABLE.get(b as usize).copied().unwrap_or(0);
        }
        out.push((buf[0] << 2) | (buf[1] >> 4));
        if chunk.len() > 2 { out.push((buf[1] << 4) | (buf[2] >> 2)); }
        if chunk.len() > 3 { out.push((buf[2] << 6) | buf[3]); }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_market(
    kind: Option<String>,
    _force_refresh: Option<bool>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    eprintln!("[market] fetch_market: kind={:?}", kind);

    // 全部用本地默认（4 server + 4 skill），不再访问 GitHub
    let all = default_market_items();
    if let Ok(json) = serde_json::to_string_pretty(&all) {
        let cp = cache_path(&state);
        let _ = std::fs::write(cp, json);
    }

    let filtered: Vec<serde_json::Value> = all.into_iter().filter(|item| {
        kind.as_ref().map(|k| item.get("kind").and_then(|v| v.as_str()) == Some(k.as_str())).unwrap_or(true)
    }).collect();

    eprintln!("[market] returning {} items (kind={:?})", filtered.len(), kind);
    Ok(filtered)
}

/// 返回本地默认的 4 server + 4 skill
fn default_market_items() -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (id, kind, author, desc, installs) in builtin_servers().into_iter().chain(builtin_skills().into_iter()) {
        out.push(json!({
            "id": id, "kind": kind, "name": id, "description": desc,
            "author": author, "source_url": "", "tags": Vec::<String>::new(),
            "installs": installs, "updated_at": "2026-05-01T00:00:00Z",
        }));
    }
    out
}

async fn fetch_and_cache(cp: &std::path::Path) -> Result<Vec<serde_json::Value>, String> {
    match fetch_from_github().await {
        Ok(list) => {
            if let Ok(json) = serde_json::to_string_pretty(&list) {
                let _ = std::fs::write(cp, json);
            }
            Ok(list)
        }
        Err(e) => {
            eprintln!("[market] GitHub fetch failed: {e}, using fallback and caching");
            let fallback = fallback_items();
            if let Ok(json) = serde_json::to_string_pretty(&fallback) {
                let _ = std::fs::write(cp, json);
            }
            Ok(fallback)
        }
    }
}

#[tauri::command]
pub async fn install_market_item(
    id: String,
    kind: String,
    source_url: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    match kind.as_str() {
        "server" => install_market_server(state.inner(), &id),
        "skill" => {
            // 按 id 决定从哪取：local 内嵌 vs github 下载
            let id_str = id.clone();
            match id.as_str() {
                "frontend-design" => install_skill_from_github(state.inner(), &id_str, "").await,
                "grill-me" => install_skill_from_github(state.inner(), &id_str, "").await,
                "superpower" => install_skill_from_github(state.inner(), &id_str, "").await,
                _ => install_market_skill(state.inner(), &id),
            }
        }
        _ => Err(format!("unknown kind '{kind}'")),
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn decompress_tar_gz(bytes: &[u8], dest: &std::path::Path) -> Result<(), String> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(dest).map_err(|e| format!("unpack failed: {e}"))
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("create dir failed: {e}"))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read dir failed: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        let dest = dst.join(&name);
        if path.is_dir() {
            copy_dir_all(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest).map_err(|e| format!("copy file failed: {e}"))?;
        }
    }
    Ok(())
}

async fn install_skill_from_github(state: &AppState, id: &str, _source_url: &str) -> Result<(), String> {
    let dir = state.storage.skills_dir();
    let skill_dir = dir.join(sanitize_id(id));
    let file = skill_dir.join(crate::storage::skills::SKILL_FILE);
    if file.exists() { return Ok(()); }
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;

    // 读 settings 拿 GitHub token
    let github_token = crate::state::try_current()
        .and_then(|s| s.storage.load_settings().ok())
        .and_then(|s| s.github_token.clone())
        .unwrap_or_default();

    // 按 id 自动选 repo + 路径：
    //   frontend-design → anthropics/claude-plugins-official/plugins/<id>/skills/<id>/
    //   grill-me         → mattpocock/skills/skills/productivity/<id>/
    //   superpower        → obra/superpowers/skills/using-superpowers/
    let (repo, path, branch) = match id {
        "frontend-design" => (
            ANTHROPIC_SKILLS_REPO,
            format!("plugins/{}/skills/{}", id, id),
            "main",
        ),
        "grill-me" => (
            "mattpocock/skills",
            format!("skills/productivity/{}", id),
            "main",
        ),
        "superpower" => (
            "obra/superpowers",
            "skills/using-superpowers".to_string(),
            "main",
        ),
        "create-skill" => (
            "alexanderop/claude-code-builder",
            format!("commands/{}", id),
            "main",
        ),
        _ => (
            ANTHROPIC_SKILLS_REPO,
            format!("plugins/{}/skills/{}", id, id),
            "main",
        ),
    };

    // 下载仓库 tarball（包含完整目录）
    let url = format!("https://api.github.com/repos/{}/tarball/{}", repo, percent_encode(branch));
    let client = reqwest::Client::builder()
        .user_agent("ms-manage/0.1.0")
        .build()
        .map_err(|e| format!("HTTP client failed: {e}"))?;
    let mut req = client.get(&url)
        .header("Accept", "application/vnd.github.v3+json");
    if !github_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {github_token}"));
    }
    let resp = req.send().await
        .map_err(|e| format!("download repo tarball failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download repo tarball failed: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("read tarball failed: {e}"))?;

    // 解压到临时目录
    let temp = std::env::temp_dir().join(format!("ms-manage-skill-{}-{}", id, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    decompress_tar_gz(&bytes, &temp)?;

    // GitHub tarball 顶层是 owner-repo-sha/，找到它
    let root: std::path::PathBuf = std::fs::read_dir(&temp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .ok_or_else(|| "tarball root directory not found".to_string())?;

    // 定位到仓库内的 skill 子目录
    let source = root.join(&path);
    if !source.exists() {
        return Err(format!("skill directory not found in repo: {}", path));
    }
    if !source.join(crate::storage::skills::SKILL_FILE).exists() {
        return Err(format!("SKILL.md not found in downloaded directory: {}", source.display()));
    }

    // 把完整目录内容复制到 skills/<id>/
    copy_dir_all(&source, &skill_dir)?;

    // 清理临时目录
    let _ = std::fs::remove_dir_all(&temp);

    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

fn install_market_server(state: &AppState, id: &str) -> Result<(), String> {
    let mut list = state.storage.load_servers().map_err(|e| e.to_string())?;
    if list.iter().any(|s| s.name == id) { return Ok(()); }
    let now = Utc::now().to_rfc3339();
    let (command, args) = match id {
        "fetch"              => ("uvx".to_string(), vec!["mcp-server-fetch".to_string()]),
        "memory"             => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-memory".to_string()]),
        "sequentialthinking" => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-sequential-thinking".to_string()]),
        "context7"           => ("npx".to_string(), vec!["-y".to_string(), "@upstash/context7-mcp".to_string()]),
        _ => return Err(format!("unknown server template '{id}'")),
    };
    let mut env = std::collections::HashMap::new();
    if id == "context7" { env.insert("CONTEXT7_API_KEY".to_string(), String::new()); }
    let mut tags = vec!["market".to_string(), id.to_string()];
    list.push(StoredServer {
        id: uuid::Uuid::new_v4().to_string(), name: id.to_string(),
        description: None, transport: "stdio".to_string(),
        command: Some(command), args, env, cwd: None, url: None,
        headers: std::collections::HashMap::new(), bearer_token: None,
        enabled: true, auto_start: Some(false), tags, project_id: None,
        disabled_tools: Vec::new(), created_at: now.clone(), updated_at: now,
        installed: false, installed_version: None, installed_path: None,
        installed_at: None, install_error: None,
    });
    state.storage.save_servers(&list).map_err(|e| e.to_string())
}

fn install_market_skill(state: &AppState, id: &str) -> Result<(), String> {
    let dir = state.storage.skills_dir();
    let skill_dir = dir.join(sanitize_id(id));
    let file = skill_dir.join(crate::storage::skills::SKILL_FILE);
    if file.exists() { return Ok(()); }
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    let body = match id {
        "create-skill" => "# create-skill\n\n创建新 Skill 的工作流：\n\n1. 问用户目的 + 场景\n2. 问调用方式\n3. 起草 frontmatter\n4. 写正文\n5. 保存到 skills/<id>/SKILL.md\n",
        "grill-me" => concat!(
            "---\n",
            "name: grill-me\n",
            "description: A relentless interview to sharpen a plan or design.\n",
            "---\n\n",
            "# /grill-me\n\n",
            "Interview me relentlessly about every aspect of this plan until we reach a shared understanding. ",
            "Walk down each branch of the design tree, resolving dependencies between decisions one-by-one. ",
            "For each question, provide your recommended answer.\n\n",
            "Ask the questions one at a time, waiting for feedback on each question before continuing. ",
            "Asking multiple questions at once is bewildering.\n\n",
            "If a question can be answered by exploring the codebase, explore the codebase instead.\n\n",
            "---\n\n",
            "## Process\n\n",
            "For each question, the model provides its recommended answer, resolving dependencies between decisions one-by-one.\n\n",
            "## When to use\n\n",
            "- User wants to stress-test a plan before building\n",
            "- User uses any \"grill\" trigger phrases\n\n",
            "## Source\n\n",
            "From https://github.com/mattpocock/skills (mattpocock/skills, productivity/grill-me + grilling)\n",
        ),
        "superpower" => "# superpower\n\n通用瑞士军刀 Skill，不知道用啥时兜底。\n\n理解任务 → 判断类型 → 执行工作流\n",
        _ => return Err(format!("unknown skill template '{id}'")),
    };
    let content = format!("---\nid: {id}\nname: {id}\ndescription: {id}\nenabled: true\n---\n\n{body}");
    std::fs::write(&file, content).map_err(|e| e.to_string())?;
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

fn sanitize_id(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

pub fn install_from_cli(id: &str) {
    use crate::state::AppState;
    let state = AppState::initialize().expect("init state failed");
    let result = builtin_skills().iter().find(|(i, _, _, _, _)| *i == id)
        .map(|_| install_market_skill(&state, id))
        .or_else(|| builtin_servers().iter().find(|(i, _, _, _, _)| *i == id)
            .map(|_| install_market_server(&state, id)))
        .unwrap_or_else(|| Err(format!("unknown market id '{id}'")));
    match result {
        Ok(()) => println!("✓ installed '{id}'"),
        Err(e) => { eprintln!("✗ install failed: {e}"); std::process::exit(2); }
    }
}