---
name: context-generator
description: 扫描项目并生成 .qoder/context/ 配置。新项目用默认模板+1 个问题；老项目基于现有材料推断+质问+生成。
---

# context-generator

> **这个 skill 解决一件事**：让任何项目（新或老）在 5–60 分钟内拥有一套专属的 context。

## 一、核心定位

| 维度 | 内容 |
|------|------|
| **输入** | 一个项目（新/老）+ 项目材料（README/架构文档/产品文档） |
| **输出** | `.qoder/context/` 下的 6 个 context 文件 + 1 份生成报告 |
| **关键动作** | 收集材料 → 推断 → **质问** → 生成 |
| **不依赖** | 不读 `.spec/`、不读 `.qoder/rules/`、**不读项目代码**（与 rules-generator 相反） |

**与 rules-generator 的核心区别**：

| 维度 | rules-generator | context-generator |
|------|----------------|-------------------|
| 输入 | **项目代码** | **项目材料**（文档） |
| 输出 | `.qoder/rules/` | `.qoder/context/` |
| 关注 | 代码约束（怎么做） | 业务背景（是什么） |
| 质问 | 代码风格不一致 | 业务边界模糊 |

**设计原则**：
- **context 是叙述性的**（给 AI 读），允许模糊
- **不读代码**——context 关注"业务"，不关注"实现"
- **不读 rules**——避免循环依赖
- **不读 spec**——spec 是后续生成，不是输入

## 二、何时使用

### 触发场景
- ✅ 给新项目生成 context
- ✅ 给老项目补全 context
- ✅ 推广 SDD 到新项目（和 rules-generator 配对使用）
- ✅ context 文件缺失/过期时重新生成

### 不适用
- ❌ 已有完整 context 的项目（直接编辑现有文件）
- ❌ 想改某条 context（直接编辑）
- ❌ 非 Java 项目（当前主要适配 Java 后端，但 context 本身语言无关）

## 三、6 个 context 文件

| 文件 | 作用 | 何时被 AI 读 |
|------|------|------------|
| `project-overview.md` | 项目是什么、解决什么问题 | 任何任务的第一步 |
| `architecture.md` | 技术架构、模块依赖 | 涉及架构/选型决策时 |
| `module-responsibility.md` | 模块/产品线划分与职责 | 涉及模块归属判断时 |
| `business-map-{X}.md` | 各产品线业务地图 | 涉及具体产品线时 |
| `tech-stack.md` | 技术栈与版本 | 选型/版本决策时 |
| `glossary.md` | 项目术语表 | 理解业务术语时 |

**说明**：business-map 按产品线拆分，**每个产品线一份**。例如：
- `business-map-lms.md`（LMS 产品线）
- `business-map-oc.md`（OC 产品线）
- `business-map-cmd.md`（CMD 产品线）

## 四、流程总览

```
[项目 + 项目材料]
    │
    ▼
┌─────────────────┐
│ 01 项目类型识别   │  → 新项目 vs 老项目
└─────────────────┘
    │
    ├── 新项目 ──▶ 1 个轻量质问 → 拷贝默认模板 → 替换占位符 → 输出
    │
    └── 老项目 ──▶ 收集材料 → 推断模式 → 质问（6 个左右）→ 生成 → 输出
```

## 五、执行步骤

### Step 1：项目类型识别
读取 `01-detect-project.md`，判定新/老。

**判定逻辑**：
- 已有 README.md、Confluence、架构文档 → 老项目
- 项目运行 ≥ 3 个月、有产品文档 → 老项目
- 全否 → 新项目

### Step 2A：新项目路径（5 分钟）
1. 问 1 个问题（产品线代号 + 业务描述）
2. 读取 6 个默认模板
3. 替换占位符
4. 写入 `.qoder/context/`

### Step 2B：老项目路径（30–60 分钟）
1. 读取 `02-gather-materials.md`，收集 5 类材料
2. 读取 `03-infer-patterns.md`，找出模糊/缺失点
3. 读取 `04-ask-questions.md`，按顺序质问
4. 读取 `05-generate-context.md`，生成 6 个 context
5. 生成 `GENERATION-REPORT.md`

## 六、关键设计：质问机制（与 rules-generator 类似）

### 6.1 6 个常见质问

| # | 主题 | 关注 |
|---|------|------|
| 1 | 产品线划分 | LMS/OC/CMD 等 |
| 2 | 项目目标 | 一句话定位 + 业务指标 |
| 3 | 核心模块清单 | 模块名 + 职责 |
| 4 | 技术架构 | 单体/微服务/混合 |
| 5 | 业务术语 | 缩写 + 全称 |
| 6 | 项目成熟度 | 起步/成长/稳定/维护 |

**每个问题都有 A/B/C/D 选项 + 推荐项**。

### 6.2 质问原则
- 每个模糊点必问
- 必须有推荐项
- 选择题不开放题
- 给上下文证据

## 七、与 rules-generator 的关系

**两者独立，按使用顺序串联**：

```
rules-generator        → 先跑（生成 .qoder/rules/）
   ↓
context-generator      → 后跑（生成 .qoder/context/）
   ↓
yto-generator-spec     → 按需跑（在 yto-dev-analysis-pipeline 阶段 2 调用）
   ↓
class-builder          → 写代码时跑

> 已归档：`spec-generator`（2026-06-28），详见 `.qoder/skills/_archived/ARCHIVED.md`
```

**为什么不合并成一个 skill**：
- 关注点不同（代码约束 vs 业务背景）
- 输入不同（代码 vs 文档）
- 时机不同（项目级 vs 需求级）

## 八、与其他 skill 的关系

```
context-generator      ← 你在这里
    │
    ├──→ 生成产物 ──→ yto-generator-spec（生成时引用 context）
    │              → yto-generator-plan（生成时引用 context）
    │              → test-case-generator（生成时引用 context）
    │
    └──→ 推荐下一步 ──→ rules-generator（如果还没跑）
                      → yto-dev-analysis-pipeline（下一步开发需求）
```

## 九、使用示例

### 示例 1：新项目
```
你：帮我用 context-generator 给新项目配 context
AI：（执行 01-detect-project.md → 判定新项目）
    （问 1 个问题：项目定位 + 产品线）
    （生成 6 个 context 文件）
```

### 示例 2：老项目
```
你：用 context-generator 补全这个项目的 context
AI：（执行 01 → 判定老项目）
    （执行 02 → 收集 README/架构文档/产品文档）
    （执行 03 → 发现 6 个模糊点）
    （执行 04 → 依次质问 6 个问题）
    （执行 05 → 生成 6 个 context + 报告）
```

## 十、注意事项

1. **不读代码**——context 关注"业务"，不关注"实现"
2. **收集材料要充分**——README、架构图、产品文档、PPT 都要看
3. **质问前要列证据**——不能基于猜测质问
4. **业务术语必须澄清**——不澄清会污染后续所有生成
5. **保留 GENERATION-REPORT.md**——未来重构依据
