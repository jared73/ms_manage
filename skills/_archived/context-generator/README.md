# context-generator 使用说明

> 30 秒看懂这个 skill 怎么用。

## 一句话说明

**输入项目材料，输出 6 份 context 文件。**

## 什么时候用？

- ✅ 新项目要配 SDD
- ✅ 老项目缺 context，要补全
- ✅ context 文件过期，要重新生成
- ✅ 推广 SDD 到兄弟项目（和 rules-generator 配对）

## 生成什么东西？

```
.qoder/context/
├── project-overview.md         # 项目是什么
├── architecture.md              # 技术架构
├── module-responsibility.md     # 模块职责
├── business-map-{X}.md         # 业务地图（按产品线，每条线一份）
├── tech-stack.md                # 技术栈
└── glossary.md                  # 术语表
```

**老项目还会额外生成**：

```
.qoder/context/
└── GENERATION-REPORT.md        # 生成报告
```

## 怎么用？

### 方式 1：直接告诉 AI
```
你：帮我用 context-generator 给这个项目配 context
```

### 方式 2：和 rules-generator 一起跑
```
你：先用 rules-generator，再用 context-generator
```

## 完整流程图

```
        ┌──────────────────┐
        │ 项目 + 项目材料   │
        └────────┬─────────┘
                 │
                 ▼
        ┌──────────────────┐
        │ 01 识别项目类型   │
        └────────┬─────────┘
                 │
         ┌───────┴───────┐
         │               │
     新项目           老项目
         │               │
         │               ▼
         │        ┌──────────────┐
         │        │ 02 收集材料   │
         │        └──────┬───────┘
         │               │
         │               ▼
         │        ┌──────────────┐
         │        │ 03 推断模式   │
         │        └──────┬───────┘
         │               │
         │               ▼
         │        ┌──────────────┐
         │        │ 04 质问用户   │  ⭐ 关键
         │        └──────┬───────┘
         │               │
         └───────┬───────┘
                 │
                 ▼
        ┌──────────────────┐
        │ .qoder/context/  │
        └──────────────────┘
```

## 与 rules-generator 的区别

| 维度 | rules-generator | context-generator |
|------|----------------|-------------------|
| 输入 | 项目代码 | 项目材料（文档） |
| 输出 | 4 个 rule | 6 个 context |
| 关注 | 代码约束 | 业务背景 |
| 质问 | 代码风格不一致 | 业务边界模糊 |
| 跑顺序 | 先 | 后 |

**推荐组合**：
```bash
# 1. 先跑 rules-generator（生成代码规则）
sdd gen-rules

# 2. 再跑 context-generator（生成业务背景）
sdd gen-context

# 3. 跑 spec-generator（按需）
sdd gen-spec
```

## 我需要准备什么材料？

### 老项目（理想情况）
- ✅ README.md
- ✅ 架构设计文档
- ✅ 产品需求文档
- ✅ 团队成员清单
- ✅ 业务术语表

### 老项目（最低要求）
- 至少要有 README.md
- 其他可以从代码推断（pom.xml 看技术栈、模块结构看产品线）

### 新项目
- 不用准备任何材料
- 回答 1 个问题即可

## 常见问题

### Q1：必须先有 rules-generator 吗？
**A**：不必须，但推荐。两者独立，但按顺序跑最合理。

### Q2：context 文件多久更新一次？
**A**：
- project-overview、architecture：项目变化时更新
- business-map：业务变化时更新
- tech-stack：技术栈变化时更新
- glossary：积累新术语时更新
- 建议每季度 review 一次

### Q3：和老项目的现有 context 冲突怎么办？
**A**：备份后覆盖。GENERATION-REPORT.md 会记录所有变更。

### Q4：项目跨多个产品线怎么办？
**A**：每个产品线生成一份 business-map-{X}.md。

### Q5：context 写错了怎么办？
**A**：直接编辑对应文件，下次跑 context-generator 时可以选择"只生成缺失的"。

## 反馈

用的时候发现问题，反馈给 skill 维护者，附上：
- 项目类型（新/老）
- 跑哪个步骤出的问题
- 具体问题描述
