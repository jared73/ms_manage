# Magpie Loop B · 社区反馈巡逻

你正在执行 MS Manage 的社区反馈循环。环境变量 `MAGPIE_LOOP=community`。
目标：对用户的使用问题、建议和零散反馈做出及时回应；能低成本修的 bug 直接进入修复流程，不能做的记录清楚。

## 0. 启动

1. 读 `automation/magpie/config.json`、`docs/agent/CODE-STANDARDS.md`、`docs/agent/LESSONS.md`。
2. 运行 `pwsh -NoProfile -File automation/magpie/get-feedback.ps1 -Json`，保存到 `state/inbox-community-<stamp>.json`。
3. 只看最近 24 小时有更新的 issue / PR；没有新反馈就写一份空的运行报告并结束。

## 1. 分类

- **使用问题**：如何配置 MCP、Claude Desktop、token、更新失败等。直接用现有文档回答，附上 README 的对应章节或 `docs/RELEASE.md`。
- **可复现 bug**：进入下面的“修复流程”。
- **feature 建议**：记录到 `state/feature-requests-<stamp>.md`，评估影响面和实现成本；只有“小、明确、不新增依赖”的才直接做，否则只回复。
- **广告 / 中转站 / 无关内容**：统一礼貌回复，不修改代码。
- **需要产品决策 / UI 大改 / 新依赖**：记录到 `state/questions-<stamp>.md`，回复用户“已记录，需要评估”，不要擅自实现。

## 2. 修复流程（与 Loop A 相同的硬规则）

1. 从 `main` 拉最新，创建 `agent/feedback-<number>-<slug>`。
2. 先写失败测试（Red Check），保存失败输出到 `state/redcheck-<number>.log`。
3. 最小修复，遵循 `docs/agent/CODE-STANDARDS.md`。
4. 运行 `pwsh -File automation/magpie/check-gate.ps1`，必须全绿。
5. 提交、push 到 `origin` 和 `github`、开 PR、等 CI、按配置合并（脚本同 Loop A）。
6. 回到原 issue / 评论下回复修复结果。

## 3. 回复规范

- 中文，简洁，先给结论再给步骤。
- 涉及命令时给可复制的完整命令。
- 不要承诺未经验证的功能；没跑过的检查写“未验证”。
- 不要把内部实现细节、token、路径泄露给用户。
- 回复后把回复内容记录到运行报告。

## 4. 结束

写 `automation/magpie/state/report-community-<stamp>.md`：

- 回复了哪些用户 / issue
- 哪些转成了代码修复
- 哪些记录为 feature request 或待决策
- 需要人类跟进的事项
