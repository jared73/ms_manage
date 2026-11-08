# Magpie 维护闭环（MS Manage）

这是一套面向 **MS Manage** 的无人值守维护工作流，参考 [yetone/magpie](https://github.com/yetone/magpie) 的“三个循环 + 错题本 + 物理刹车”思路。

它不是“让 AI 随便改代码”，而是把重复工作拆成可验证、可回滚的闭环：

```
Windows 计划任务 / 手动触发
        │
        ▼
  run-loop.ps1  ──►  opencode run --agent build
        │                 │
        │                 ├── loops/01-issues.md      问题 / PR 巡逻、修复、合并
        │                 ├── loops/02-community.md   社区反馈、答疑、轻量修复
        │                 └── loops/03-nightly.md     清理、错题本、发版
        │
        ├── get-feedback.ps1     Gitea / GitHub 反馈汇总
        ├── check-gate.ps1       tsc / lint / i18n / vitest / build / cargo
        ├── create-pr.ps1        GitHub PR
        ├── wait-ci.ps1          等 GitHub Actions CI
        ├── merge-pr.ps1         自动合并（可选）
        └── release.ps1          bump → tag → push → GitHub Release
```

## 快速开始

### 0. 前置条件

- Windows + PowerShell 7（`pwsh`）
- `opencode` v2.2+，且已配置 `minimax-cn-coding-plan/MiniMax-M3.1-Flash` 模型
- Git 对 `origin`（Gitea）和 `github`（GitHub）都有推送权限
- 环境变量：
  - `GITHUB_TOKEN`：可选；未设置时会自动回退 Git Credential Manager 中 `https://github.com` 的凭据
  - `GITEA_TOKEN`：可选；只有在启用 Gitea 反馈源时才需要（当前默认关闭，见下文“反馈源”）
- Tauri 构建所需：Node 20+、Rust stable、Tauri CLI（`npm install` 后由 `@tauri-apps/cli` 提供）

### 1. 先看模板（不执行）

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/check-gate.ps1 -Quick
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/get-feedback.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/run-loop.ps1 -Loop nightly -DryRun
```

`run-loop.ps1 -DryRun` 只打印完整提示词，不会启动 opencode。

### 2. 手动跑一次

建议先在**独立的 clean checkout / worktree**里跑，避免和你的编辑器抢分支：

```powershell
git worktree add ..\ms_manage-magpie -b agent/magpie-main main
cd ..\ms_manage-magpie
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/run-loop.ps1 -Loop nightly
```

第一次建议只跑 `nightly`，观察 `automation/magpie/logs/` 和 `state/report-*.md` 是否正常。

### 3. 注册 Windows 计划任务

```powershell
# 先看会注册什么
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/install-scheduled-tasks.ps1

# 确认后注册
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/install-scheduled-tasks.ps1 -Register

# 需要时删除
pwsh -NoProfile -ExecutionPolicy Bypass -File automation/magpie/install-scheduled-tasks.ps1 -Remove
```

默认计划：

| 任务 | 频率 | 说明 |
|---|---|---|
| `MSManage-Magpie-Issues` | 每 120 分钟 | 处理 issue / PR |
| `MSManage-Magpie-Community` | 每 240 分钟 | 社区反馈、答疑 |
| `MSManage-Magpie-Nightly` | 每天 03:10 | 清理、错题本、发版 |

> 计划任务默认以当前用户交互式运行，所以需要保持用户登录；如果要开机无人值守运行，改成“无论用户是否登录”并在任务计划程序中保存凭据。

## 安全刹车

这套流程按“自动合并 + 自动发版”设计，但保留了 Magpie 强调的物理约束：

1. **clean tree**：`run-loop.ps1` 发现工作区有未提交改动直接退出，绝不覆盖人类工作。
2. **全局锁**：`state/run.lock` 防止三个循环同时改仓库。
3. **gate 先行**：合并、发版前必须通过 `check-gate.ps1`（tsc / lint / i18n / test / build / cargo）。
4. **受保护路径**：workflow、签名、release 脚本、`tauri.conf.json` updater 配置、`.env`、密钥等不允许自动改。
5. **单轮限额**：每次最多处理 `max_issues_per_run` 条 issue，防止一次性大面积改动。
6. **发版收敛**：默认 `release_mode = "nightly"`，把多个修复合成每天一次发布，避免 Magpie 早期“单日 170 个版本”的失控。
7. **失败不硬来**：gate/CI 连续失败 3 次就回滚并在 issue 说明，不无限重试。

## 目录说明

| 路径 | 作用 |
|---|---|
| `config.json` | 仓库、模型、限额、保护路径、gate 步骤、发版配置 |
| `run-loop.ps1` | 单个循环的入口（锁、日志、调用 opencode） |
| `loops/*.md` | 三个循环的提示词，可直接阅读和修改 |
| `get-feedback.ps1` | 汇总 Gitea / GitHub 的 open issue、PR |
| `check-gate.ps1` | 本地完整质量门禁 |
| `create-pr.ps1` / `wait-ci.ps1` / `merge-pr.ps1` | GitHub PR 流程 |
| `release.ps1` | 版本 bump、tag、push，触发 GitHub Actions Release |
| `install-scheduled-tasks.ps1` | 注册/删除 Windows 计划任务 |
| `state/` | 运行报告、prompt、gate 结果（已 gitignore） |
| `logs/` | opencode 输出日志（已 gitignore） |

## 配置说明

### 反馈源

`config.json → feedback.sources` 默认启用：

- `github`：`jared73/ms_manage`（公开仓库，走 Git Credential Manager 或 `GITHUB_TOKEN`）
- `gitea`：`http://www.73x.space:9100/Liuqingshan/ms_manage`（默认关闭）

Gitea 的 `GITEA_TOKEN` 需要有 **read:issue** 权限才能读取 issue 列表；当前用户的 token 能读仓库但不能读 issues（API 返回 403）。如果要在 Gitea 上收反馈，请到 Gitea 设置里重新生成带 issue 权限的 token，然后把 `enabled` 改成 `true`。目前建议以 GitHub Issues 为主。

### 发版模式

- `release_mode = "nightly"`（默认）：issue 循环只合并，nightly 统一发版。
- `release_mode = "immediate"`：每次有合并就发版，版本号增长快，不建议长期使用。
- 只有 `feat:` / `fix:` / `perf:` 提交才会触发自动发版；`docs:` / `chore:` / `test:` 不会。

### 自动化开关

- `auto_merge`：CI 全绿后自动合并 PR。
- `auto_release`：允许 nightly 自动 bump + tag + 发布。

第一次使用建议把 `auto_merge` / `auto_release` 设为 `false`，跑一周观察日志后再打开。模板默认按你选择的“自动合并 + 自动发版”开启，但可以在 `config.json` 里随时改回。

## 与 Trellis 的关系

仓库已有 `.trellis/` 工作流。`run-loop.ps1` 会在提示词开头注入维护上下文，并在有 `MAGPIE_LOOP` 环境变量时按 `docs/agent/CODE-STANDARDS.md` 执行。若你希望手动开的 opencode 会话也遵守同一套规则，把 `docs/agent/AGENTS-snippet.md` 的内容追加到项目根目录的 `AGENTS.md` 末尾（该文件被 `.gitignore` 忽略，模板不会直接改它）。

维护模式下：

- 跳过 Trellis 的交互式“是否创建任务”审批；
- 需要任务记录时直接创建 `.trellis/tasks/MM-DD-<slug>/`；
- 业务代码规范仍以 `.trellis/spec/` 为准。

普通对话和手动开发不受影响。

## 常见问题

**Q：`opencode run` 找不到模型？**  
先运行 `opencode models | Select-String minimax` 确认 `minimax-cn-coding-plan/MiniMax-M3.1-Flash` 可用；不可用就改 `config.json → agent.model`。

**Q：GitHub token 没配置？**  
脚本会尝试 Git Credential Manager。仍失败时，设置用户环境变量：
```powershell
setx GITHUB_TOKEN "<your-pat>"
```
然后重开终端。

**Q：计划任务没有运行？**  
检查任务计划程序里的 `MSManage-Magpie-*`，确认“上次运行结果”和 `automation/magpie/logs/`。Windows 需要用户登录；服务器场景请改用“无论用户是否登录”。

**Q：怎么暂停自动化？**  
`install-scheduled-tasks.ps1 -Remove`，或把 `config.automation.auto_merge` / `auto_release` 设为 `false`。
