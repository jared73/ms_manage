# 05 · 生成 rule 文件

> 目标：根据用户选择和扫描报告，生成 `.qoder/rules/` 下的 4 个 rule 文件 + 1 份生成报告。
>
> **本步骤是机械执行**：把 04 步的选择填入模板，不需要再做判断。

## 输入

- 步骤 04 的"用户选择清单"
- 步骤 02 的"扫描报告"
- 步骤 03 的"模式推断报告"
- 4 个 rule 文件模板（`templates/` 目录下）

## 输出

```
.qoder/rules/
├── always-on.md          # 项目硬约束
├── java-class.md         # Java 类通用规则
├── spec-reference.md     # 何时加载 .spec/
├── pom-xml.md            # POM 硬约束
└── GENERATION-REPORT.md  # 生成报告（仅老项目）
```

## 模板替换机制

每个模板里都有占位符 `{{XXX}}`，根据用户选择替换：

### 占位符清单

| 占位符 | 来源 | 示例值 |
|--------|------|--------|
| `{{COMPANY}}` | 用户输入 | yto56 |
| `{{PRODUCT_LINE}}` | 用户输入 | oc |
| `{{PACKAGE_STYLE}}` | 质问 1 | cn.com.{COMPANY}.{PRODUCT_LINE}.{module}.{layer} |
| `{{MODULE_STYLE}}` | 质问 2 | {productLine}-{function}-{business}-{appType} |
| `{{CONTROLLER_VALID}}` | 质问 3 | @Valid / @Validated / none |
| `{{CONTROLLER_SWAGGER}}` | 质问 4 | full / simple / none |
| `{{CONTROLLER_PATH}}` | 质问 5 | class-level / method-level / mixed |
| `{{SERVICE_TRANSACTION}}` | 质问 6 | class-level / method-level / selective |
| `{{EXCEPTION_HANDLING}}` | 质问 7 | global / per-controller / mixed |
| `{{LOG_FORMAT}}` | 质问 8 | json / text |

## 模板替换示例

### 模板（`templates/always-on.md.tpl`）

```markdown
---
name: always-on
description: 项目级硬约束（永不关闭）
---

# 项目硬约束

## 命名
- 包路径：`{{PACKAGE_STYLE}}`
- 模块命名：`{{MODULE_STYLE}}`

## Controller
- 必须 @RestController
- 参数校验：{{CONTROLLER_VALID}}
- Swagger 注解：{{CONTROLLER_SWAGGER}}
- 路径前缀：{{CONTROLLER_PATH}}

## Service
- Impl 必须 @Service
- 事务注解：{{SERVICE_TRANSACTION}}

## 异常处理
{{EXCEPTION_HANDLING}}

## 日志
- 格式：{{LOG_FORMAT}}
```

### 替换后（用户选了 A 的所有推荐项）

```markdown
---
name: always-on
description: 项目级硬约束（永不关闭）
---

# 项目硬约束

## 命名
- 包路径：`cn.com.yto56.oc.{module}.{layer}`
- 模块命名：`{productLine}-{function}-{business}-{appType}`

## Controller
- 必须 @RestController
- 参数校验：必须 @Valid
- Swagger 注解：必须 @Api + @ApiOperation
- 路径前缀：类级 @RequestMapping

## Service
- Impl 必须 @Service
- 事务注解：类级 @Transactional

## 异常处理
全局 @ControllerAdvice 统一处理

## 日志
- 格式：JSON
```

## 4 个 rule 文件的生成逻辑

### 文件 1：always-on.md

**模板来源**：`templates/always-on.md.tpl`

**替换占位符**：
- `{{COMPANY}}` → 用户输入
- `{{PRODUCT_LINE}}` → 用户输入
- `{{PACKAGE_STYLE}}` → 质问 1
- `{{MODULE_STYLE}}` → 质问 2
- `{{CONTROLLER_VALID}}` → 质问 3
- `{{CONTROLLER_SWAGGER}}` → 质问 4
- `{{CONTROLLER_PATH}}` → 质问 5
- `{{SERVICE_TRANSACTION}}` → 质问 6
- `{{EXCEPTION_HANDLING}}` → 质问 7
- `{{LOG_FORMAT}}` → 质问 8

### 文件 2：java-class.md

**模板来源**：`templates/java-class.md.tpl`

**几乎不需要替换**，只改公司名：
- `{{COMPANY}}` → 用户输入

**内容**：Java 类通用结构、注释、import、禁止项。

### 文件 3：spec-reference.md

**模板来源**：`templates/spec-reference.md.tpl`

**替换占位符**：
- `{{PRODUCT_LINE}}` → 用户输入

**内容**：何时加载哪份 .spec/ 文档。

### 文件 4：pom-xml.md

**模板来源**：`templates/pom-xml.md.tpl`

**几乎不需要替换**。

**内容**：Maven 依赖管理、模块结构、禁止项。

### 文件 5：GENERATION-REPORT.md（仅老项目）

**内容模板**：

```markdown
---
name: generation-report
description: rules-generator 生成报告
generated_at: {时间}
project: {项目名}
generator: rules-generator v1.0
---

# rules-generator 生成报告

> 本报告由 rules-generator 自动生成，记录扫描 + 推断 + 质问 + 生成的完整过程。

## 1. 项目类型

{新项目 / 老项目}

## 2. 扫描统计

### 2.1 基础数据
- Java 类总数：{N}
- 模块数：{M}
- 配置文件数：{K}
- 技术栈依赖：{L} 个

### 2.2 类后缀分布

（前 10 个）

| 后缀 | 数量 |
|------|------|
| Controller | {n} |
| Service | {n} |
| Impl | {n} |
| ... | ... |

### 2.3 技术栈

| 技术 | 版本 |
|------|------|
| Spring Boot | {ver} |
| Java | {ver} |
| ... | ... |

## 3. 模式推断

### 3.1 完全一致项（直接采用）
- ✅ {项 1}
- ✅ {项 2}
- ...

### 3.2 不一致项（已质询）
- ⚠️ {项 1}（不一致度 {N}%）
- ⚠️ {项 2}（不一致度 {N}%）
- ...

## 4. 用户选择

| # | 主题 | 选择 | 备注 |
|---|------|------|------|
| 1 | 包名风格 | {A/B/C/D} | {理由} |
| 2 | 模块命名 | {A/B/C/D} | {理由} |
| 3 | Controller @Valid | {A/B/C/D} | {理由} |
| ... | ... | ... | ... |

## 5. 生成的 rule

- `.qoder/rules/always-on.md`
- `.qoder/rules/java-class.md`
- `.qoder/rules/spec-reference.md`
- `.qoder/rules/pom-xml.md`

## 6. 已知差距

> 基于扫描结果，以下代码与新 rule 不一致，需要后续改造：

| 类别 | 不一致数 | 占比 | 建议 |
|------|---------|------|------|
| Controller 缺 @Valid | 5 | 22% | 跑 code-fixer |
| Impl 缺 @Transactional | 10 | 22% | 跑 code-fixer |
| ... | ... | ... | ... |

## 7. 下一步建议

1. **跑 rules-checker** — 验证当前代码合规度
2. **跑 code-fixer** — 批量修复不一致代码
3. **跑 spec-generator** — 生成 .spec/（已归档 2026-06-28，详见 `.qoder/skills/_archived/ARCHIVED.md`）
4. **提交 rule 到 Git** — 团队共享

## 8. 反馈

如果生成的 rule 有问题，请：
1. 直接编辑 `.qoder/rules/*.md`
2. 在本报告末尾加备注
3. 反馈给 skill 维护者
```

## 生成的执行命令

```bash
# 1. 确保 .qoder/rules/ 目录存在
mkdir -p .qoder/rules

# 2. 写入 4 个 rule 文件
# （AI 内部用文件写入工具）

# 3. 写入 GENERATION-REPORT.md（老项目）

# 4. 提示用户检查
ls -la .qoder/rules/
```

## 替换占位符的工具函数（伪代码）

```python
def generate_rule(template_path, choices, output_path):
    with open(template_path) as f:
        template = f.read()

    for key, value in choices.items():
        placeholder = "{{" + key + "}}"
        template = template.replace(placeholder, str(value))

    with open(output_path, 'w') as f:
        f.write(template)

# 调用
generate_rule(
    "templates/always-on.md.tpl",
    {
        "COMPANY": "yto56",
        "PRODUCT_LINE": "oc",
        "PACKAGE_STYLE": "cn.com.yto56.oc.{module}.{layer}",
        "MODULE_STYLE": "{productLine}-{function}-{business}-{appType}",
        "CONTROLLER_VALID": "@Valid",
        "CONTROLLER_SWAGGER": "@Api + @ApiOperation",
        "CONTROLLER_PATH": "类级 @RequestMapping",
        "SERVICE_TRANSACTION": "类级 @Transactional",
        "EXCEPTION_HANDLING": "全局 @ControllerAdvice",
        "LOG_FORMAT": "JSON"
    },
    ".qoder/rules/always-on.md"
)
```

## 生成后自检

AI 必须自检生成的 rule 文件：

```markdown
## 生成自检

- [ ] 4 个 rule 文件都存在
- [ ] 占位符都被替换（无残留 `{{XXX}}`）
- [ ] 用户选择都正确反映
- [ ] 包名/类名/模块名等关键约束清晰
- [ ] 至少有 1 个反例（"禁止 XX"）
- [ ] 至少有 1 个正例（"必须 XX"）

自检通过 ✅
```

## 步骤 05 输出

```markdown
## 步骤 05 完成

### 生成的文件

```
.qoder/rules/
├── always-on.md          (3.2 KB)
├── java-class.md         (2.8 KB)
├── spec-reference.md     (1.5 KB)
├── pom-xml.md            (1.8 KB)
└── GENERATION-REPORT.md  (5.4 KB)  ← 仅老项目
```

### 摘要

- **包名风格**：cn.com.yto56.oc.{module}.{layer}
- **模块命名**：{productLine}-{function}-{business}-{appType}
- **Controller 校验**：@Valid
- **Controller 文档**：@Api + @ApiOperation
- **Controller 路径**：类级 @RequestMapping
- **Service 事务**：类级 @Transactional
- **异常处理**：全局 @ControllerAdvice
- **日志格式**：JSON

### 下一步建议

1. **review 生成的 4 个 rule** — 确认 AI 推断合理
2. **跑 rules-checker** — 验证代码合规度
3. **跑 code-fixer** — 修复不一致
4. **提交到 Git** — 团队共享

---

✅ rules-generator 全部完成！
```

## 错误处理

### 错误 1：模板文件不存在
```
⚠️ 模板 templates/always-on.md.tpl 不存在
使用内置默认模板生成...
```

### 错误 2：占位符未替换
```
⚠️ 检测到残留占位符：{{UNKNOWN}}
请检查用户选择是否完整
```

### 错误 3：写入失败
```
❌ 写入 .qoder/rules/always-on.md 失败
权限不足 / 目录不存在
```

## 备注

- **本步骤是机械执行**，不再做判断
- **占位符替换必须完整**，不能留任何 `{{XXX}}`
- **GENERATION-REPORT.md 是重要产物**，未来重构的依据
- **生成后必须自检**
