# 归档记录

## 1. spec-generator（2026-06-28）

**归档日期**：2026-06-28
**归档原因**：架构决策 — pipeline 已切换到项目内专用版 `yto-generator-spec`

## 背景

原 `spec-generator` 与 `api-contract-generator`、`task-planner` 同批从通用架构模板 fork 而来，存在以下硬伤（详见 `.claude/memory/.qoder-skills-audit.md`）：

- 强绑 `LEASE_xxx` 业务状态、`ifid` 术语、`CustomeException`/`SiteNoFoundException`、虚构 Header（X-User-Code 等全仓库 0 匹配）
- README 索引与正文错位（03-state-machine.md 装的是 GWT 验收，05-acceptance.md 装的是状态机图）
- `metadata.references` 引用的 `.qoder/spec/`、`.qoder/rules/`、`.qoder/context/` 全是空目录（死链）
- 模板残留 `{初始状态}`、`{状态A}` 等占位符，违反自身"重要约束 #1"
- 没有任何 read-source 强制步骤，引导 AI 脑补字段名

## 架构决策

2026-06-28 用户明确：pipeline 不再引入 `spec-generator`，因为"架构优化了流程觉得生成的东西太多"。

具体落实：

| 改造项 | 旧 | 新 |
|---|---|---|
| analysis pipeline 阶段 2 | 调用 `spec-generator`（fork 模板） | 调用 `yto-generator-spec`（项目内专用版） |
| execute pipeline | 同上 | 不再调任何 spec，只读 plan.md |
| api-contract-generator | execute pipeline 仍引用 | execute pipeline 完全砍掉该环节（契约走 Swagger 注解 / DTO 源文件事实源） |

`spec-generator/` 整个目录本次会话首次落地，**git 无任何提交记录**，所以本次"归档"等于"丢弃"。

## 替代技能

- **需求规格说明书生成**：`.qoder/skills/yto-generator-spec/`（由 `yto-dev-analysis-pipeline` 阶段 2 调用）
- **后端契约事实源**：`docs/backend/PROJECT_BASELINE.md`（待建立，按 `preflight-project-baseline` 技能计划从 `@ApiModelProperty` / `@RequestMapping` / `DataResult.java` / `BasePageResponse.java` / `ReturnStatusEnums.java` 事实抽取）

## 外部引用清理

`context-generator` 与 `rules-generator` 多处将 `spec-generator` 列为下游可选技能（grep 共 13 处），其中 9 处本次会话也是 untracked 新增。归档后这些引用语义会失效，下一步需将它们切到 `yto-generator-spec` 或明确标 deprecated。

详见 `.claude/memory/.qoder-skills-audit.md`。

---

## 3. context-generator（2026-06-28）

**归档日期**：2026-06-28
**归档原因**：与 `spec-generator` 同批次通用 fork 模板；产物 `.qoder/context/` 无下游消费者

### 决策依据（grep 全仓验证）

| 验证维度 | 结果 |
|---|---|
| `context-generator` 在 pipeline SKILL.md 中作为 Read/调用源 | **0 匹配** |
| 外部被引用方 | 仅 `rules-generator` 系列 5 处（全部 untracked） |
| 自引用 | `context-generator/` 内部 12 处（自描述，删除随目录走） |
| 输出目录消费者 | `.qoder/context/` 在归档 §2 中已确认 0 读取 |

### 与 spec-generator / .qoder/context/ 的关系

| 项 | spec-generator | context-generator | .qoder/context/ |
|---|---|---|---|
| 来源 | 通用架构模板 fork | 通用架构模板 fork 生成 | context-generator 产物 |
| 状态 | 已归档到 `_archived/spec-generator/` | **本次归档到 `_archived/context-generator/`** | 已彻底删除（§2） |
| 归档原因 | pipeline 不再调用 | pipeline 不再调用，产物无消费者 | 目录无消费者 |
| git 提交记录 | 本次会话前为 0 | 本次会话前为 0 | 本次会话前为 0（全部 untracked） |

### 外部引用修复

`rules-generator` 系列 5 处引用 `context-generator`，已全部改写：

| 文件 | 改写 |
|---|---|
| `rules-generator/SKILL.md` §一核心定位表 | "不读 `.qoder/context/`" → "context 已归档，无需读取" |
| `rules-generator/SKILL.md` §七与其他 skill 的关系 | 整段重写为"`rules-generator` 自身独立" |
| `rules-generator/SKILL.md` §八图谱下游推荐 | 去掉 `context-generator`，footer 追加已归档列表 |
| `rules-generator/01-detect-project.md` §下一步建议 | 删除 "跑 context-generator" 一项 |
| `rules-generator/05-generate-rules.md` §下一步建议 | 删除 "跑 context-generator" 一项 |
| `rules-generator/examples/new-project-output.md` | 替换为已归档的 spec-generator |
| `rules-generator/examples/old-project-output.md` × 2 | 删除 "跑 context-generator" |

### 残留引用（低风险，本轮未处理）

| 引用位置 | 类型 |
|---|---|
| `api-contract-generator/SKILL.md` | 用户保留的技能，需后续清理 |
| `test-case-generator/SKILL.md` | 用户保留的技能，需后续清理 |

详见 `.claude/memory/.qoder-skills-audit.md`。

---

## 2. .qoder/context/ 目录（2026-06-28）

**归档日期**：2026-06-28
**归档原因**：pipeline 不读取，目录本身无下游消费者；同批次通用 fork 模板产物

### 决策依据（grep 全仓验证）

| 验证维度 | 结果 |
|---|---|
| `.qoder/context/` 在 pipeline SKILL.md 中作为 Read 源 | **0 匹配** |
| `yto-dev-analysis-pipeline` 读取路径 | `.requirements/{name}-{ver}/spec.md`（不读 context） |
| `yto-dev-execute-pipeline` 读取路径 | `.spec/00-base.md` + plan.md（不读 context） |
| `yto-project-map-init` 输出路径 | `.spec/{productLine}-business-map.md`（**同名 business-map，路径不同**，不读 context） |
| `yto-generator-spec` 读取路径 | `.spec/oc-project-business-map.md`（不读 context） |
| `yto-generator-plan` 读取路径 | `.spec/oc-business-*.md` + `.spec/oc-project-business-map.md`（不读 context） |

### 与 spec-generator 的关系

| 项 | spec-generator | .qoder/context/ |
|---|---|---|
| 来源 | 通用架构模板 fork | `context-generator` 通用架构模板 fork 生成 |
| 状态 | 已归档到 `_archived/spec-generator/` | 整个目录删除（彻底抛弃） |
| 归档原因 | pipeline 不再调用 | pipeline 不再读取，目录无消费者 |
| git 提交记录 | 本次会话前为 0 | 本次会话前为 0（全部 untracked） |

> 注：归档执行过程中误用 `rm -rf` 一次性删除，但因为 `.qoder/context/` 全部为本次会话 untracked 文件（git 从未持有），`git status` 无任何 deleted 记录，仓库状态干净。

### 替代事实源

| 旧产物（context/） | 新事实源（pipeline 真实读取） |
|---|---|
| `business-map-hbd.md` | `.spec/{productLine}-business-map.md`（由 `yto-project-map-init` 生成） |
| `tech-stack.md` | `.spec/hbd-spec-pom-base.md` + `always-on.md` 第 1-2 节 |
| `glossary.md` | `.spec/hbd-business-*.md`（由 `yto-generator-spec` 维护） |
| `module-responsibility.md` | `.spec/hbd-spec-application-responsibility.md` |
| `architecture.md` / `project-overview.md` | 无直接对应，参考现有同类代码 + `always-on.md` |
| `GENERATION-REPORT.md` | 本次会话元数据，无下游引用，直接抛弃 |

### 外部引用修复

`.qoder/rules/spec-reference.md` 共 4 处提及 `.qoder/context/`，已全部改写：
- 第 10 行回退路径：去掉 `.qoder/context/`，改走"现有同类代码 + always-on.md + java-class.md"
- 第 40 行加载顺序：去掉第一步 `.qoder/context/`
- 第 60 行 Step 2：去掉"读取 .qoder/context/ 相关模块知识"
- 第 106 行规范缺失回退：去掉"读取 .qoder/context/ 相关模块知识"，步骤号重排

### 残留引用（低风险，不在本轮修复范围）

`context-generator` / `rules-generator` / `api-contract-generator` / `test-case-generator` 4 个 fork 模板技能的 SKILL.md / 文件中仍有"spec-generator / context / rules"互相引用，全部为本次会话 untracked。下一步与"四件套通用 fork 模板统一归档"一并处理。

详见 `.claude/memory/.qoder-skills-audit.md`。