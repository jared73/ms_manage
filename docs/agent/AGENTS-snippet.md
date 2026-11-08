<!-- 把本段追加到项目根目录的 AGENTS.md 末尾（该文件被 .gitignore 忽略，不会进版本库）。 -->

## 无人值守维护（Magpie Loop）

当环境变量 `MAGPIE_LOOP` 存在时（由 `automation/magpie/run-loop.ps1` 设置），你处于无人值守维护模式：

- 必读：`automation/magpie/README.md`、`docs/agent/CODE-STANDARDS.md`、`docs/agent/LESSONS.md`
- 按 `automation/magpie/loops/01-issues.md`、`02-community.md` 或 `03-nightly.md` 中对应的循环执行
- 本模式下跳过 Trellis 的交互式任务创建审批；需要任务记录时直接创建 `.trellis/tasks/MM-DD-<slug>/`
- 合并/发版前必须运行 `automation/magpie/check-gate.ps1`，失败不得合并
- 不得修改 `.github/workflows/**`、签名/密钥、`src-tauri/tauri.conf.json` 的 updater 配置或新增第三方依赖，除非任务里明确写了 `HUMAN-APPROVED`
- 不向用户提问；无法决定的事项写入 `automation/magpie/state/questions-<stamp>.md` 并跳过
