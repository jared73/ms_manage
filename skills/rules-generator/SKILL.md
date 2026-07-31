---
name: rules-generator
description: 扫描项目并生成 .qoder/rules/ 配置。新项目用默认模板，老项目基于实际代码扫描+质问+生成。
---

# rules-generator

> **这个 skill 解决一件事**：让任何项目（新或老）在 5–60 分钟内拥有一套专属的 rule。

## 一、核心定位

| 维度 | 内容 |
|------|------|
| **输入** | 一个 Java 后端项目（新项目 or 老项目） |
| **输出** | `.qoder/rules/` 下的 4 个 rule 文件 + 1 份生成报告 |
| **关键动作** | 扫描 → 推断 → **质问** → 生成 |
| **不依赖** | 不读 `.spec/`、不读 README（context 已归档，无需读取） |

**设计原则**：
- **直接看真相（代码），不看转述（context/文档）**——避免信息损失
- **质问胜于假设**——发现多种实现方式时主动问，不替用户做主
- **零决策起步**——推荐项让用户基本"无脑选 A"

## 二、何时使用

### 触发场景
- ✅ 给新项目配置 SDD
- ✅ 给老项目改造 SDD
- ✅ 推广 SDD 到兄弟项目
- ✅ 重新生成 rule（业务方向调整时）

### 不适用
- ❌ 已有 rule 的项目（用 `rules-checker` 检查）
- ❌ 只想改某条 rule（直接编辑 rule 文件）
- ❌ 非 Java 项目（当前只支持 Java 后端）

## 三、流程总览

```
[项目代码]
    │
    ▼
┌─────────────────┐
│ 01 项目类型识别   │  → 新项目 vs 老项目
└─────────────────┘
    │
    ├── 新项目 ──▶ 1 个轻量质问 → 拷贝默认模板 → 替换占位符 → 输出
    │
    └── 老项目 ──▶ 扫描 → 推断模式 → 质问（8 个左右）→ 生成 → 输出
```

**详细流程见后续步骤文件。**

## 四、输出清单

### 4 个 rule 文件（无论新老都生成）

| 文件 | 作用 | 何时加载 |
|------|------|---------|
| `always-on.md` | 项目级硬约束（命名、注解、包路径） | 永远生效 |
| `java-class.md` | Java 类通用规则（结构、注释、import） | 写任何 Java 类时 |
| `spec-reference.md` | 何时加载哪份 .spec/ | 写代码前必读 |
| `pom-xml.md` | POM 修改硬约束 | 修改 pom.xml 时 |

### 1 份生成报告（仅老项目）

`.qoder/rules/GENERATION-REPORT.md`，包含：
- 扫描统计
- 每个质问的选择 + 推荐理由
- 与项目现状的差距

## 五、执行步骤

### Step 1：项目类型识别
读取 `01-detect-project.md`，判定新/老项目。

### Step 2A：新项目路径
1. 问 1 个问题（包名风格）
2. 读取 `templates/` 下 4 个模板
3. 替换占位符
4. 写入 `.qoder/rules/`

### Step 2B：老项目路径
1. 读取 `02-scan-codebase.md`，执行 6 维度扫描
2. 读取 `03-infer-patterns.md`，找出不一致点
3. 读取 `04-ask-questions.md`，按顺序质问
4. 读取 `05-generate-rules.md`，生成 4 个 rule
5. 生成 `GENERATION-REPORT.md`

## 六、关键设计：质问机制

### 6.1 什么是"质问"
发现项目中存在 **N 种不同的实现方式** 时，主动列出所有选项，让用户选一个作为项目标准。

**示例**：

```markdown
## 质问 3/8：Controller @Valid 注解

扫描发现 3 种实现：
A. @RequestBody @Valid XxxReq req      (18 个, 78%)   ← 推荐
B. @RequestBody XxxReq req             (3 个, 13%)
C. @Validated + @RequestBody XxxReq    (2 个, 9%)

请选择哪种作为本项目标准？
- [ ] A. @Valid（推荐：主流做法）
- [ ] B. 不加注解
- [ ] C. @Validated（嵌套校验场景）
- [ ] D. 其他（请说明）
```

### 6.2 质问原则
- **每个不一致点必问**——不能跳过
- **必须有推荐项**——降低决策成本
- **选择题不要开放题**——4 个选项封顶
- **给统计数据**——让用户基于事实决策

### 6.3 质问清单（见 `questions/` 目录）
- 01-controller-valid（参数校验）
- 02-controller-swagger（接口文档）
- 03-controller-path（路径前缀）
- 04-controller-response（统一返回）
- 05-service-transaction（事务注解）
- 06-exception-handling（异常处理）
- 07-log-format（日志格式）
- 08-package-naming（包名风格）
- 09-module-naming（模块命名）

## 七、与其他 skill 的关系

`rules-generator` 自身独立，不依赖任何下游 skill。

**为什么不需要前置 context**：
- context 是叙述性的（给 AI 读），会失真
- rules 是约束性的（给 CI 跑），必须精确
- 直接看代码最可靠

**历史变更**：`context-generator` 已于 2026-06-28 归档（详见 `.qoder/skills/_archived/ARCHIVED.md`），本 skill 不再与之协同。

## 八、与其他 skill 的关系

```
rules-generator     ← 你在这里
    │
    ├──→ 生成产物 ──→ rules-checker（验证代码是否符合）
    │
    └──→ 推荐下一步 ──→ code-scanner（看哪些代码不合规）
                      → code-fixer（修复不合规代码）
                      → yto-dev-analysis-pipeline（需求进来时跑）

> 已归档：`spec-generator`（2026-06-28）、`context-generator`（2026-06-28），详见 `.qoder/skills/_archived/ARCHIVED.md`
```

## 九、使用示例

### 示例 1：新项目
```
你：帮我用 rules-generator 给这个新项目配 rule
AI：（执行 01-detect-project.md → 判定新项目）
    （问 1 个问题：包名风格？）
    （生成 4 个 rule 文件）
    （输出报告）
```

### 示例 2：老项目
```
你：用 rules-generator 扫描我的项目
AI：（执行 01-detect-project.md → 判定老项目）
    （执行 02-scan-codebase.md → 扫描 6 维度）
    （执行 03-infer-patterns.md → 发现 8 个不一致点）
    （执行 04-ask-questions.md → 依次质问 1/8, 2/8, ..., 8/8）
    （你回答每个问题）
    （执行 05-generate-rules.md → 生成 4 个 rule）
    （生成 GENERATION-REPORT.md）
```

## 十、注意事项

1. **质问前必须先扫描**——不能基于猜测质问
2. **每个问题独立选择**——前一个选择不影响后一个
3. **生成后必须 review**——AI 生成的 rule 可能有偏差
4. **建议先跑小项目**——验证 skill 效果再推大项目
5. **保留 GENERATION-REPORT.md**——未来重构的依据
