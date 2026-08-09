# MS Manage 发布手册（含踩坑记录）

本文件记录 v1.7.0 起确立的发布流程、环境要求，以及实现自动更新期间踩过的所有坑。
**新坑务必补记到「踩坑记录」末尾**。

## 一、自动更新架构

```
客户端（Tauri updater 插件）
  └─ 设置 → 关于 → 「检查更新」/ 安装版自检
       └─ GET http://www.73x.space:9100/Liuqingshan/ms-manage-releases/
             releases/download/v{当前版本}/update.json   ← 更新清单（JSON）
              └─ 清单里 signature + url 指向安装包 → 下载 → 验签 → 静默安装 → 重启
```

- **更新源**：Gitea 公开仓库 `Liuqingshan/ms-manage-releases`（**不含源码**，只放产物）
- **安全模型**：安装包用 minisign 私钥签名，客户端内置公钥验签。即使更新源被攻破/被篡改，
  验签失败会自动中止 —— 所以 http 明文传输也是可接受的（`dangerousInsecureTransportProtocol: true`）
- **仅安装版（NSIS）支持自动更新**；绿色版（便携版）只能提示手动下载新版

## 二、环境要求（构建机一次性准备）

| 项 | 位置 / 值 | 丢失后果 |
|---|---|---|
| 签名私钥 | `C:\Users\admin\.tauri\ms_manage.key`（密码 `msm-2026-key`） | **无法发新版**，客户端全部无法升级 |
| 签名公钥 | 已写入 `src-tauri/tauri.conf.json → plugins.updater.pubkey` | — |
| Gitea Token | 环境变量 `GITEA_TOKEN`（scope：`write:repository` + `write:user`） | 无法上传 release |
| 运行环境 | **PowerShell 7+（pwsh）**，脚本在 PS 5.1 下有原生命令引号缺陷 | 上传必失败 |

备份私钥文件 + 密码到密码管理器；私钥**严禁**提交到 git。

## 三、发布流程

```powershell
# 1. 改代码，本地验证（lint / test / tsc）
npm run lint
npm run test

# 2. 一键发布（自动完成：bump 版本 → 前端构建 → Rust 构建 → 签名 → 打 zip → 上传 Gitea → 同步便携版）
pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1

# 可选参数
#   -NoBump    不升版本，用当前版本重发
#   -SkipBuild 跳过构建（改了 update.json 生成逻辑后快速重发）
```

脚本执行内容（`scripts/publish-release.ps1`）：
1. bump minor 版本（1.7.0 → 1.8.0，同步 package.json / Cargo.toml / tauri.conf.json）
2. `npm run tauri:build`（含 `tsc --noEmit` 类型检查；用 `TAURI_SIGNING_PRIVATE_KEY` 字符串变量签名）
3. `scripts/build-zip.ps1` 打绿色版 zip
4. 创建/更新 Gitea Release `vX.Y.Z`，上传 4 个附件：
   - `MS Manage_X.Y.Z_x64-setup.exe`（安装版，自动更新的安装介质）
   - `MS Manage_X.Y.Z_x64-setup.exe.sig`（安装包签名）
   - `update.json`（**自动更新清单**，客户端拉的就是它）
   - `MS-Manage-vX.Y.Z-win-x64.zip`（绿色版）
5. 同步 `dist-portable/MS Manage.exe`

### 发布后验证（必做）

```powershell
# 清单可匿名访问
curl http://www.73x.space:9100/Liuqingshan/ms-manage-releases/releases/download/vX.Y.Z/update.json

# 安装包可匿名下载（注意空格要 %20 编码）
curl -o NUL -w "%{http_code}" "http://www.73x.space:9100/Liuqingshan/ms-manage-releases/releases/download/vX.Y.Z/MS%20Manage_X.Y.Z_x64-setup.exe"

# 客户端验证：设置 → 关于 → 检查更新
#   当前版本 == 线上版本 → 提示"已是最新版本"
#   当前版本 < 线上版本   → 弹"发现新版本" → 下载 → 重启安装
```

发布后顺手提交 git（版本文件 + CHANGELOG + 发布文档更新）。

## 四、踩坑记录（按时间顺序）

### 1. Tauri updater 要求 HTTPS（启动闪退 0xC0000409）
**症状**：程序双击后瞬间退出（黑框一闪 / 无窗口）。退出码 `0xC0000409`（STATUS_STACK_BUFFER_OVERRUN，
Rust panic=abort 的 fail-fast）。
**根因**：`plugins.updater.endpoints` 是 `http://`，插件初始化直接 panic：
`The configured updater endpoint must use a secure protocol like https`
**修复**：`tauri.conf.json` 加 `"dangerousInsecureTransportProtocol": true`（自建 http 服务器专用，
安全性由签名校验兜底）。
**排查方法**：GUI 程序没有控制台，用 `Start-Process -RedirectStandardError` 抓 panic 输出。

### 2. capabilities ACL 拒绝 updater 命令
**症状**：点「检查更新」→ `Command plugin:updater|check not allowed by ACL`
**根因**：Tauri 2 权限系统默认拒绝插件命令。
**修复**：`src-tauri/capabilities/default.json` 的 permissions 加 `"updater:default"`。

### 3. endpoint 占位符 {{version}} 无效
**症状**：`Could not fetch a valid release JSON from the remote`
**根因**：updater 插件只替换 `{{current_version}}`、`{{target}}`、`{{arch}}`、`{{bundle_type}}`，
`{{version}}` 原样保留 → 请求 `v{{version}}/update.json` → 404。
**修复**：endpoint 用 `v{{current_version}}/update.json`。

### 4. 私有仓库匿名访问全部 404
**症状**：update.json / exe 匿名 curl 全部 404（带 token 则 200）。
**根因**：Gitea 对私有仓库的匿名访问伪装成 404。
**修复**：建**独立公开仓库** `ms-manage-releases`（不装源码，只放产物）。

### 5. Gitea 空仓库不能创建 Release
**症状**：API 创建 release 报 `repo is empty`。
**修复**：先往默认分支提交一个 README（API：`PUT /repos/{owner}/{repo}/contents/README.md`）。

### 6. 上传附件失败：PS 5.1 原生命命令引号缺陷
**症状**：`curl: URL malformed` / 上传失败，且 curl 返回码诡异。
**根因**：PowerShell 5.1 对含空格参数的原生命令引号重建有缺陷。
**修复**：发布脚本必须用 **pwsh 7** 运行；路径/URL 中的空格统一用 `[uri]::EscapeDataString` 编码。

### 7. Gitea 删除附件 API 路径
**坑**：删除附件必须用 `DELETE /repos/{owner}/{repo}/releases/{release_id}/assets/{asset_id}`
（少写 release_id 会 404）。
**坑**：同名附件存在多个时 `Where-Object` 匹配到数组，`"$($obj.id)"` 展开成 `"22 23"` 导致 URL 畸形 ——
必须循环删除。

### 8. Gitea Token 权限
**坑**：创建仓库需要 `write:user` scope；只给 `write:repository` 会 403
（`token does not have at least one of required scope(s), required=[write:user]`）。

### 9. tauri signer 非交互生成
**坑**：`tauri signer generate` 直接跑会挂起等交互输入。
**修复**：`npm run tauri -- signer generate --ci --force -p <密码>`。
**坑**：签名阶段只认**字符串变量** `TAURI_SIGNING_PRIVATE_KEY`（文件路径变量
`TAURI_SIGNING_PRIVATE_KEY_PATH` 不生效）——脚本里读文件内容注入字符串。

### 10. GitHub Releases 的坑（已弃用方案，留档）
- 大陆直连 `objects.githubusercontent.com`（GitHub 资产 CDN）极不稳定，下载经常挂起 —— 这是当初放弃 GitHub 的主因。
- 资产文件名中的空格会被 GitHub 自动替换成点（`MS Manage_...exe` → `MS.Manage_...exe`），
  update.json 的 URL 必须用转换后的名字，否则 404。
- 删除仓库需要 `delete_repo` scope（`public_repo` 不够）。

### 11. 发布流程自动化提醒
- `update.json` 的 `pub_date` 用 UTC ISO8601（`Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')`）。
- 每次发布 `update.json` 的 signature 都会变（含时间戳），脚本每次发布都会重新生成，无需手工维护。
- 版本号单一来源：`package.json`（`scripts/bump-version.cjs` 同步到 Cargo.toml / tauri.conf.json）。

### 12. update.json signature 字段：必须用 .sig 原始文本，禁止二次 base64
**症状**：客户端「检查更新」弹「发现新版本」→ 点「立即更新」→ toast `更新失败: Invalid encoding in minisign data`。
**根因**：Tauri signer 生成的 `.sig` 文件**本身就是 base64 编码的 minisign 签名包**（1 行文本）。如果脚本里写成
`readFileSync(sigPath).toString("base64")`（对 base64 文本**再 base64 一次**）→ 双重编码 → Tauri 端 `minisign::Signature::decode` 失败。
**修复**：直接读 .sig 原始文本：`$sigText = (Get-Content $sig -Raw).Trim()`（`publish-release.ps1:117` 就是这么写的）。
**排查**：上传后 `curl .../vX.Y.Z/update.json` 看 signature 是否与本地 `.sig` 内容一致（应完全相等，长度 420 左右）。

### 13. endpoint 占位符保持 `v{{current_version}}/update.json`，不要改成固定路径
**背景**：曾有人（AI 助手）把 endpoint 从 `v{{current_version}}/update.json` 改成固定 `/update.json`（因为当时 dev binary
拉 `v1.7.0/update.json` 404 以为目录错配）。**这是误判**——Gitea Release 附件 URL 天然带 `v{tag}` 前缀，
`{{current_version}}` 占位符由 Tauri 客户端替换成当前版本号，恰好匹配 Gitea 的 release 目录结构。
**教训**：**不要改 endpoint**。旧版本客户端拉 `v{旧版本}/update.json` 是**预期行为**——Gitea 每个 release
都有自己目录下的 update.json。如果某个旧版本目录没 manifest（404），是**没给那个旧版本发布过 update.json**
（比如 v1.7.0 release 的 update.json 要**更新**为新版本 manifest 指向 1.8.0），不是 endpoint 的问题。

### 14. 完整发布流程（照这个走，别再手动）
```powershell
# 一键发布（推荐，覆盖：bump → 构建 → 签名 → 打zip → 上传 Gitea → 同步便携版）
pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1

# 参数:
#   -NoBump     不升版本，用当前版本重发（改了 update.json 逻辑后快速重发）
#   -SkipBuild  跳过构建（产物已存在时直接重发）
```
**手动兜底路径**（publish-release.ps1 不可用时才用，不推荐）：
1. `node scripts/bump-version.cjs` → 版本 1.8.0
2. `npm run tauri:build:nobump` → 产物 `src-tauri/target/release/bundle/nsis/MS Manage_1.8.0_x64-setup.exe` + `.sig`
3. 生成 update.json（**signature 必须取 .sig 原始文本**）并上传为 Gitea v1.8.0 release 附件
4. 记得**同步更新所有旧版本 release 的 update.json**（如 v1.7.0 的 update.json 要指向 1.8.0，让老用户能升级）
