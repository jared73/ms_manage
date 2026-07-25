use crate::state::AppState;
use crate::storage::StoredServer;
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tauri::State;

const ANTHROPIC_SKILLS_REPO: &str = "anthropics/claude-plugins-official";

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

#[tauri::command]
pub async fn fetch_market(
    kind: Option<String>,
    _force_refresh: Option<bool>,
    _state: State<'_, Arc<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    eprintln!("[market] fetch_market: kind={:?}", kind);

    // 全部用本地默认(4 server + 4 skill),不再访问 GitHub。
    // 注意:之前会写 cache_path —— 现在改为始终由 default 提供数据,
    // cache 仅作离线 fallback,如果将来真的要接 GitHub,可以基于 cache 判断 stale。
    let all = default_market_items();

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

#[tauri::command]
pub async fn install_market_item(
    id: String,
    kind: String,
    _source_url: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    match kind.as_str() {
        "server" => {
            let new_id = install_market_server(state.inner(), &id)?;
            // market 安装 server 后自动在后台触发预安装，前端可订阅 install_progress
            crate::cmd_install::schedule_install(&app, &state, &new_id
            ).await?;
            Ok(())
        }
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
    // 三道防护:
    // - entry count ≤ 10,000(防止 5 GB 仓库被无脑展开)
    // - 单 entry size ≤ 64 MB(防单个文件 OOM)
    // - 总累计解压字节 ≤ 1 GB(防 zip bomb,sparse 编码后放大 10x 仍可控)
    // 所有上限都比单用户合理 skill 体积大一到两个数量级,超过即视为异常。
    const MAX_ENTRIES: usize = 10_000;
    const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    // 手动遍历 entries 并校验路径，防止 tar 里的 .. 或绝对路径写到 dest 目录外
    // (tar::Archive::unpack 不做路径清理)
    let mut count = 0usize;
    let mut total_uncompressed: u64 = 0;
    for entry in archive.entries().map_err(|e| format!("read tar entries: {e}"))? {
        count += 1;
        if count > MAX_ENTRIES {
            return Err(format!("tar entry 超过 {MAX_ENTRIES} 上限,疑似恶意/异常 tarball"));
        }
        let mut entry = entry.map_err(|e| format!("read tar entry: {e}"))?;
        let entry_size = entry.header().size().unwrap_or(0);
        if entry_size > MAX_ENTRY_BYTES {
            return Err(format!("tar entry 过大:{} bytes (上限 {})", entry_size, MAX_ENTRY_BYTES));
        }
        total_uncompressed += entry_size;
        if total_uncompressed > MAX_TOTAL_BYTES {
            return Err(format!("tarball 累计解压超过 {MAX_TOTAL_BYTES} 字节,疑似 zip bomb"));
        }
        let entry_path = entry.path().map_err(|e| format!("tar entry path: {e}"))?;
        // 跳过绝对路径和含 .. 的条目
        if entry_path.is_absolute() { continue; }
        if entry_path.components().any(|c| matches!(c, std::path::Component::ParentDir)) { continue; }
        let outpath = dest.join(&entry_path);
        if entry.header().entry_type() == tar::EntryType::Directory {
            std::fs::create_dir_all(&outpath).map_err(|e| format!("create dir: {e}"))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("create parent: {e}"))?;
            }
            let mut out = std::fs::File::create(&outpath).map_err(|e| format!("create file: {e}"))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("write file: {e}"))?;
        }
    }
    Ok(())
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
        .user_agent("ms-manage/0.2.0")
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
    // 下载体积硬上限(256 MB):GitHub single-file download 限 100 MB,完整 repo tarball 上限 5 GB
    // —— 单个 user 不会让一个 skill 市场安装超过几百 MB,超过一律拒。
    // 边下边校验 Content-Length,避免先全部加载再判定 OOM。
    const MAX_TARBALL_BYTES: u64 = 256 * 1024 * 1024;
    if let Some(cl) = resp.content_length() {
        if cl > MAX_TARBALL_BYTES {
            return Err(format!(
                "tarball 过大:Content-Length {cl} > 上限 {MAX_TARBALL_BYTES} (skill 仓库可能被入侵或 repo 误指)"
            ));
        }
    }
    // 累计下载字节:实际 body 也可能撒谎说 0,我们每 8 MB 累加一次。
    // 用 streaming 拿到字节而非 bytes().await,可以边读边限制总量。
    use futures::StreamExt;
    let mut total: u64 = 0;
    let mut buf = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("read tarball chunk failed: {e}"))?;
        total += chunk.len() as u64;
        if total > MAX_TARBALL_BYTES {
            return Err(format!(
                "tarball 下载超过 {MAX_TARBALL_BYTES} 字节上限(实际累计 {total}),已中止"
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    let bytes = buf;

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

fn install_market_server(state: &AppState, id: &str) -> Result<String, String> {
    let mut list = state.storage.load_servers().map_err(|e| e.to_string())?;
    // 仅当已有记录带 "market" tag 时才复用，否则视为同名但不同配置的自建 server。
    if let Some(existing) = list.iter().find(|s| s.name == id && s.tags.iter().any(|t| t == "market")) {
        return Ok(existing.id.clone());
    }
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
    let tags = vec!["market".to_string(), id.to_string()];
    let new_id = uuid::Uuid::new_v4().to_string();
    list.push(StoredServer {
        id: new_id.clone(), name: id.to_string(),
        description: None, transport: "stdio".to_string(),
        command: Some(command), args, env, cwd: None, url: None,
        headers: std::collections::HashMap::new(),
        enabled: true, auto_start: Some(false), tags, project_id: None,
        disabled_tools: Vec::new(), created_at: now.clone(), updated_at: now,
        installed: false, installed_version: None, installed_path: None,
        installed_at: None, install_error: None,
    });
    state.storage.save_servers(&list).map_err(|e| e.to_string())?;
    Ok(new_id)
}

fn install_market_skill(state: &AppState, id: &str) -> Result<(), String> {
    let dir = state.storage.skills_dir();
    let skill_dir = dir.join(sanitize_id(id));
    let file = skill_dir.join(crate::storage::skills::SKILL_FILE);
    if file.exists() { return Ok(()); }
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
    let _now = Utc::now().to_rfc3339();
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
            .map(|_| {
                install_market_server(&state, id).map(|_new_id| ())
            }))
        .unwrap_or_else(|| Err(format!("unknown market id '{id}'")));
    match result {
        Ok(()) => println!("✓ installed '{id}'"),
        Err(e) => { eprintln!("✗ install failed: {e}"); std::process::exit(2); }
    }
}