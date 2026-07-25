//! 跨平台进程创建辅助
//!
//! Windows 关键问题：`.cmd/.bat/.ps1` 文件被 cmd.exe 解释时会 allocate console 闪窗。
//! 解决：读 `.cmd` 文件内容，找出它真正调用的 `.exe`（如 `node.exe npx-cli.js`），
//! 直接 spawn 那个 `.exe`，绕过 cmd.exe。
//!
//! macOS / Linux：npx / uvx 等是 shebang 脚本或原生二进制，直接按 PATH 查找即可。

use std::path::PathBuf;
use anyhow::Context;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command as TokioCommand;

/// 创建进程时用的无窗口标志
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
pub(crate) fn apply_no_window(cmd: &mut TokioCommand) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
pub(crate) fn apply_no_window(_cmd: &mut TokioCommand) {}

/// 判断是否需要走 `cmd.exe /c` 包装（tokio/std 通用）。
#[cfg(windows)]
fn needs_cmd_wrapper(path: &std::path::Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .map_or(false, |e| e == "cmd" || e == "bat" || e == "ps1")
}


/// 构建一个 tokio::process::Command。
/// 自动完成 PATH 解析、Windows cmd.exe /c 包装、CREATE_NO_WINDOW、stdin/stdout/stderr pipe。
pub(crate) fn build_tokio_command(prog: &str, args: &[String]) -> TokioCommand {
    let resolved = resolve_cmd(prog);
    #[cfg(windows)]
    let mut c = if needs_cmd_wrapper(&resolved) {
        let mut c = TokioCommand::new("cmd.exe");
        c.arg("/c").arg(&resolved);
        c.args(args);
        c
    } else {
        let mut c = TokioCommand::new(&resolved);
        c.args(args);
        c
    };
    #[cfg(not(windows))]
    let mut c = TokioCommand::new(&resolved);
    #[cfg(not(windows))]
    {
        c.args(args);
    }
    c.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        apply_no_window(&mut c);
    }
    // npm/uv 类命令注入国内镜像(安装/探测/运行统一走这里)。
    // build_tokio_command 没有用户显式 env,所以传空 map —— 用户 build 自己的 command 时另算。
    inject_registry_env(&mut c, prog, &std::collections::HashMap::new());
    c
}

/// 异步执行命令并返回 stdout。
pub(crate) async fn run_capture(prog: &str, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let out = build_tokio_command(prog, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .output()
        .await
        .with_context(|| format!("spawn {prog} 失败"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("{} 退出码 {:?}: {}", prog, out.status.code(), stderr.trim());
    }
    Ok(out.stdout)
}

/// 浏览器/二进制下载镜像（npmmirror 二进制源）。
/// 解决 playwright/puppeteer 等包安装时下载浏览器走 googleapis 被墙/慢的问题。
const PLAYWRIGHT_MIRROR: &str = "https://registry.npmmirror.com/-/binary/playwright";
const CHROME_FOR_TESTING_MIRROR: &str = "https://registry.npmmirror.com/-/binary/chrome-for-testing";

/// 浏览器下载镜像的 env 键值对（playwright / puppeteer 共用一组）。
fn browser_mirror_envs() -> [(&'static str, &'static str); 4] {
    [
        ("PLAYWRIGHT_DOWNLOAD_HOST", PLAYWRIGHT_MIRROR),
        // playwright 旧版本变量
        ("PLAYWRIGHT_DOWNLOAD_HOST_TEMPLATE", PLAYWRIGHT_MIRROR),
        ("PUPPETEER_DOWNLOAD_BASE_URL", CHROME_FOR_TESTING_MIRROR),
        // puppeteer 旧版本变量
        ("PUPPETEER_DOWNLOAD_HOST", CHROME_FOR_TESTING_MIRROR),
    ]
}

/// 给 npm/uv 类命令注入国内镜像加速环境变量。
/// 读取 settings；`auto_inject_npm_registry` 开启时给 Node 包管理器注入 NPM_CONFIG_REGISTRY，
/// 并注入 playwright/puppeteer 浏览器下载镜像；给 uv/uvx 注入 UV_INDEX_URL / PIP_INDEX_URL。
/// 安装 / 探测 / 运行 server 都走这里，保证国内机器一致性。
///
/// `user_env` 是 server 配置里用户显式设置的 env —— 注入**不覆盖**这些值
/// (与 `proxy::mod::inject_npm_registry` 行为对齐)。
pub(crate) fn inject_registry_env(c: &mut TokioCommand, prog: &str, user_env: &std::collections::HashMap<String, String>) {
    let Some(state) = crate::state::try_current() else { return; };
    let Ok(settings) = state.storage.load_settings() else { return; };
    let stem = std::path::Path::new(prog)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Node.js 包管理器加速（默认淘宝镜像）+ 浏览器二进制镜像
    if matches!(stem.as_str(), "npx" | "npm" | "pnpm" | "pnpx" | "yarn" | "bun" | "bunx" | "node")
        && settings.auto_inject_npm_registry
    {
        let npm_registry = settings.npm_registry.trim();
        if !npm_registry.is_empty()
            && !user_env.contains_key("NPM_CONFIG_REGISTRY")
            && !user_env.contains_key("npm_config_registry")
        {
            c.env("NPM_CONFIG_REGISTRY", npm_registry);
            c.env("npm_config_registry", npm_registry);
            c.env("npm_registry", npm_registry);
        }
        for (k, v) in browser_mirror_envs() {
            if !user_env.contains_key(k) {
                c.env(k, v);
            }
        }
    }
    // uv 加速（Python MCP server）—— 用户显式配置优先(以前总是覆盖,会冲掉用户的镜像)
    if matches!(stem.as_str(), "uv" | "uvx") {
        let mirror = "https://mirrors.aliyun.com/pypi/simple/";
        if !user_env.contains_key("UV_INDEX_URL") {
            c.env("UV_INDEX_URL", mirror);
        }
        if !user_env.contains_key("PIP_INDEX_URL") {
            c.env("PIP_INDEX_URL", mirror);
        }
    }
}


/// 异步执行命令并返回完整 Output，不因非零退出码报错。
/// 用于「非零退出码本身是合法结果」的探测场景（如 `npm ls -g <pkg>` 对未安装包返回 1）。
pub(crate) async fn run_capture_output(prog: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
    build_tokio_command(prog, &args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .output()
        .await
        .with_context(|| format!("spawn {prog} 失败"))
}

/// 异步流式执行命令，stdout/stderr 逐行打印到 eprintln!。
pub(crate) async fn run_streamed(prog: &str, args: &[String]) -> anyhow::Result<()> {
    use std::collections::VecDeque;
    const ERR_TAIL_KEEP: usize = 64;

    let mut child = build_tokio_command(prog, args)
        .spawn()
        .with_context(|| format!("spawn {prog} 失败"))?;

    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    let stderr = child.stderr.take();

    let mut out_reader = tokio::io::BufReader::new(stdout).lines();
    let mut err_reader = stderr.map(|e| tokio::io::BufReader::new(e).lines());

    let mut out_done = false;
    let mut err_done = false;
    // 只保留尾部 ERR_TAIL_KEEP 行 stderr，避免 verbose 输出无界占用内存。
    let mut err_buf: VecDeque<String> = VecDeque::with_capacity(ERR_TAIL_KEEP);

    let read_result: anyhow::Result<()> = async {
        while !out_done || !err_done {
            tokio::select! {
                line = out_reader.next_line(), if !out_done => {
                    match line {
                        Ok(Some(line)) => {
                            if !line.is_empty() {
                                eprintln!("[install] {prog}: {line}");
                            }
                        }
                        Ok(None) => out_done = true,
                        Err(e) => anyhow::bail!("stdout: {e}"),
                    }
                }
                line = async {
                    match err_reader.as_mut() {
                        Some(r) => r.next_line().await,
                        None => std::future::pending().await,
                    }
                }, if !err_done => {
                    match line {
                        Ok(Some(line)) => {
                            if !line.is_empty() {
                                if err_buf.len() == ERR_TAIL_KEEP {
                                    err_buf.pop_front();
                                }
                                err_buf.push_back(line.clone());
                                eprintln!("[install] {prog}!: {line}");
                            }
                        }
                        Ok(None) => err_done = true,
                        Err(e) => anyhow::bail!("stderr: {e}"),
                    }
                }
            }
        }
        Ok(())
    }
    .await;

    let status = child.wait().await.with_context(|| format!("wait {prog}"))?;
    if let Err(e) = read_result {
        // 读取失败 / 子进程错误时确保子进程被杀掉，否则 npm/uv 安装进程会成孤儿继续运行。
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(e);
    }
    if !status.success() {
        let tail = err_buf.iter().cloned().collect::<Vec<_>>().join("; ");
        anyhow::bail!("{} 退出码 {:?}{}", prog, status.code(),
            if tail.is_empty() { String::new() } else { format!(": {tail}") });
    }
    Ok(())
}

/// PATH 解析：`npx` → 平台对应的真实可执行路径
///
/// - Windows：优先找 `.exe`，再 `.cmd/.bat/.ps1`，避免闪窗
/// - Unix：直接查 PATH，检查文件存在且可执行
pub(crate) fn resolve_cmd(name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() || p.extension().is_some() { return p; }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            {
                let suffixes = ["exe", "cmd", "bat", "ps1", ""];
                for ext in &suffixes {
                    let candidate = if ext.is_empty() { dir.join(name) } else { dir.join(format!("{name}.{ext}")) };
                    if let Ok(meta) = candidate.metadata() {
                        if meta.is_file() && (meta.len() >= 4096 || (*ext != "" && *ext != "ps1")) {
                            return candidate;
                        }
                    }
                }
            }
            #[cfg(not(windows))]
            {
                let candidate = dir.join(name);
                if is_executable(&candidate) {
                    return candidate;
                }
            }
        }
    }
    p
}

/// Unix 可执行性检查：文件存在且至少有一个可执行位
#[cfg(not(windows))]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

/// .cmd/.bat 文件解析：读内容找出真实 .exe 路径 + 脚本路径
/// 返回 (真实 exe 路径, 传给 exe 的参数)
/// 如果解析失败，返回 None（fallback 到 cmd.exe /c 包装）
///
/// 算法：从最后一行往回扫，找含 `%*` 的真正调用行（如 `"%NODE_EXE%" "%NPX_CLI_JS%" %*`），
/// 跳过 FOR /F 块内部的 SET 行。如果找不到含 `%*` 的行，fallback 到最后一条含 .js 的非控制流行。
#[cfg(windows)]
pub(crate) fn parse_cmd_file(cmd_path: &PathBuf) -> Option<(PathBuf, Vec<String>)> {
    let content = std::fs::read_to_string(cmd_path).ok()?;
    let cmd_dir = cmd_path.parent()?;

    // node.exe 在 npx.cmd 同级目录
    let node_exe = cmd_dir.join("node.exe");
    if !node_exe.exists() { return None; }

    let lines: Vec<&str> = content.lines().collect();

    // 策略：优先找含 %* 的行（真正的调用行），其次找最后一条含 .js 的非控制流行
    let mut best_script: Option<String> = None;

    for line in lines.iter().rev() {
        let trimmed = line.trim_start();
        // 跳过空行、注释、控制流关键字（大小写不敏感）
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.is_empty()
            || trimmed.starts_with('@')
            || lower.starts_with("if ")
            || lower.starts_with("for ")
            || lower.starts_with("set ")
            || lower.starts_with("echo ")
            || lower.starts_with("endlocal")
            || lower.starts_with("local")
            || lower.starts_with("goto ")
            || lower.starts_with("call ")
        {
            continue;
        }

        // 找 .js 路径（含 node_modules 或 %dp0%）
        let mut script_path: Option<String> = None;
        for token in trimmed.split_whitespace() {
            let t = token.trim_matches('"').trim_matches('\'');
            if t.ends_with(".js") && (t.contains("node_modules") || t.contains("%dp0%")) {
                let resolved = t.replace("%dp0%", &cmd_dir.to_string_lossy())
                    .replace('\\', "/");
                script_path = Some(resolved);
            }
        }

        if let Some(sp) = script_path {
            // 含 %* 的行是真正的调用行，直接用
            if trimmed.contains("%*") {
                return Some((node_exe, vec![sp]));
            }
            // 否则暂存，继续往前找更好的
            if best_script.is_none() {
                best_script = Some(sp);
            }
        }
    }

    let script = best_script?;
    Some((node_exe, vec![script]))
}

/// uvx 缓存路径（绕过 uvx 直接 spawn 缓存的 .exe）
#[cfg(windows)]
pub(crate) fn resolve_uvx_cached_exe(package: &str) -> Option<PathBuf> {
    let cache_root = dirs::data_local_dir()?.join("uv").join("cache").join("archive-v0");
    if !cache_root.is_dir() { return None; }
    let Ok(entries) = std::fs::read_dir(&cache_root) else { return None; };
    let target = format!("{package}.exe");
    for e in entries.flatten() {
        let exe = e.path().join("Scripts").join(&target);
        if exe.is_file() { return Some(exe); }
    }
    None
}

/// Unix 上 uv tool 安装的可执行文件路径。
/// uv 默认把工具链接到 `~/.local/bin/<package>`。
#[cfg(not(windows))]
pub(crate) fn resolve_uvx_cached_exe(package: &str) -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".local").join("bin").join(package))
        .filter(|p| is_executable(p))
}

/// 把命令 + 参数转成可直接 spawn 的形式
/// - Windows：uvx <pkg> → 绕过 uvx，用缓存的 .exe；npx <pkg> → 直接找 npm 缓存里的 node.exe + npx-cli.js
/// - Unix：uvx / npx 等 shebang 脚本或原生二进制直接 spawn
/// - .exe      → 原样
pub(crate) fn resolve_spawn_command(cmd: &PathBuf, args: &[String]) -> (PathBuf, Vec<String>) {
    // uvx 绕过
    if let Some(name) = cmd.file_stem().and_then(|s| s.to_str()) {
        if name.eq_ignore_ascii_case("uvx") && !args.is_empty() {
            if let Some(cached) = resolve_uvx_cached_exe(&args[0]) {
                let mut new_args = Vec::with_capacity(args.len() - 1);
                new_args.extend(args[1..].iter().cloned());
                return (cached, new_args);
            }
        }
    }

    #[cfg(windows)]
    {
        // npx / npm .cmd 文件解析
        if cmd.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).map_or(false, |e| e == "cmd" || e == "bat") {
            if let Some((exe, mut exe_args)) = parse_cmd_file(cmd) {
                exe_args.extend(args.iter().cloned());
                return (exe, exe_args);
            }
        }
    }

    // Unix：如果 cmd 是 shebang 脚本（如 npx），std::process::Command 会按 shebang 执行。
    // 上面 uvx 未命中缓存时，也直接返回原 cmd，由系统调用 uvx 本体。

    (cmd.clone(), args.to_vec())
}
