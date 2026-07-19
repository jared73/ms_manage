//! 包管理器安装状态探测（npm / uv）
//!
//! 这里只放与 runtime 无关的解析逻辑；实际执行命令的同步/异步适配器放在调用方
//! 或 process_util.rs 中，避免同一套解析代码写两份。

use regex::Regex;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgManager {
    Npm,
    Uv,
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub version: String,
    pub path: String,
}

/// 判断一条 server 配置是否需要预安装，以及使用哪个包管理器。
/// 返回的包名保留原始版本/标签规格（如 `tavily-mcp@0.1.4`、`@playwright/mcp@latest`），
/// 供 `npm i -g <spec>` 精确安装。需要匹配已装包名时，调用方自行 `strip_version`。
pub fn detect_pkg(command: &str, args: &[String]) -> Option<(PkgManager, String)> {
    let cmd = command.to_ascii_lowercase();
    // 跳过 flag 取第一个非 flag 参数作为包名。
    // 对 uv/uvx，需要先跳过所有子命令（tool / run）。
    let pkg: &str = match cmd.as_str() {
        "uvx" | "uv" => {
            let mut iter = args.iter().filter(|a| !a.starts_with('-'));
            // 跳过 uv 的子命令（tool、run、x 等 uv 子命令字面量）
            let mut next_arg = iter.next()?;
            while next_arg.eq_ignore_ascii_case("tool")
                || next_arg.eq_ignore_ascii_case("run")
            {
                next_arg = iter.next()?;
            }
            next_arg
        }
        _ => args.iter().find(|a| !a.starts_with('-'))?,
    };
    // 注意：不 strip_version —— 安装时要保留用户指定的版本/标签。
    match cmd.as_str() {
        "npx" | "npm" | "pnpm" | "pnpx" | "yarn" | "bun" | "bunx" => {
            Some((PkgManager::Npm, pkg.to_string()))
        }
        "uvx" | "uv" => Some((PkgManager::Uv, pkg.to_string())),
        _ => None,
    }
}

/// `@scope/foo@1.0.0` → `@scope/foo`，`foo@latest` → `foo`
pub fn strip_version(pkg: &str) -> &str {
    if pkg.starts_with('@') {
        if let Some(idx) = pkg[1..].find('@') {
            return &pkg[..=idx];
        }
        return pkg;
    }
    pkg.split('@').next().unwrap_or(pkg)
}

/// 包名 → 候选可执行文件名（去掉 scope，取最后一段）
pub fn pkg_basename(pkg: &str) -> &str {
    let no_scope = pkg.strip_prefix('@').unwrap_or(pkg);
    no_scope.split('/').last().unwrap_or(no_scope)
}

/// 校验包名格式，防止命令注入 / 路径遍历。
/// 允许两种合法形式：
///   - 无 scope：`foo-bar.baz_1`
///   - 有 scope： `@scope/pkg-name`
pub fn validate_pkg_name(pkg: &str) -> bool {
    if pkg.is_empty() || pkg.len() > 214 {
        return false;
    }
    if pkg.contains("..") || pkg.contains("//") {
        return false;
    }
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(?:[a-zA-Z0-9._-]+|@[a-zA-Z0-9._-]+/[a-zA-Z0-9._-]+)(@[a-zA-Z0-9._^~\-]+)?$")
            .unwrap()
    });
    re.is_match(pkg)
}

/// 解析 `npm ls -g <pkg> --json=true --depth=0` 的输出，返回版本号。
pub fn parse_npm_ls(output: &[u8], pkg: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(output).ok()?;
    v.get("dependencies")
        .and_then(|d| d.get(pkg))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// 根据 npm 全局 prefix 和包名，解析出真实可执行文件路径。
/// 优先读 package.json 的 bin 字段，fallback 扫 <prefix>/node_modules/.bin/。
pub fn resolve_npm_bin(prefix: &Path, pkg: &str) -> Option<PathBuf> {
    let pkg_json_path = prefix.join("node_modules").join(pkg).join("package.json");
    let bin_path = std::fs::read_to_string(&pkg_json_path)
        .ok()
        .and_then(|j| {
            let v: serde_json::Value = serde_json::from_str(&j).ok()?;
            v.get("bin").and_then(|b| match b {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(o) => {
                    o.values().next().and_then(|x| x.as_str().map(String::from))
                }
                _ => None,
            })
        })
        .map(|rel| {
            let p = Path::new(&rel);
            if p.is_absolute() {
                PathBuf::from(rel)
            } else {
                prefix.join("node_modules").join(pkg).join(&rel)
            }
        });

    if let Some(p) = bin_path {
        return Some(p);
    }

    let bin_name = pkg_basename(pkg);
    let bin_dir = prefix.join("node_modules").join(".bin");
    for ext in ["", ".cmd", ".bat", ".ps1", ".exe"] {
        let cand = bin_dir.join(format!("{bin_name}{ext}"));
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// 解析 `npm prefix -g` 的输出。
pub fn parse_npm_prefix(output: &[u8]) -> Option<PathBuf> {
    let s = String::from_utf8_lossy(output);
    let prefix = s.trim();
    if prefix.is_empty() {
        None
    } else {
        Some(PathBuf::from(prefix))
    }
}

/// 解析 `uv tool list --show-paths` 的输出。
/// 输出格式：
///   <pkg> v<ver> (<env-dir>)
///   - <exe-name> (<abs-path>)
pub fn parse_uv_tool_list(output: &[u8], pkg: &str) -> Option<ProbeResult> {
    let text = String::from_utf8_lossy(output);
    let mut pkg_ver: Option<String> = None;
    let mut exe_paths: Vec<String> = Vec::new();

    // uv tool list 按包分组：包行后紧跟该包的可执行文件行。
    // 当 current_target 为 true 时，后续 exe 行属于目标包。
    let mut current_target = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('-') {
            let rest = rest.trim();
            // 格式：`- <exe-name> (<abs-path>)`
            // 路径可能含空格，所以从最后一个 " (" 切分。
            if !current_target {
                continue;
            }
            if let Some(idx) = rest.rfind(" (") {
                let path = rest[idx + 2..rest.len() - 1].trim();
                exe_paths.push(path.to_string());
            }
        } else {
            // 包行：`<pkg> v<ver> (<env-dir>)`
            let mut parts = line.split_whitespace();
            let name = parts.next().unwrap_or("");
            current_target = name.eq_ignore_ascii_case(pkg);
            if current_target {
                if let Some(token) = parts.next() {
                    let ver = token.strip_prefix('v').unwrap_or(token);
                    if !ver.is_empty() {
                        pkg_ver = Some(ver.to_string());
                    }
                }
            }
        }
    }
    if pkg_ver.is_none() && exe_paths.is_empty() {
        return None;
    }
    let path = exe_paths.into_iter().next().unwrap_or_default();
    Some(ProbeResult {
        version: pkg_ver.unwrap_or_else(|| "?".to_string()),
        path,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_version() {
        assert_eq!(strip_version("foo@1.0.0"), "foo");
        assert_eq!(strip_version("@scope/foo@1.0.0"), "@scope/foo");
        assert_eq!(strip_version("foo@latest"), "foo");
        assert_eq!(strip_version("foo"), "foo");
        assert_eq!(strip_version("@scope/foo"), "@scope/foo");
    }

    #[test]
    fn test_detect_pkg() {
        use super::PkgManager;

        // npm 系：跳过 flag，取第一个非 flag 参数
        assert_eq!(
            detect_pkg("npx", &vec!["-y".into(), "@scope/pkg".into()]),
            Some((PkgManager::Npm, "@scope/pkg".into()))
        );
        // 带版本/标签：保留规格（安装时按此精确安装）
        assert_eq!(
            detect_pkg("npx", &vec!["-y".into(), "foo@1.0.0".into()]),
            Some((PkgManager::Npm, "foo@1.0.0".into()))
        );
        assert_eq!(
            detect_pkg("npx", &vec!["-y".into(), "@scope/pkg@latest".into()]),
            Some((PkgManager::Npm, "@scope/pkg@latest".into()))
        );

        // uv：uvx 直接传 pkg
        assert_eq!(
            detect_pkg("uvx", &vec!["mcp-server-fetch".into()]),
            Some((PkgManager::Uv, "mcp-server-fetch".into()))
        );
        // uv tool run <pkg> 跳过 tool/run
        assert_eq!(
            detect_pkg("uv", &vec!["tool".into(), "run".into(), "mcp-server-fetch".into()]),
            Some((PkgManager::Uv, "mcp-server-fetch".into()))
        );
        // uv run <pkg> 跳过 run
        assert_eq!(
            detect_pkg("uv", &vec!["run".into(), "mcp-server-fetch".into()]),
            Some((PkgManager::Uv, "mcp-server-fetch".into()))
        );
        // 非 uv/uvx/npx 命令返回 None
        assert_eq!(detect_pkg("node", &vec!["server.js".into()]), None);
    }

    #[test]
    fn test_pkg_basename() {
        assert_eq!(pkg_basename("@scope/foo"), "foo");
        assert_eq!(pkg_basename("foo"), "foo");
        assert_eq!(pkg_basename("@scope/nested/foo"), "foo");
    }

    #[test]
    fn test_validate_pkg_name() {
        assert!(validate_pkg_name("@modelcontextprotocol/server-memory"));
        assert!(validate_pkg_name("foo-bar"));
        assert!(validate_pkg_name("foo@1.0.0"));
        assert!(validate_pkg_name("foo@latest"));
        assert!(validate_pkg_name("@scope/pkg@latest"));
        assert!(validate_pkg_name("@scope/pkg@0.1.4"));
        assert!(validate_pkg_name("foo.bar_baz-1"));
        assert!(validate_pkg_name("@scope/pkg"));
        assert!(!validate_pkg_name("foo;bar"));
        assert!(!validate_pkg_name("foo|bar"));
        assert!(!validate_pkg_name("foo/../bar"));
        assert!(!validate_pkg_name("foo//bar"));
        assert!(!validate_pkg_name("@scope"));
        assert!(!validate_pkg_name("@scope/"));
        assert!(!validate_pkg_name("/foo"));
        assert!(!validate_pkg_name("foo/"));
        assert!(!validate_pkg_name(""));
        assert!(!validate_pkg_name("foo bar"));
    }

    #[test]
    fn test_parse_npm_ls() {
        let json = r#"{"dependencies":{"foo":{"version":"1.2.3"}}}"#;
        assert_eq!(parse_npm_ls(json.as_bytes(), "foo"), Some("1.2.3".to_string()));
        assert_eq!(parse_npm_ls(json.as_bytes(), "bar"), None);
    }

    #[test]
    fn test_parse_uv_tool_list() {
        let text = br#"mcp-server-fetch v1.0.0 (C:\Users\x\.local\share\uv\tools\mcp-server-fetch)
- mcp-server-fetch (C:\Users\x\.local\bin\mcp-server-fetch.exe)
"#;
        let res = parse_uv_tool_list(text, "mcp-server-fetch").unwrap();
        assert_eq!(res.version, "1.0.0");
        assert!(res.path.contains("mcp-server-fetch"));

        // 路径含空格
        let text2 = br#"my-tool v2.0.0-beta.1 (/path/with spaces)
- my-tool (/path/with spaces/bin/my-tool)
"#;
        let res2 = parse_uv_tool_list(text2, "my-tool").unwrap();
        assert_eq!(res2.version, "2.0.0-beta.1");
        assert_eq!(res2.path, "/path/with spaces/bin/my-tool");

        // 不匹配：包名完全不同
        let text3 = br#"other-tool v1.0.0
- other-tool (/bin/other-tool)
"#;
        assert!(parse_uv_tool_list(text3, "my-tool").is_none());
    }
}
