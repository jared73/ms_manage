# Magpie Loop C · 夜间复盘 / 错题本 / 发版

你正在执行 MS Manage 的夜间循环。环境变量 `MAGPIE_LOOP=nightly`。
目标：让系统收敛，而不是让代码量膨胀。清理、复盘、把教训变成规则；只有在有真实用户价值时才发版。

## 0. 启动

1. 读 `docs/agent/CODE-STANDARDS.md`、`docs/agent/LESSONS.md`、`automation/magpie/config.json`。
2. 查看最近 24 小时的提交：

   ```powershell
   git log --since="24 hours ago" --pretty=format:"%h %ad %s" --date=short
   ```

3. 查看最近 24 小时合并的 PR / 处理的 issue（从 `state/report-*.md` 和 git log 里汇总）。

## 1. 动态错题本（最重要）

对最近 24 小时的每个修复问三个问题：

1. 这个修复后来是不是又返工了？
2. 这个改动是不是根本不该合入？
3. 这个验证是不是流于形式（测试没真挂、只跑了 mock）？

发现典型反例时，在 `docs/agent/LESSONS.md` 的“观察中”表格新增一行：

- ID：`L-<next>`
- First seen：今天日期
- Seen：`1×`
- Area：`ci` / `release` / `frontend` / `rust` / `i18n` / `docs`
- 现象 / 根因 / 修复提交 / 预防规则：写具体，不写“注意质量”这种空话

如果同一个教训在 **3 个不同日期** 的夜审中重复出现（`Seen 3×`），把它从“观察中”移到“已升格”，并同步写进 `docs/agent/CODE-STANDARDS.md` 的“硬规则”一节。规则要能被执行，不要写口号。

## 2. 低风险清理

- 只做有测试覆盖、行为不变的小清理：死代码、重复工具函数、明显的日志/注释问题。
- 不做大重构、不换框架、不升级大版本依赖。
- 任何清理都必须跑 `pwsh -File automation/magpie/check-gate.ps1`。
- 如果没有值得做的清理，明确写“本轮无清理”，不要为了提交而提交。

## 3. 文档一致性

抽查 README / docs/RELEASE.md / package.json 里的命令和版本是否一致；发现小错直接改，改完跑 gate。

## 4. 发版（有真实变更才发）

1. 读取最新 tag：

   ```powershell
   git describe --tags --abbrev=0
   ```

2. 查看未发布提交：`git log <latest-tag>..main --oneline`。
3. 只有存在 `feat:` / `fix:` / `perf:` 提交时才发版；只有文档/注释/测试改动时不发版。
4. 运行 dry-run 确认版本计划：

   ```powershell
   pwsh -File automation/magpie/release.ps1
   ```

5. 确认无误后执行：

   ```powershell
   pwsh -File automation/magpie/release.ps1 -Execute
   ```

   该脚本会再跑一次完整 gate、bump 版本、打 tag、推送 `github` 和 `origin`，触发 `.github/workflows/release.yml`。

6. 如果 gate 失败或推送失败：不要强推、不要绕过，写报告并退出。

## 5. 结束

写 `automation/magpie/state/report-nightly-<stamp>.md`：

- 错题本新增/升格了哪些条目
- 清理和文档修改
- 发版结果（版本号 / release URL / 失败原因）
- 下一天建议优先处理什么

最后用一句话总结。不要输出完整日志。
