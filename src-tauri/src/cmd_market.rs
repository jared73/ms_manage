use crate::state::AppState;
use crate::storage::StoredServer;
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tauri::State;

/// 内置推荐列表（也作为 registry 的离线 fallback）
fn builtin_items() -> Vec<(&'static str, &'static str, &'static str, &'static str, u64)> {
    vec![
        ("filesystem", "server", "@modelcontextprotocol/server-filesystem", "本地文件系统读写（推荐 . 作为工作目录）", 12000),
        ("fetch", "server", "@modelcontextprotocol/server-fetch", "网页抓取（HTML → Markdown）", 8500),
        ("github", "server", "@modelcontextprotocol/server-github", "GitHub API（issues / PRs / code）", 15000),
        ("postgres", "server", "@modelcontextprotocol/server-postgres", "PostgreSQL 只读查询", 4200),
        ("puppeteer", "server", "@modelcontextprotocol/server-puppeteer", "浏览器自动化", 5800),
        ("memory", "server", "@modelcontextprotocol/server-memory", "基于知识图谱的长期记忆", 6300),
        ("code-review", "skill", "local", "代码评审 Skill", 910),
        ("explain-code", "skill", "local", "逐段解释代码 Skill", 1300),
        ("commit-msg", "skill", "local", "生成 Conventional Commits 信息", 2100),
    ]
}

#[tauri::command]
pub async fn fetch_market(kind: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    let result: Vec<serde_json::Value> = builtin_items().into_iter()
        .filter(|(_, k, _, _, _)| kind.as_ref().map(|kk| kk == *k).unwrap_or(true))
        .map(|(id, kind, author, desc, installs)| json!({
            "id": id,
            "kind": kind,
            "name": id,
            "description": desc,
            "author": author,
            "tags": [],
            "installs": installs,
            "updated_at": "2026-05-01T00:00:00Z",
        }))
        .collect();
    Ok(result)
}

#[tauri::command]
pub async fn install_market_item(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let item = builtin_items().into_iter().find(|(i, _, _, _, _)| *i == id)
        .ok_or_else(|| format!("market item '{id}' not found"))?;
    let (mid, kind, author, desc, _installs) = item;
    match kind {
        "server" => install_market_server(state.inner(), mid, author, desc),
        "skill"  => install_market_skill(state.inner(), mid, author, desc),
        _ => Err(format!("unknown market item kind '{kind}'")),
    }
}

fn install_market_server(state: &AppState, id: &str, _author: &str, desc: &str) -> Result<(), String> {
    // 幂等：已存在同名 id 的 server 直接返回 OK
    let mut list = state.storage.load_servers().map_err(|e| e.to_string())?;
    if list.iter().any(|s| s.name == id) {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    let (command, args) = match id {
        "filesystem" => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), ".".to_string()]),
        "fetch"      => ("uvx".to_string(), vec!["mcp-server-fetch".to_string()]),
        "github"     => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()]),
        "postgres"   => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-postgres".to_string()]),
        "puppeteer"  => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-puppeteer".to_string()]),
        "memory"     => ("npx".to_string(), vec!["-y".to_string(), "@modelcontextprotocol/server-memory".to_string()]),
        _ => return Err(format!("unknown server template '{id}'")),
    };
    let mut env = std::collections::HashMap::new();
    if id == "github" {
        env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".to_string(), String::new());
    }
    let mut tags = vec!["market".to_string()];
    tags.push(id.to_string());
    let s = StoredServer {
        id: uuid::Uuid::new_v4().to_string(),
        name: id.to_string(),
        description: Some(desc.to_string()),
        transport: "stdio".to_string(),
        command: Some(command.to_string()),
        args,
        env,
        cwd: None,
        url: None,
        headers: std::collections::HashMap::new(),
        bearer_token: None,
        enabled: true,
        auto_start: Some(false),
        tags,
        project_id: None,
        disabled_tools: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    list.push(s);
    state.storage.save_servers(&list).map_err(|e| e.to_string())
}

fn install_market_skill(state: &AppState, id: &str, _author: &str, desc: &str) -> Result<(), String> {
    use crate::storage::StoredSkill;
    let dir = state.storage.skills_dir();
    let skill_dir = dir.join(sanitize_id(id));
    let file = skill_dir.join(crate::storage::skills::SKILL_FILE);
    if file.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    let body = match id {
        "code-review" => "# code-review\n\n执行严格的代码评审。覆盖：\n\n- 命名一致性与可读性\n- 错误处理与边界条件\n- 并发与资源释放\n- 性能与可维护性\n\n针对每条意见，给出文件:行号 引用与最小可行修改示例。\n",
        "explain-code" => "# explain-code\n\n逐段解释给定代码片段。规则：\n\n- 每一段先给出一句话目的\n- 再展开讲关键 API/算法\n- 最后说明潜在陷阱\n",
        "commit-msg" => "# commit-msg\n\n根据 git diff 生成 Conventional Commits 提交信息。\n\n格式：\n\n```\n<type>(<scope>): <subject>\n\n<body>\n```\n\ntype ∈ feat / fix / refactor / docs / test / chore / perf。\nsubject 不超过 72 字符，祈使语气。\n",
        _ => return Err(format!("unknown skill template '{id}'")),
    };
    let content = format!("---\nid: {id}\nname: {id}\ndescription: {desc}\nenabled: true\n---\n\n{body}");
    std::fs::write(&file, content).map_err(|e| e.to_string())?;
    let _ = StoredSkill { id: id.to_string(), name: id.to_string(), description: Some(desc.into()), content: body.into(), frontmatter: Default::default(), dir_path: skill_dir.to_string_lossy().to_string(), enabled: true, created_at: now.clone(), updated_at: now };
    // 触发 reconcile
    let _ = crate::storage::skills::reconcile_after_change(&state.storage);
    Ok(())
}

fn sanitize_id(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// CLI 入口：直接从默认 storage 路径安装一个市场 item（不进 GUI）
pub fn install_from_cli(id: &str) {
    use crate::state::AppState;
    let state = match AppState::initialize() {
        Ok(s) => s,
        Err(e) => { eprintln!("init state failed: {e}"); std::process::exit(1); }
    };
    let item = builtin_items().into_iter().find(|(i, _, _, _, _)| *i == id);
    match item {
        None => { eprintln!("✗ unknown market id '{id}'"); std::process::exit(2); }
        Some((_, kind, author, desc, _)) => {
            let result = match kind {
                "server" => install_market_server(&state, id, author, desc),
                "skill"  => install_market_skill(&state, id, author, desc),
                _ => Err(format!("unknown kind '{kind}'")),
            };
            match result {
                Ok(()) => println!("✓ installed '{id}' ({kind})"),
                Err(e) => { eprintln!("✗ install failed: {e}"); std::process::exit(2); }
            }
        }
    }
}
