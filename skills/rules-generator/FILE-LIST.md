# rules-generator · 完整文件清单

> 这是 `rules-generator` skill 的所有文件清单，你可以对照检查是否有遗漏。

## 目录结构

```
.qoder/skills/rules-generator/
├── SKILL.md                          # 主入口
├── README.md                         # 使用说明
│
├── 01-detect-project.md              # 步骤 1：项目类型识别
├── 02-scan-codebase.md               # 步骤 2：代码扫描
├── 03-infer-patterns.md              # 步骤 3：模式推断
├── 04-ask-questions.md               # 步骤 4：质问用户
├── 05-generate-rules.md              # 步骤 5：生成 rule
│
├── questions/                        # 9 个标准质问
│   ├── 01-controller-valid.md
│   ├── 02-controller-swagger.md
│   ├── 03-controller-path.md
│   ├── 04-controller-response.md
│   ├── 05-service-transaction.md
│   ├── 06-exception-handling.md
│   ├── 07-log-format.md
│   ├── 08-package-naming.md
│   └── 09-module-naming.md
│
├── templates/                        # 4 个 rule 文件模板
│   ├── always-on.md.tpl
│   ├── java-class.md.tpl
│   ├── spec-reference.md.tpl
│   └── pom-xml.md.tpl
│
└── examples/                         # 输出示例
    ├── new-project-output.md
    └── old-project-output.md
```

## 文件统计

| 类别 | 数量 | 大约字符数 |
|------|------|----------|
| 主入口 + README | 2 | 600 行 |
| 步骤文件 | 5 | 1500 行 |
| 质问文件 | 9 | 800 行 |
| 模板文件 | 4 | 500 行 |
| 示例文件 | 2 | 200 行 |
| **合计** | **22** | **~3600 行** |

## 关键文件作用

| 文件 | 谁会读 | 何时读 |
|------|-------|--------|
| `SKILL.md` | AI | 接到"用 rules-generator"指令时 |
| `README.md` | 人 | 第一次使用前 |
| `01-detect-project.md` | AI | 第一步判定项目类型 |
| `02-scan-codebase.md` | AI | 老项目时执行扫描 |
| `03-infer-patterns.md` | AI | 扫描后推断不一致 |
| `04-ask-questions.md` | AI | 开始质问用户 |
| `05-generate-rules.md` | AI | 质问后生成文件 |
| `questions/*` | AI | 具体质问内容 |
| `templates/*` | AI | 生成 rule 时读取 |
| `examples/*` | 人 | 看输出长什么样 |

## 快速验证清单

部署到新项目后，验证以下 6 件事：

```
□ 1. 主入口 SKILL.md 存在且 frontmatter 正确
□ 2. 5 个步骤文件齐全
□ 3. 9 个质问文件齐全
□ 4. 4 个模板文件齐全
□ 5. 2 个示例文件存在
□ 6. 在新项目跑一次能正常输出 4 个 rule
```

## 下一步建议

部署完 rules-generator 后，可以做：

1. **在 1 个真实老项目试点**——验证效果
2. **收集问题反馈**——更新质问库和模板
3. **接入 CI**——把 rule 接入 PR 检查
4. **写配套 skill**——`rules-checker`（验证代码合规）、`code-fixer`（修复不合规）
5. **推广到其他项目**——把这套机制复制过去
