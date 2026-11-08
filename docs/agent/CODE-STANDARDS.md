# MS Manage 自动化维护硬规范

> 本文件是 Magpie 维护循环的“硬规则”。业务代码的具体写法仍以 `.trellis/spec/` 下的分层规范为准；本文件只补充无人值守场景下必须执行的硬约束。

## 1. 合并与发版前必须跑的门槛

任何自动化改动在合并前必须运行：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/check-gate.ps1
```

它等价于 CI 的核心步骤：

- `git diff --check`
- `npx tsc --noEmit`
- `npm run lint`
- `npm run check:i18n`
- `npm test`
- `npm run build`
- `cargo check --message-format=short`
- `cargo test --lib --message-format=short`

任何一步失败都不得合并、不得打 tag。没有运行过的检查必须在报告里写 `Not run`，不能假装通过。

## 2. 测试的“物理证伪”

### Red Check（回退挂测）

修 bug 时必须先写测试，并证明：

1. 新测试在**修改前的旧代码**上会失败（真的 fail，不是跳过、不是编译错误）。
2. 失败信息明确指出坏掉的预期行为。
3. 把失败输出保存到 `automation/magpie/state/redcheck-<issue>.log`。

### Break Check（人为击穿）

对关键断言做一次“故意改坏”的检查：临时改坏分支或断言，确认测试能报警；恢复后测试必须重新变绿。无法做 Break Check 的场景要在 PR 里说明原因。

## 3. 改动边界

- 一个 issue / PR 只解决一个主题，禁止顺手重构、格式化全文件、重命名无关变量。
- 单个改动尽量不超过 `config.limits.max_files_per_issue` 个文件；超过就拆分或在报告里说明。
- 不新增第三方依赖。确实需要时，停下来写 `state/questions-*.md`，由人类审批。
- 不修改以下受保护路径：
  - `.github/workflows/**`、`.github/actions/**`
  - `scripts/publish-release.ps1`
  - `src-tauri/tauri.conf.json` 中的 updater / 签名配置
  - `*.key`、`*.pem`、`.env*`
  - `automation/magpie/**` 自身
- 不读取、不打印、不提交任何 token、私钥、`.env` 内容。

## 4. 前端约定

- React 18 + TypeScript + Vite + Tailwind。
- 组件按现有 `src/components`、`src/pages` 的组织方式拆分；不要引入新的状态库。
- 所有面向用户的文案必须同时更新 `src/i18n/zh.ts` 与 `src/i18n/en.ts`，然后跑 `npm run check:i18n`。
- IPC 契约以 `src/lib/ipc.ts` 为准；改 Rust command 时必须同步前端类型和调用方。
- 不改变既有视觉风格；UI/UX 大改必须由人类拍板。

## 5. Rust / Tauri 约定

- 保持 `src-tauri/src/cmd_*.rs` 按领域拆分；不要把所有命令堆进 `main.rs`。
- 新增/修改存储逻辑时补 `#[cfg(test)]` 单元测试；持久化操作要有失败路径测试。
- 不修改 `tauri.conf.json` 的权限、updater 公钥或签名配置。
- `cargo check` / `cargo test --lib` 必须通过；长耗时测试在报告里注明时间。

## 6. 提交与分支

- 分支命名：`agent/issue-<number>-<slug>`、`agent/feedback-<number>-<slug>`。
- 提交信息使用中文 conventional commit，参考最近 5 条提交的风格：
  - `fix: 修复 ...`
  - `feat: 增加 ...`
  - `test: 补充 ...`
  - `chore(release): bump x.y.z -> x.y+1.0`
- 不 amend 历史提交，不强推，不直接对 `main` 做 force push。

## 7. 发版

- 只通过 `automation/magpie/release.ps1 -Execute` 发版；不要手工改版本号或手工打 tag。
- 发版前必须跑完整 gate；tag 必须等于 `package.json` 版本，格式 `vX.Y.Z`。
- 发布后检查 GitHub Release 是否生成，把版本号和链接写进运行报告。

## 8. 硬规则（由错题本升格）

> 本节由 nightly 循环维护；同一教训出现 3 次后从 `LESSONS.md` 移到这里。

- 发版前 CI 必须全绿；不允许在 CI 红灯时打 tag。
- 同类 bug 必须全量排查兄弟模块，不能只修截图里的那一个入口。
- 修改数据格式、IPC 契约或 updater 配置时，必须同时验证升级路径和回滚路径。
- 任何“修好了”的结论都必须有真实运行证据；没有证据就写 `未验证`。
