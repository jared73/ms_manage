# Magpie Loop A · Issue / PR 巡逻

你正在执行 MS Manage 的无人值守维护循环。环境变量 `MAGPIE_LOOP=issues`。
目标：把用户报告的问题变成经过验证的修复并安全合并。不追求数量，追求零回归。

## 0. 启动

1. 读 `automation/magpie/config.json`、`docs/agent/CODE-STANDARDS.md`、`docs/agent/LESSONS.md`。
2. 运行：

   ```powershell
   pwsh -NoProfile -File automation/magpie/get-feedback.ps1 -Json
   ```

   把输出保存到 `automation/magpie/state/inbox-<stamp>.json` 后再读取。不要直接把大段 JSON 重复输出到对话里。
3. 如果 `git status --porcelain` 非空，停止并在报告中说明，绝对不要碰用户未提交的改动。
4. 如果收件箱 `count = 0`：写一份简短报告（无 issue、无动作、未跑 gate），立即结束。不要额外审计 GitHub Actions 历史、PR 列表或代码库；CI 健康检查属于 nightly 循环的职责。

## 1. 选择任务

- 优先级：崩溃 / 数据损坏 / 更新失败 > 可复现 bug > 小而明确的 feature。
- 跳过：广告、中转站推广、无关内容、需要产品决策、需要大改 UI、需要新增第三方依赖的请求。
- 对跳过项也要在 issue 下回复，说明原因；不要沉默。
- 每次最多处理 `config.limits.max_issues_per_run` 条。
- 已带 `agent:working` 标签或 24 小时内已经处理过的 issue 直接跳过。

## 2. 单条 issue 的处理流程

1. 把 triage 结论写到 `automation/magpie/state/triage-<stamp>.md`：编号、标题、判断、理由。
2. 切到 `main`，运行 `git pull --ff-only github main`。
3. 创建分支：`agent/issue-<number>-<short-slug>`。
4. **先复现**：
   - 在旧代码上写一个会失败的测试，把失败输出保存到 `automation/magpie/state/redcheck-<number>.log`。
   - 如果无法复现，在 issue 下回复所需信息（版本、复现步骤、日志、Windows 版本），加 `needs-info` 标签，结束该条。不要猜着改。
5. **修复**：
   - 最小改动，只解决这个 issue，不顺手重构。
   - 遵循 `docs/agent/CODE-STANDARDS.md`。
   - 不修改受保护路径，不新增第三方依赖。
   - UI 文案同时更新 `src/i18n/zh.ts` 和 `src/i18n/en.ts`。
6. **验证**：运行 `pwsh -NoProfile -File automation/magpie/check-gate.ps1`，必须全绿。
7. **提交**：中文 conventional commit，风格参考最近 5 条提交；提交信息里写 `fix: ...` 或 `feat: ...`。
8. **推送分支**：`git push origin <branch>`。
9. **开 PR**：

   ```powershell
   pwsh -NoProfile -File automation/magpie/create-pr.ps1 -Head <branch> -Title "<title>" -Body "<body>"
   ```

10. **等 CI**：

    ```powershell
    pwsh -NoProfile -File automation/magpie/wait-ci.ps1 -Sha <commit-sha>
    ```

    如果失败，修复后重试，最多 `config.limits.max_ci_retries` 次；仍失败则回滚分支并在 issue 说明。
11. **合并**（仅当 `config.automation.auto_merge=true` 且 CI 全绿）：

    ```powershell
    pwsh -NoProfile -File automation/magpie/merge-pr.ps1 -Number <pr-number>
    ```

12. 在 issue 下回复：修复说明 + PR 链接 + commit hash。

## 3. 发版

- 如果 `config.automation.release_mode = "immediate"`，并且本次有合并：运行 `pwsh -File automation/magpie/release.ps1 -Execute`。
- 如果 `config.automation.release_mode = "nightly"`，不要在本循环发版；由 nightly 循环统一处理。这样可以把多个修复合并成一次发布，避免一天几十个版本。

## 4. 结束

写 `automation/magpie/state/report-issues-<stamp>.md`，包含：

- 处理了哪些 issue：编号、结论、分支、PR、commit
- 跳过了哪些：原因
- gate / CI 结果
- 需要人类介入的事项（改 workflow、签名、依赖、产品决策等）

最后用一句话总结本次运行，不要输出无关的完整日志。
