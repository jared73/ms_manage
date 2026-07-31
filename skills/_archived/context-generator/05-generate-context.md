# 05 · 生成 context 文件

> 目标：根据用户回答和材料收集报告，生成 `.qoder/context/` 下的 6 个 context 文件 + 1 份生成报告。
>
> **本步骤是机械执行**：把 04 步的答案填入模板，不需要再做判断。

## 输入

- 步骤 04 的"用户回答清单"
- 步骤 02 的"材料收集报告"
- 步骤 03 的"模式推断报告"
- 6 个 context 文件模板（`templates/` 目录下）

## 输出

```
.qoder/context/
├── project-overview.md         # 项目是什么
├── architecture.md              # 技术架构
├── module-responsibility.md     # 模块职责
├── business-map-{X}.md         # 业务地图（每个产品线一份）
├── tech-stack.md                # 技术栈
├── glossary.md                  # 术语表
└── GENERATION-REPORT.md         # 生成报告（仅老项目）
```

## 模板替换机制

每个模板里都有占位符 `{{XXX}}`，根据用户回答替换：

### 占位符清单

| 占位符 | 来源 | 示例值 |
|--------|------|--------|
| `{{PROJECT_NAME}}` | 用户输入 | 圆通订单中心 |
| `{{PROJECT_TAGLINE}}` | 质问 2 | 处理订单全生命周期 |
| `{{COMPANY}}` | 用户输入 | yto56 |
| `{{PRODUCT_LINES}}` | 质问 1 | oc, lms, cmd |
| `{{CURRENT_PRODUCT_LINE}}` | 用于 business-map | oc |
| `{{CORE_MODULES}}` | 质问 3 | order, payment, inventory |
| `{{ARCHITECTURE_TYPE}}` | 质问 4 | 微服务 |
| `{{GLOSSARY}}` | 质问 5 | LMS=物流, OC=订单中心, CMD=推送 |
| `{{MATURITY}}` | 质问 6 | 稳定期 |
| `{{TARGET_USERS}}` | 用户输入 | 内部 / 外部 |
| `{{TEAM_MEMBERS}}` | 推断 | TL 张三，后端 5 人 |
| `{{BUSINESS_METRICS}}` | 推断 | DAU 1 万 / 订单 100 万 / 天 |

## 模板替换示例

### 模板（`templates/project-overview.md.tpl`）

```markdown
---
name: project-overview
description: 项目总览
---

# {{PROJECT_NAME}}

> {{PROJECT_TAGLINE}}

## 一句话定位
{{PROJECT_TAGLINE}}

## 项目目标
- 业务目标 1：[待补充]
- 业务目标 2：[待补充]

## 关键时间节点
- 立项：[推断]
- 上线：[推断]
- 当前阶段：{{MATURITY}}

## 核心指标
{{BUSINESS_METRICS}}

## 相关人/团队
{{TEAM_MEMBERS}}

## 产品线
{{PRODUCT_LINES}}
```

### 替换后（用户回答了所有问题）

```markdown
---
name: project-overview
description: 项目总览
---

# 圆通订单中心

> 处理订单全生命周期

## 一句话定位
处理订单全生命周期

## 项目目标
- 业务目标 1：[待补充]
- 业务目标 2：[待补充]

## 关键时间节点
- 立项：[推断]
- 上线：[推断]
- 当前阶段：稳定期

## 核心指标
DAU 1 万 / 订单 100 万 / 天

## 相关人/团队
TL 张三，后端 5 人

## 产品线
oc, lms, cmd
```

## 6 个 context 文件的生成逻辑

### 文件 1：project-overview.md

**模板来源**：`templates/project-overview.md.tpl`

**替换占位符**：
- `{{PROJECT_NAME}}` → 质问 1 或用户输入
- `{{PROJECT_TAGLINE}}` → 质问 2
- `{{MATURITY}}` → 质问 6
- `{{BUSINESS_METRICS}}` → 推断或用户输入
- `{{TEAM_MEMBERS}}` → 推断或用户输入
- `{{PRODUCT_LINES}}` → 质问 1

### 文件 2：architecture.md

**模板来源**：`templates/architecture.md.tpl`

**替换占位符**：
- `{{ARCHITECTURE_TYPE}}` → 质问 4
- `{{TECH_STACK}}` → 从材料收集报告
- `{{MODULE_DEPENDENCIES}}` → 从材料推断

**内容**：整体架构、技术选型、模块关系、关键决策。

### 文件 3：module-responsibility.md

**模板来源**：`templates/module-responsibility.md.tpl`

**替换占位符**：
- `{{CORE_MODULES}}` → 质问 3
- `{{MODULE_RESPONSIBILITY_MAP}}` → 用户回答细化

**内容**：模块清单、模块职责、依赖关系、新增模块判断。

### 文件 4：business-map-{X}.md（每个产品线一份）

**模板来源**：`templates/business-map.md.tpl`

**生成数量**：质问 1 回答的产品线数量
- 1 条产品线 → 1 份 business-map-oc.md
- 3 条产品线 → 3 份 business-map-oc.md / -lms.md / -cmd.md

**替换占位符**：
- `{{CURRENT_PRODUCT_LINE}}` → 当前产品线代号
- `{{PRODUCT_LINE_FULL_NAME}}` → 从术语表
- `{{BUSINESS_SCOPE}}` → 用户回答或推断
- `{{KEY_CAPABILITIES}}` → 从材料推断

**内容**：该产品线下的所有 ms 模块、业务能力、关键场景。

### 文件 5：tech-stack.md

**模板来源**：`templates/tech-stack.md.tpl`

**替换占位符**：
- `{{LANGUAGE_VERSION}}` → 从 pom.xml
- `{{FRAMEWORK_VERSION}}` → 从 pom.xml
- `{{MIDDLEWARE}}` → 从 application.yml
- `{{TOOLS}}` → 从 pom.xml

**内容**：技术栈清单、版本锁定、用途说明。

### 文件 6：glossary.md

**模板来源**：`templates/glossary.md.tpl`

**替换占位符**：
- `{{GLOSSARY}}` → 质问 5

**内容**：业务术语、产品线代号、模块名解释。

### 文件 7：GENERATION-REPORT.md（仅老项目）

**内容模板**：

```markdown
---
name: generation-report
description: context-generator 生成报告
generated_at: {时间}
project: {项目名}
generator: context-generator v1.0
---

# context-generator 生成报告

> 本报告由 context-generator 自动生成，记录材料 + 推断 + 质问 + 生成的完整过程。

## 1. 项目类型

{新项目 / 老项目}

## 2. 材料收集

### 2.1 已收集材料
- {材料 1}
- {材料 2}
- ...

### 2.2 缺失材料
- {缺失 1}
- {缺失 2}
- ...

## 3. 模式推断

### 3.1 充分项（直接采用）
- ✅ {项 1}
- ✅ {项 2}
- ...

### 3.2 模糊项（已质询）
- ⚠️ {项 1}（模糊度 {N}%）
- ⚠️ {项 2}（模糊度 {N}%）
- ...

## 4. 用户回答

| # | 主题 | 回答 | 备注 |
|---|------|------|------|
| 1 | 产品线 | {回答} | |
| 2 | 定位 | {回答} | |
| 3 | 模块 | {回答} | |
| 4 | 架构 | {回答} | |
| 5 | 术语 | {回答} | |
| 6 | 成熟度 | {回答} | |

## 5. 生成的 context

- .qoder/context/project-overview.md
- .qoder/context/architecture.md
- .qoder/context/module-responsibility.md
- .qoder/context/business-map-{X}.md × {N} 份
- .qoder/context/tech-stack.md
- .qoder/context/glossary.md

## 6. 已知不足

> 基于材料收集结果，以下信息可能不准确，需要补充：

| 类别 | 不足内容 | 建议 |
|------|---------|------|
| 业务指标 | 缺少 SLA/DAU 等数据 | 联系 PMO 补充 |
| 团队成员 | 缺少完整名单 | 联系 HR 补充 |
| 架构图 | 没有找到架构图 | 团队补充 |
| ... | ... | ... |

## 7. 下一步建议

1. **review 6 个 context 文件** — 确认 AI 推断合理
2. **补充缺失信息** — 根据"已知不足"补充
3. **跑 rules-generator** — 如果还没跑
4. **提交 context 到 Git** — 团队共享
5. **季度 review** — 持续维护

## 8. 反馈

如果生成的 context 有问题：
1. 直接编辑 .qoder/context/*.md
2. 在本报告末尾加备注
3. 反馈给 skill 维护者
```

## 生成的执行命令

```bash
# 1. 确保 .qoder/context/ 目录存在
mkdir -p .qoder/context

# 2. 写入 6 个 context 文件
# （AI 内部用文件写入工具）

# 3. 写入 GENERATION-REPORT.md（老项目）

# 4. 提示用户检查
ls -la .qoder/context/
```

## 替换占位符的工具函数（伪代码）

```python
def generate_context(template_path, answers, output_path):
    with open(template_path) as f:
        template = f.read()

    for key, value in answers.items():
        placeholder = "{{" + key + "}}"
        template = template.replace(placeholder, str(value))

    with open(output_path, 'w') as f:
        f.write(template)

# 调用（生成 6 个文件）
for tmpl, output in [
    ("templates/project-overview.md.tpl", ".qoder/context/project-overview.md"),
    ("templates/architecture.md.tpl", ".qoder/context/architecture.md"),
    ("templates/module-responsibility.md.tpl", ".qoder/context/module-responsibility.md"),
    ("templates/tech-stack.md.tpl", ".qoder/context/tech-stack.md"),
    ("templates/glossary.md.tpl", ".qoder/context/glossary.md"),
]:
    generate_context(tmpl, answers, output)

# 生成多份 business-map
for product_line in product_lines:
    generate_context(
        "templates/business-map.md.tpl",
        {**answers, "CURRENT_PRODUCT_LINE": product_line},
        f".qoder/context/business-map-{product_line}.md"
    )
```

## 生成后自检

AI 必须自检生成的 context 文件：

```markdown
## 生成自检

- [ ] 6 个 context 文件都存在
- [ ] business-map 文件数量 = 产品线数量
- [ ] 占位符都被替换（无残留 `{{XXX}}`）
- [ ] 用户回答都正确反映
- [ ] 项目名/产品线/术语等关键信息清晰
- [ ] 至少 1 个章节标注"待补充"
- [ ] GENERATION-REPORT.md 记录完整（老项目）

自检通过 ✅
```

## 步骤 05 输出

```markdown
## 步骤 05 完成

### 生成的文件

```
.qoder/context/
├── project-overview.md         (1.2 KB)
├── architecture.md              (1.5 KB)
├── module-responsibility.md     (1.0 KB)
├── business-map-oc.md           (1.8 KB)
├── business-map-lms.md          (1.5 KB)
├── business-map-cmd.md          (1.2 KB)
├── tech-stack.md                (0.8 KB)
├── glossary.md                  (0.5 KB)
└── GENERATION-REPORT.md         (3.5 KB)  ← 仅老项目
```

### 摘要

- **产品线**：oc, lms, cmd
- **项目定位**：处理订单全生命周期
- **核心模块**：order, payment, inventory
- **技术架构**：微服务
- **项目阶段**：稳定期

### 下一步建议

1. **review 生成的 context** — 确认 AI 推断合理
2. **补充缺失信息** — 根据"已知不足"补充
3. **跑 rules-generator** — 如果还没跑
4. **提交到 Git** — 团队共享

---

✅ context-generator 全部完成！
```

## 错误处理

### 错误 1：模板文件不存在
```
⚠️ 模板 templates/project-overview.md.tpl 不存在
使用内置默认模板生成...
```

### 错误 2：占位符未替换
```
⚠️ 检测到残留占位符：{{UNKNOWN}}
请检查用户回答是否完整
```

### 错误 3：业务地图数量不对
```
⚠️ business-map 文件数量（{A}）与产品线数量（{B}）不一致
请检查
```

## 备注

- **本步骤是机械执行**，不再做判断
- **占位符替换必须完整**，不能留任何 `{{XXX}}`
- **business-map 按产品线生成多份**
- **GENERATION-REPORT.md 是重要产物**
- **生成后必须自检**
