# 04 · 质问用户

> 目标：按推断清单，依次向用户提出选择题，收集项目标准。
>
> **这是整个 skill 的核心价值**——发现不一致，质问你选哪个作为标准。

## 质问原则

### 原则 1：每个不一致点必问
不能跳过。哪怕只有 2 个例外也要问（也许那 2 个才是对的）。

### 原则 2：必须有推荐项
降低决策成本。**80% 的情况用户无脑选 A 即可**。

### 原则 3：选择题不要开放题
4 个选项封顶，**必须有"其他"**（让用户自填）。

### 原则 4：给统计数据
让用户基于事实决策，不是基于 AI 的猜测。

### 原则 5：每个问题独立
前一个选择不影响后一个。即使选了矛盾项，下一个仍正常问。

## 质问格式（标准模板）

```markdown
## 质问 N/M：[主题]

**扫描发现**：
- A. [方式 A 描述]   — {数量} 个 ({占比}%)  ← 推荐
- B. [方式 B 描述]   — {数量} 个 ({占比}%)
- C. [方式 C 描述]   — {数量} 个 ({占比}%)
- D. 其他（请说明）

**推荐理由**：{为什么 A 是最优}

**示例**：
```java
// 方式 A
{A 的代码示例}

// 方式 B
{B 的代码示例}
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 8 个标准质问（按顺序）

### 质问 1/8：包名风格

> 详见 `questions/08-package-naming.md`

### 质问 2/8：模块命名

> 详见 `questions/09-module-naming.md`

### 质问 3/8：Controller 参数校验

> 详见 `questions/01-controller-valid.md`

### 质问 4/8：Controller Swagger

> 详见 `questions/02-controller-swagger.md`

### 质问 5/8：Controller 路径前缀

> 详见 `questions/03-controller-path.md`

### 质问 6/8：Service 事务注解

> 详见 `questions/05-service-transaction.md`

### 质问 7/8：异常处理

> 详见 `questions/06-exception-handling.md`

### 质问 8/8：日志格式

> 详见 `questions/07-log-format.md`

**注意**：实际执行时，AI 会根据推断结果调整顺序（优先问不一致度高的）。

## 质问执行流程

### 步骤 4.1：准备工作

```markdown
# 准备开始质问

项目共发现 {N} 个不一致点，将依次询问 {M} 个问题。

预计耗时：{M} × 30 秒 ≈ {X} 分钟

每个问题都有推荐项，**大部分情况选 A 即可**。

如果某个问题选 D（其他），请简单说明理由。

准备开始：
1. Controller 参数校验
2. Controller Swagger
3. Controller 路径前缀
4. Service 事务注解
5. 异常处理
6. 日志格式
7. 依赖注入
8. ...

按 Enter 开始第一个问题。
```

### 步骤 4.2：依次质问

每问完一个问题，AI 要：
1. **确认用户选择**
2. **记录到临时清单**（步骤 05 用）
3. **简短总结**（"好，我们用 A"）
4. **进入下一个问题**

```markdown
✅ 收到，选择 A（标准 @Valid）

进入质问 4/8：Controller Swagger 注解
```

### 步骤 4.3：收尾

```markdown
# 质问完成

## 用户选择汇总

| # | 主题 | 选择 | 备注 |
|---|------|------|------|
| 1 | 包名风格 | A | cn.com.{公司}.{业务线}.{模块}.{层} |
| 2 | 模块命名 | A | {业务线}-{功能}-{业务}-{appType} |
| 3 | Controller @Valid | A | 标准 @Valid |
| 4 | Controller Swagger | A | @Api + @ApiOperation |
| 5 | Controller 路径前缀 | A | 类级 @RequestMapping |
| 6 | Service 事务注解 | A | 类级 @Transactional |
| 7 | 异常处理 | A | 全局 @ControllerAdvice |
| 8 | 日志格式 | A | JSON 格式 |

---

下一步：进入步骤 05-generate-rules.md，生成 4 个 rule 文件。
```

## 特殊场景处理

### 场景 1：用户回答"全部选 A"

```markdown
你：全部选 A
AI：好的，已记录所有选择为推荐项（A）。进入步骤 05。
```

**仍然合法**，但 AI 要提醒：

```
⚠️ 注意：默认全选 A 会基于行业最佳实践。
如果团队有特殊约定，生成的 rule 可能需要微调。
```

### 场景 2：用户回答"先不生成，给我看看扫描报告"

```markdown
你：先不生成，给我看看扫描报告
AI：（输出完整扫描报告 + 模式推断报告，暂停流程）
```

### 场景 3：用户在某个问题犹豫

```markdown
你：这个我看不出来，能给更多上下文吗？
AI：（提供该问题相关的代码样本、影响范围、迁移成本分析）
```

### 场景 4：用户选 D（其他）

```markdown
你：D，因为我们公司有特殊要求
AI：好的，请说明具体内容（例如"必须用 @Validated 因为有全局拦截器"）
你：必须用 @Validated
AI：好的，已记录：D = @Validated（团队硬性要求）。进入下一题。
```

### 场景 5：项目太小，扫描不到数据

```markdown
AI：项目只有 5 个 Controller，无法统计占比。
   将直接按行业最佳实践推荐，您可以选择确认或修改。

   1. Controller @Valid：默认 A（@Valid）
   2. Controller Swagger：默认 A（@Api + @ApiOperation）
   ...

请确认或修改（直接回车 = 全部默认）。
```

## 用户体验优化

### 优化 1：批量质问

**高级用户**可以一次性回答：
```
你：1A 2A 3A 4A 5A 6A 7A 8A
AI：（批量记录）
```

### 优化 2：跳过已知项

```
你：1 我看下问题描述。2 跳过我选了 A。3 ...
```

### 优化 3：导出为配置文件

```
你：能不能把这些问题导出成 yaml，我填完再导回来？
AI：（导出 questions.yaml 模板）
```

## 质问记录格式

每问完一个问题，AI 内部维护：

```yaml
# 内部记录（步骤 05 会用）
user_choices:
  package_naming: A
  module_naming: A
  controller_valid: A
  controller_swagger: A
  controller_path: A
  service_transaction: A
  exception_handling: A
  log_format: A
  dependency_injection: A

notes:
  controller_valid: ""  # 用户选 D 时记录理由
  ...
```

**步骤 05-generate-rules.md 会读这份记录**。

## 反模式（要避免）

### ❌ 反模式 1：跳过质问直接推荐
```
AI：我看了你的项目，建议用 @Valid。  ← 错误，跳过质问
```

### ❌ 反模式 2：质问后不记录
```
AI：好的，下一题。
    （但忘了把上题答案记下来）  ← 错误
```

### ❌ 反模式 3：质问超过 10 个
```
AI：还有 12 个问题...  ← 错误，用户会崩溃
```

### ❌ 反模式 4：推荐项没有理由
```
A. @Valid
B. @Validated
C. 无校验

推荐 A。  ← 错误，没有理由
```

### ❌ 反模式 5：用开放题
```
你希望参数校验怎么做？  ← 错误，应该是选择题
```

## 步骤 04 完成后输出

```markdown
## 步骤 04 完成

**用户选择**：8/8 全部回答
**待生成文件**：4 个 rule
**生成报告**：1 份

进入步骤 05-generate-rules.md。
```

## 备注

- **每个具体质问的完整内容**见 `questions/` 目录下对应文件
- **本步骤只控制流程**，不展开具体质问内容
- **本步骤产出**是一份"用户选择清单"，作为步骤 05 的输入
