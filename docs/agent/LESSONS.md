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

## 已升格（硬规范）

以下规则来自 `docs/RELEASE.md` 的既有踩坑记录，已直接写入 `docs/agent/CODE-STANDARDS.md`：

- 发版前 CI 必须全绿；CI 红灯时不得打 tag。
- `update.json` 必须 UTF-8 无 BOM，且发布前验证可解析。
- tag 版本必须与 `package.json` 版本一致。
- 签名私钥、token、`.env` 不得出现在提交、日志、issue 或截图中。

## 归档

（暂无。已被证明无效或不再适用的条目移到这里，并注明原因。）
