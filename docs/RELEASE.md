# MS Manage 发布手册（含踩坑记录）

本文件记录 v1.7.0 起确立的发布流程、环境要求，以及实现自动更新期间踩过的所有坑。
**新坑务必补记到「踩坑记录」末尾**。

## 一、自动更新架构

```
客户端（Tauri updater 插件）
  └─ 设置 → 关于 → 「检查更新」/ 安装版自检
       └─ GET https://github.com/jared73/ms_manage/releases/
             latest/download/update.json                  ← 更新清单（JSON）
              └─ 清单里 signature + url 指向安装包 → 下载 → 验签 → 静默安装 → 重启
```

> ⚠️ **endpoint 里不能用 `{{current_version}}` 拼 tag**。
> 该占位符在运行时会被替换成**客户端自己正在运行的版本号**，于是客户端永远只会请求
> 「自己那一版」的清单，`version` 不会比本地新 → **自动更新对所有用户恒等于关闭**。
> 静态清单必须指向一个与客户端版本无关的固定资产（`releases/latest/download/update.json`）。
> 详见「五、踩坑记录」#13 / #15。

- **更新源**：GitHub Releases（`jared73/ms_manage`，公开仓库）
- **安全模型**：安装包用 minisign 私钥签名，客户端内置公钥验签。即使 GitHub 被攻破/被篡改，
  验签失败会自动中止 —— 不需要额外 HTTPS 信任机制
- **仅安装版（NSIS）支持自动更新**；绿色版（便携版）只能提示手动下载新版

> **当前状态（2026-11-09）**：`src-tauri/tauri.conf.json` 里仍是
> `releases/download/v{{current_version}}/update.json`，也就是上面被判定为错误的写法，
> **自动更新目前对所有用户无效**（v2.0.0 已发布，用户从未收到过更新提示）。
> 修复要改 `tauri.conf.json`（受保护路径）并确定唯一更新源，
> 已记录在 Magpie 维护循环的待决事项（`state/questions-*.md`，该目录不入版本库），待人工处理。

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
- [ ] **`update.json` 里的 `version` 确实比客户端本地版本新**（数值比较，不是"字符串看着更大"）
      —— 例：`curl -s <endpoint> | jq -r .version` 与客户端版本号比大小。
      只验证"能拉到"是**不够**的：这正是踩坑 #13 的漏网之处
      （v2.0.0 的 manifest 能正常拉到，但它的 `version` 恰好等于客户端版本，于是永远"已是最新"）
- [ ] 用**上一个已发布版本**的客户端 URL 实测一次：`.../download/v<旧版本>/update.json`
      拿到的 manifest `version` 必须 **> 旧版本号**。
      （当前流水线只往当版 release 写清单，这一项**必然不通过** —— 见踩坑 #15）

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

### 13. Tauri updater endpoint 不能用 `{{current_version}}` 拼 tag

`{{current_version}}` 是 Tauri 插件的**占位符**，运行时会被替换成**客户端自己正在运行的版本号**
（`tauri-plugin-updater` 的 `updater.rs`：`let version = self.current_version.to_string();`
→ `.replace("{{current_version}}", &encoded_version)`）。

所以 endpoint 不能写成"用客户端版本拼 tag"：

- ❌ `releases/download/v{{current_version}}/update.json` → 客户端 v1.9.0 去请求
  `.../download/v1.9.0/update.json`，拿回来的是**自己那一版**的清单，
  `version == current_version`；插件默认比较 `release.version > current_version` 恒为 false
  → **永远提示"已是最新版本"，自动更新对所有用户恒等于关闭**
- ❌ `releases/download/{{current_version}}/update.json` → 请求 `.../download/1.9.0/update.json`，
  同样找不到 tag
- ✅ 静态清单必须指向一个**固定的、与客户端版本无关的资产**：
  `releases/latest/download/update.json`（GitHub 的 `latest` 指向最新 release，不含预发布）

> **平台限制：GitHub 和 Gitea 都不支持"不带 tag 的 release 资产下载"**，
> `releases/download/update.json`（缺 tag）在两个平台都是 404。
> 2026-08-09 的 `dc848ba fix(updater): endpoint 改为固定 /update.json` 就是踩了这个坑：
> 思路对、URL 形态错，**合入 2 分 21 秒后被 `da6af7d` 回滚**，而且回滚提交没写原因。

### 12. update.json 签名是 base64 字符串,不能带 BOM

`[System.IO.File]::WriteAllText` 默认带 BOM(UTF-8 BOM `0xEF 0xBB 0xBF`),
Tauri updater 解析会失败 — 必须用 `UTF8Encoding($false)` 构造。

### 11. 私钥路径变量 vs 私钥字符串变量

tauri-cli 2.11 的 updater 签名阶段只认字符串变量 `TAURI_SIGNING_PRIVATE_KEY`,
不认路径变量 `TAURI_SIGNING_PRIVATE_KEY_PATH`。PowerShell 脚本里读文件内容再注入字符串。
(详见 `publish-release.ps1` 的注释)

### 15. 发布流水线只写「当版」清单 → 自动更新对所有用户恒等于关闭

- **现象**：v2.0.0 于 2026-08-29 发布后，客户端「检查更新」永远显示"已是最新版本"，
  没有任何用户收到过更新提示；`v2.0.0/update.json` 与 `latest/download/update.json`
  的 `version` 都是 `2.0.0`，正好等于客户端自己的版本号。
- **根因（两处叠加，缺一不可）**：
  1. 端点用 `v{{current_version}}` 拼 tag（见 #13）→ 客户端只请求「自己那一版」的清单；
  2. 流水线（`scripts/publish-release.ps1` / `.github/workflows/release.yml`）**只往当版 release
     写一份 `update.json`**，从不维护"上一版 release 的清单" → 缺口在下一个版本**必然复发**。
- **为什么之前没被发现**：第四节 checklist 的 4 条断言**全是"能取到"**，
  没有一条验证"manifest 的 `version` 比本地新"，恰好漏掉本 bug 的形态。
- **验证方式（发布后必做）**：拿**上一个版本**的客户端 URL 实测 ——
  `curl -s .../download/v<旧版本>/update.json | jq -r .version` 的结果必须 **> 旧版本号**。
- **修复方向（两者都需人工决策，涉及受保护路径）**：
  - 端点改成 `releases/latest/download/update.json`
    ——**不需要改流水线**，新端点与"当版清单"完全兼容，已实测该 URL 返回 200 + 合法 manifest；
  - 或由流水线额外维护"上一版清单"（改 `publish-release.ps1` / `release.yml`）。
- **注意**：只改本文档不改 `tauri.conf.json` 会造成"文档说 A、代码是 B"的分裂状态；
  正确顺序是**先改 endpoint，再同步文档**。
