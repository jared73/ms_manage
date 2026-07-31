# context-generator · 完整文件清单

> 这是 `context-generator` skill 的所有文件清单。

## 目录结构

```
.qoder/skills/context-generator/
├── SKILL.md                          # 主入口
├── README.md                         # 使用说明
├── FILE-LIST.md                      # 本文件
│
├── 01-detect-project.md              # 步骤 1：项目类型识别
├── 02-gather-materials.md            # 步骤 2：收集材料
├── 03-infer-patterns.md              # 步骤 3：模式推断
├── 04-ask-questions.md               # 步骤 4：质问用户
├── 05-generate-context.md            # 步骤 5：生成 context
│
├── questions/                        # 6 个标准质问
│   ├── 01-product-line.md
│   ├── 02-project-tagline.md
│   ├── 03-core-modules.md
│   ├── 04-architecture.md
│   ├── 05-glossary.md
│   └── 06-maturity.md
│
├── templates/                        # 6 个 context 模板
│   ├── project-overview.md.tpl
│   ├── architecture.md.tpl
│   ├── module-responsibility.md.tpl
│   ├── business-map.md.tpl
│   ├── tech-stack.md.tpl
│   └── glossary.md.tpl
│
└── examples/                         # 输出示例
    ├── new-project-output.md
    └── old-project-output.md
```

## 文件统计

| 类别 | 数量 | 大约字符数 |
|------|------|----------|
| 主入口 + README + FILE-LIST | 3 | 800 行 |
| 步骤文件 | 5 | 1500 行 |
| 质问文件 | 6 | 600 行 |
| 模板文件 | 6 | 500 行 |
| 示例文件 | 2 | 200 行 |
| **合计** | **22** | **~3600 行** |

## 关键文件作用

| 文件 | 谁会读 | 何时读 |
|------|-------|--------|
| `SKILL.md` | AI | 接到"用 context-generator"指令时 |
| `README.md` | 人 | 第一次使用前 |
| `01-detect-project.md` | AI | 第一步判定项目类型 |
| `02-gather-materials.md` | AI | 老项目时收集材料 |
| `03-infer-patterns.md` | AI | 收集后推断模糊点 |
| `04-ask-questions.md` | AI | 开始质问用户 |
| `05-generate-context.md` | AI | 质问后生成文件 |
| `questions/*` | AI | 具体质问内容 |
| `templates/*` | AI | 生成 context 时读取 |
| `examples/*` | 人 | 看输出长什么样 |

## 与 rules-generator 的对应关系

| rules-generator | context-generator | 说明 |
|----------------|-------------------|------|
| SKILL.md | SKILL.md | 主入口 |
| 01-detect-project.md | 01-detect-project.md | 项目类型识别 |
| 02-scan-codebase.md | 02-gather-materials.md | 数据收集 |
| 03-infer-patterns.md | 03-infer-patterns.md | 模式推断 |
| 04-ask-questions.md | 04-ask-questions.md | 质问用户 |
| 05-generate-rules.md | 05-generate-context.md | 生成文件 |
| questions/ (9 个) | questions/ (6 个) | 质问库 |
| templates/ (4 个) | templates/ (6 个) | 文件模板 |
| examples/ (2 个) | examples/ (2 个) | 输出示例 |

## 关键差异

| 维度 | rules-generator | context-generator |
|------|----------------|-------------------|
| 输入 | 项目代码 | 项目材料（文档） |
| 输出 | 4 个 rule | 6+ 个 context（含多份 business-map） |
| 质问类型 | 选择题为主 | 填空题 + 选择题 |
| 质问数量 | 8 个左右 | 6 个左右 |
| 收集时间 | 慢（要扫代码） | 快（只看几个文档） |
| 适用时机 | 项目级 | 项目级（先于 rules） |

## 快速验证清单

部署到新项目后，验证以下 7 件事：

```
□ 1. 主入口 SKILL.md 存在且 frontmatter 正确
□ 2. 5 个步骤文件齐全
□ 3. 6 个质问文件齐全
□ 4. 6 个模板文件齐全
□ 5. 2 个示例文件存在
□ 6. 在新项目跑一次能正常输出 6 个 context
□ 7. 在老项目跑一次能正常输出 6+N 个 context（N=产品线数）+ 1 份报告
```

## 下一步建议

部署完 context-generator 后，可以做：

1. **在 1 个真实老项目试点**——验证效果
2. **收集问题反馈**——更新质问库和模板
3. **接入 spec-generator**——让 spec-generator 阶段 0 自动加载 context
4. **写配套 skill**——`context-updater`（context 变更跟踪）
5. **推广到其他项目**——把这套机制复制过去
