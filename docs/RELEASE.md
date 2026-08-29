# MS Manage 发布手册（含踩坑记录）

本文件记录 v1.7.0 起确立的发布流程、环境要求，以及实现自动更新期间踩过的所有坑。
**新坑务必补记到「踩坑记录」末尾**。

## 一、自动更新架构

```
客户端（Tauri updater 插件）
  └─ 设置 → 关于 → 「检查更新」/ 安装版自检
       └─ GET https://github.com/jared73/ms_manage/releases/
             download/v{当前版本}/update.json            ← 更新清单（JSON）
              └─ 清单里 signature + url 指向安装包 → 下载 → 验签 → 静默安装 → 重启
```

- **更新源**：GitHub Releases（`jared73/ms_manage`，公开仓库）
- **安全模型**：安装包用 minisign 私钥签名，客户端内置公钥验签。即使 GitHub 被攻破/被篡改，
  验签失败会自动中止 —— 不需要额外 HTTPS 信任机制
- **仅安装版（NSIS）支持自动更新**；绿色版（便携版）只能提示手动下载新版

## 二、环境要求（构建机一次性准备）

| 项 | 位置 / 值 | 丢失后果 |
|---|---|---|
| 签名私钥 | `C:\Users\admin\.tauri\ms_manage.key`（密码由本地密码管理器保管） | **无法发新版**，客户端全部无法升级 |
| 签名公钥 | 已写入 `src-tauri/tauri.conf.json → plugins.updater.pubkey` | — |
| GitHub PAT | 环境变量 `GITHUB_TOKEN`（scope：`repo`） | 本地手动发布时无法上传 release |

### GitHub Actions（CI/CD 自动发布）额外需要

| Secret | 来源 | 用途 |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | 本地 `.tauri/ms_manage.key` 文件全文（粘贴时去掉换行） | CI 编译时签名 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码（**不要写进任何提交的文档或 issue**） | 同上 |

进入 https://github.com/jared73/ms_manage/settings/secrets/actions 添加。

备份私钥文件 + 密码到密码管理器；私钥 + 密码**严禁**提交到 git、issue、文档、截图。

## 三、发布流程

### 方案 A：CI 自动（推荐）

```bash
# 1. 改代码,本地验证
npm run lint
npm test

# 2. 提交
git add -A
git commit -m "feat: xxx"
git push github main

# 3. 打 tag 触发 release workflow
git tag v1.9.0
git push github v1.9.0
```

CI 自动完成:build → 签名 → 创建 GitHub Release → 上传 setup.exe / .sig / update.json / 便携版

### 方案 B：本地手动（紧急修复 / 测试用）

```powershell
# 一键发布(自动完成:bump 版本 → 前端构建 → Rust 构建 → 签名 → 打 zip → 上传 GitHub → 同步便携版)
$env:GITHUB_TOKEN = '<your-pat-with-repo-scope>'
pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1

# 可选参数
#   -NoBump    不升版本,用当前版本重发
#   -SkipBuild 跳过构建(改了 update.json 生成逻辑后快速重发)
```

需要 `GITHUB_TOKEN` 环境变量(`scope: repo`)。

## 四、端到端验证 checklist

- [ ] 客户端「设置 → 关于 → 检查更新」能拉到 `update.json`（无 CORS/网络问题）
- [ ] `update.json` 里的 `signature` 与 `.exe.sig` 一致（用 `minisign -V` 验签本地测试）
- [ ] `update.json` 的 `url` 下载下来的 `.exe` 与 release asset 一致
- [ ] 更新安装后版本号、commit hash 与 release 一致

## 五、踩坑记录

（按时间倒序；新坑追加到末尾）

### 14. 改用 GitHub Releases 当更新源（v1.9.0）

之前用 Gitea `Liuqingshan/ms-manage-releases` 仓做更新源。改用 GitHub `jared73/ms_manage` 同仓发布的优点:
- 一个仓全搞定(源码+发布),不用维护 ms-manage-releases 这个空仓
- GitHub 国内直连虽慢但稳定,SASE/Clash 代理 (127.0.0.1:7890) 一开就走
- 公开发布天然带 CDN 加速

注意点:
- Tauri config `dangerousInsecureTransportProtocol` 改成 false(GitHub 是 https)
- CI 必须在 `windows-latest` 跑(交叉编译到 Windows 没意义)
- 私有签名私钥放 GitHub Secrets,workflow 里读 env 注入

### 13. Tauri updater endpoint 用 {{current_version}} 占位符

endpoint 配置必须用 `{{current_version}}`（带 v 前缀拼接）:
- ✅ `releases/download/v{{current_version}}/update.json`  → `releases/download/v1.9.0/update.json`
- ❌ `releases/download/{{current_version}}/update.json`     → `releases/download/1.9.0/update.json`（找不到 tag）

### 12. update.json 签名是 base64 字符串,不能带 BOM

`[System.IO.File]::WriteAllText` 默认带 BOM(UTF-8 BOM `0xEF 0xBB 0xBF`),
Tauri updater 解析会失败 — 必须用 `UTF8Encoding($false)` 构造。

### 11. 私钥路径变量 vs 私钥字符串变量

tauri-cli 2.11 的 updater 签名阶段只认字符串变量 `TAURI_SIGNING_PRIVATE_KEY`,
不认路径变量 `TAURI_SIGNING_PRIVATE_KEY_PATH`。PowerShell 脚本里读文件内容再注入字符串。
(详见 `publish-release.ps1` 的注释)