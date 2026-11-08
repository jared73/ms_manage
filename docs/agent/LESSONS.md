# LESSONS — 动态错题本

> 由 Magpie nightly 循环维护。每次夜审只记录“真实返工、不该合入、验证流于形式”的反例，不记流水账。
>
> 规则：
> - 每条事故写清楚：现象、根因、修复提交、可执行的预防规则。
> - `Seen` 是**不同日期**被再次捕获的次数；同一天重复出现只算 1 次。
> - `Seen 3×` 后从“观察中”移到“已升格”，并同步写入 `docs/agent/CODE-STANDARDS.md` 的“硬规则”一节。
> - 不要写“注意质量”“认真测试”这类无法执行的空话。

## 观察中

| ID | First seen | Seen | Area | 现象 | 根因 | 修复提交 | 预防规则 |
|---|---|---|---|---|---|---|---|
| L-001 | 2026-10-13 | 1× | release | `update.json` 写入后 Tauri updater 解析失败 | PowerShell 默认 UTF-8 带 BOM | `docs/RELEASE.md` 踩坑 #12 | 生成 update.json 必须使用 `UTF8Encoding($false)`，并在发布前做一次 JSON 解析验证 |
| L-002 | 2026-10-13 | 1× | release | tag 版本与 `package.json` 不一致导致 release workflow 失败 | 手工打 tag 前没有检查版本号 | `docs/RELEASE.md` 踩坑 #13 | 发版只通过 `release.ps1`，tag 前强制校验 `package.json` 版本 |
| L-003 | 2026-11-08 | 1× | automation | 自动化循环可能在用户未提交改动时切分支 | 运行器缺少 clean-tree 检查 | `automation/magpie/run-loop.ps1` | 每次运行前检查 `git status --porcelain`，非空即停止并写入报告 |
| L-004 | 2026-11-08 | 2× | release | v2.0.0 发布 11 天，客户端「检查更新」恒显示"已是最新版本"，**没有任何用户收到过更新提示** | updater endpoint 用 `v{{current_version}}` 拼 tag，而该占位符在运行时被替换成**客户端自己的版本号**（tauri-plugin-updater `updater.rs:553,570`），客户端只会请求「自己那一版」的清单；流水线又只往当版 release 写清单，于是 manifest 的 `version` 恒等于本地版本，插件默认比较 `release.version > current_version` 恒 false（`updater.rs:714`） | 未修复（`src-tauri/tauri.conf.json` 属受保护路径，需人工决策）；文档侧已更正 `docs/RELEASE.md` #13 / #15 | 发版后必须断言 manifest 的 `version` **大于**客户端版本（`curl -s <endpoint>` 取 `jq -r .version` 做数值比较）；只验证"能拉到"**不算**通过。endpoint 不得用客户端自身版本做占位符，静态清单固定指向 `releases/latest/download/update.json` |
| L-005 | 2026-11-08 | 2× | release | `dc848ba fix(updater): endpoint 改为固定 /update.json` 合入后 **2 分 21 秒**被 `da6af7d` 回滚，回滚提交**未写原因**；该问题至今未重新修复 | 修法 URL 用了不带 tag 的 `releases/download/update.json`，而 GitHub 与 Gitea **都不支持**无 tag 的 release 资产下载（两边实测 404）；改动合入前**没有实测目标 URL**，凭直觉改完就合 | `da6af7d`（回滚）；至今未修复 | 改 updater endpoint 这类"外部依赖型"配置，必须先用 `curl -o /dev/null -w "%{http_code}"` 实测目标 URL 返回 200 **且**内容可解析，再改配置再合入；回滚提交**必须**在信息里写明回滚原因 |
| L-006 | 2026-11-08 | 2× | ci | `main` 上 CI 从 2026-08-29 起连续 **6 次全红**（Backend job `cargo check` panic），**2.5 个月无人发现**，直到 2026-11-08 才被 issues 循环定位 | `tauri::generate_context!()` 在非 dev 模式要嵌入 `frontendDist`(`../dist`) 目录，目录不存在直接 panic；`dist/` 被 .gitignore 从不入库，CI 的 frontend job 与 backend job 之间也没有 artifact 传递；而本地 gate 的步骤顺序是 `frontend-build`(第 6 步) → `cargo-check`(第 7 步)，**复用了工作区里已存在的 `dist/`** → 本地永远绿、CI 永远红 | `02a2023 chore(ci): build frontend before cargo check`（此后 CI 连续 7 次绿） | 判断"CI 会不会绿"只能看 CI run 的 `conclusion`，本地 gate 结果**不能**替代；任何依赖构建产物的步骤必须在**干净 checkout** 里验证（CI 中 `cargo-*` 之前必须先产出 `dist/`）；本地 gate 若复用了工作区已有产物，报告里必须标注"该步骤在 CI 中不成立" |
| L-007 | 2026-11-08 | 2× | automation | 反馈源读取失败时 `get-feedback.ps1` 仍输出 `count: 0` + 退出码 0，issues/community 循环连续多轮报告"没有新反馈"（实为"读不到"）；`run-loop.sh` 的空收件箱快路径会据此**直接跳过整轮** | 源级异常被 `try/catch` 吞掉（`get-feedback.ps1:134-137`）只打一条 `Write-Warning`；而 `Write-Warning` 在 pwsh 7 里走 **stdout**，所以"stderr 为空"**不能**当成功判据（已实测：stdout 有 `WARNING:` 行、stderr 为 0 字节） | 未修复（`automation/magpie/**` 属受保护路径） | 读反馈源必须用**双判据**：stdout 能被严格 JSON 解析 **且** 全文无 `WARNING` 行；任何"没有反馈"的结论都要有独立旁证（认证请求 / `activities/feeds` 全量枚举）；`get-feedback.ps1` 后续应加 `errors` 字段并在失败时以非 0 退出码结束 |

## 已升格（硬规范）

以下规则来自 `docs/RELEASE.md` 的既有踩坑记录，已直接写入 `docs/agent/CODE-STANDARDS.md`：

- 发版前 CI 必须全绿；CI 红灯时不得打 tag。
- `update.json` 必须 UTF-8 无 BOM，且发布前验证可解析。
- tag 版本必须与 `package.json` 版本一致。
- 签名私钥、token、`.env` 不得出现在提交、日志、issue 或截图中。

## 归档

（暂无。已被证明无效或不再适用的条目移到这里，并注明原因。）
