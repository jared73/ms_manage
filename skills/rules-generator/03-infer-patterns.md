# 03 · 模式推断

> 目标：从扫描报告中找出"多种实现方式"的不一致点，作为质问的依据。
>
> **本步骤不质问用户，只识别问题。**质问在步骤 04。

## 核心判断标准

对每个维度，统计每种"实现方式"的占比：

| 占比 | 判定 | 处理 |
|------|------|------|
| **100%** | 完全一致 | ✅ 写入 rule，无需质问 |
| **70%–99%** | 主流 + 少量例外 | ⚠️ 质问，推荐主流方案 |
| **40%–69%** | 多派系并存 | ⚠️ 质问，让用户裁决 |
| **< 40%** | 无主流 | ⚠️ 质问，必须选一个 |
| **= 0% 但有 1–2 个** | 偶发 | 📝 记录但不质问（极小众） |

## 推断流程

### 步骤 3.1：建立"待质问清单"

把扫描报告中所有"不一致项"汇总成一张表：

```markdown
## 待质问清单

| # | 维度 | 涉及范围 | 不一致程度 | 优先级 |
|---|------|---------|-----------|--------|
| 1 | Controller @Valid | 23 个 Controller | 22% 不一致 | 高 |
| 2 | Controller Swagger | 23 个 Controller | 48% 不一致 | 高 |
| 3 | Controller 路径前缀 | 23 个 Controller | 35% 不一致 | 中 |
| 4 | Service 事务注解 | 45 个 Impl | 56% 不一致 | 高 |
| 5 | 异常处理 | 23 个 Controller | 待补充 | 高 |
| 6 | 日志格式 | 配置文件 | 待补充 | 中 |
| 7 | 包名风格 | 所有类 | 100% 一致 | 低（直接采用） |
| 8 | 模块命名 | 所有模块 | 100% 一致 | 低（直接采用） |
| 9 | 依赖注入 | 所有类 | 混合 | 中 |
```

### 步骤 3.2：分析每个"不一致点"

对每个不一致点，必须输出**完整分析**（示例）：

```markdown
### 不一致点 1：Controller @Valid 用法

**扫描数据**：
- @RequestBody @Valid XxxReq req: 18 个 (78%)
- @RequestBody XxxReq req（无校验）: 3 个 (13%)
- @RequestBody @Validated XxxReq req: 2 个 (9%)

**模式归纳**：

#### 方式 A：标准 @Valid
```java
@PostMapping("/create")
public Result<CreateOrderResp> createOrder(@Valid @RequestBody CreateOrderReq req) {
    // 18 个类用此方式
}
```

#### 方式 B：无校验
```java
@PostMapping("/create")
public Result<CreateOrderResp> createOrder(@RequestBody CreateOrderReq req) {
    // 3 个类用此方式（可能漏写或有意为之）
}
```

#### 方式 C：@Validated（嵌套校验）
```java
@PostMapping("/create")
public Result<CreateOrderResp> createOrder(@Validated @RequestBody CreateOrderReq req) {
    // 2 个类用此方式（需要嵌套校验时）
}
```

**推荐**：A（标准 @Valid，主流做法）

**为什么 C 不推荐为主流**：
- @Validated 在嵌套校验场景才需要
- 普通校验用 @Valid 足够
- 99% 的团队用 @Valib 做参数校验

**该问题将作为质问 1/8 在步骤 04 提出**。
```

### 步骤 3.3：判断是否需要质问

每个不一致点都要回答 3 个问题：

```
Q1：是否有"明显主流"（≥70%）？
  - 是 → 推荐主流 + 简短理由
  - 否 → 必须让用户决策

Q2：推荐方案是否"团队可接受"？
  - 是 → 直接推荐
  - 否 → 列出所有方案让用户选

Q3：选错会不会"破坏现有代码"？
  - 不会 → 推荐项更激进（推最佳实践）
  - 会 → 推荐项更保守（保护现有代码）
```

### 步骤 3.4：生成"质问清单"

把"需要质问"的不一致点按顺序排好：

```markdown
## 质问清单（步骤 04 将依次执行）

| # | 主题 | 优先级 | 期望用户操作 |
|---|------|--------|------------|
| 1 | Controller @Valid | 高 | 选 A/B/C/D |
| 2 | Controller Swagger | 高 | 选 A/B/C |
| 3 | Controller 路径前缀 | 中 | 选 A/B/C |
| 4 | Service 事务注解 | 高 | 选 A/B/C |
| 5 | 异常处理 | 高 | 选 A/B/C |
| 6 | 日志格式 | 中 | 选 A/B |
| 7 | 依赖注入 | 中 | 选 A/B |
| 8 | 包名风格（如果不一致） | 低 | 选 A/B |
```

**最大质问数：8 个**（避免疲劳）

**最少质问数：3 个**（保证质量）

如果不一致点 > 8 个，**合并相似项**（比如 @Api + @ApiOperation 合成"Swagger 风格"）。

## 9 个常见不一致点模板

> 每个不一致点都有标准模板，质问时按模板填充。

### 模板 1：Controller 参数校验

**扫描命令**：
```bash
echo "A. @Valid：$(grep -r '@Valid' src --include="*Controller.java" | wc -l)"
echo "B. @Validated：$(grep -r '@Validated' src --include="*Controller.java" | wc -l)"
echo "C. 无校验：$(find src -name '*Controller.java' | xargs grep -L '@Valid\|@Validated' | wc -l)"
```

**质问模板**：见 `questions/01-controller-valid.md`

### 模板 2：Controller Swagger

**扫描命令**：
```bash
echo "A. @Api + @ApiOperation：$(grep -l '@ApiOperation' $(find src -name '*Controller.java') | wc -l)"
echo "B. 只用 @ApiOperation：$(grep -l '@ApiOperation' $(find src -name '*Controller.java') | xargs grep -L '@Api(' | wc -l)"
echo "C. 无 Swagger：$(find src -name '*Controller.java' | xargs grep -L '@Api\|@ApiOperation' | wc -l)"
```

**质问模板**：见 `questions/02-controller-swagger.md`

### 模板 3：Controller 路径前缀

**扫描命令**：
```bash
echo "A. 类级 @RequestMapping：$(grep -B1 'class.*Controller' $(find src -name '*Controller.java') | grep -c '@RequestMapping')"
echo "B. 方法级 @RequestMapping/@GetMapping/@PostMapping：..."
```

**质问模板**：见 `questions/03-controller-path.md`

### 模板 4：Service 事务注解

**扫描命令**：
```bash
echo "A. 类级 @Transactional：$(grep -B5 'class.*Impl' $(find src -name '*Impl.java') | grep -c '^@Transactional')"
echo "B. 方法级 @Transactional：$(grep -A3 'public' $(find src -name '*Impl.java') | grep -c '@Transactional')"
echo "C. 无 @Transactional：$(find src -name '*Impl.java' | xargs grep -L '@Transactional' | wc -l)"
```

**质问模板**：见 `questions/05-service-transaction.md`

### 模板 5：异常处理

**扫描命令**：
```bash
echo "A. 全局 @ControllerAdvice：$(find src -name '*ExceptionHandler.java' -o -name '*ControllerAdvice.java' | wc -l)"
echo "B. Controller 内 try-catch：$(grep -c 'try {' $(find src -name '*Controller.java'))"
echo "C. 抛自定义异常不处理：..."
```

**质问模板**：见 `questions/06-exception-handling.md`

### 模板 6：日志格式

**扫描命令**：
```bash
# 6.1 yml 配置
grep -A 5 "logging:" $(find . -name "application*.yml" | head -1)

# 6.2 logback 配置
grep "pattern" $(find . -name "logback*.xml" -o -name "logback*.yml" 2>/dev/null)
```

**质问模板**：见 `questions/07-log-format.md`

### 模板 7：依赖注入

**扫描命令**：
```bash
echo "@Autowired：$(grep -r '@Autowired' src --include='*.java' | wc -l)"
echo "@Resource：$(grep -r '@Resource' src --include='*.java' | wc -l)"
```

**质问模板**：见 `questions/` 后续扩展

### 模板 8：包名风格

**扫描命令**：
```bash
find src -name "*.java" | head -5 | xargs -I {} dirname {} | sort -u
```

**质问模板**：见 `questions/08-package-naming.md`

### 模板 9：模块命名

**扫描命令**：
```bash
grep "<module>" pom.xml
```

**质问模板**：见 `questions/09-module-naming.md`

## 推断报告输出

AI 在步骤 03 结束时输出：

```markdown
# 模式推断报告

**扫描对象**：{项目}
**不一致点数**：{N} 个
**将质问数**：{M} 个

---

## 完全一致项（直接采用）

✅ Controller 用 @RestController（100%）
✅ Impl 用 @Service（100%）
✅ 包路径风格 {具体风格}（100%）

## 不一致项（将进入步骤 04 质问）

⚠️ Controller @Valid 用法（不一致度 22%）
⚠️ Controller Swagger 注解（不一致度 48%）
⚠️ Controller 路径前缀（不一致度 35%）
⚠️ Service 事务注解（不一致度 56%）
⚠️ 异常处理（待确认）
⚠️ 日志格式（待确认）
⚠️ 依赖注入（不一致度 X%）

---

## 质问计划

将依次询问 8 个问题，**大部分有推荐项**。

预计用户操作：
- 8 个问题 × 30 秒 ≈ 4 分钟
- 选 A（推荐）的概率 ≈ 80%

进入步骤 04-ask-questions.md。
```

## 注意事项

1. **不要省略扫描命令**——每个推断都要有数据支撑
2. **不要替用户选**——只推荐，不替代
3. **不要问已经一致的问题**——100% 一致直接采用
4. **合并相似问题**——避免重复质问
5. **限制在 8 个以内**——超过会让用户疲劳
6. **优先级要标**——让用户知道哪些必须决策、哪些可以快速过
