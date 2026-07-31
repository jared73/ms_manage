# 质问 1/9：Controller 参数校验

## 扫描命令

```bash
echo "=== 参数校验注解分布 ==="
echo "A. @Valid 使用数：$(grep -r '@Valid' src --include="*Controller.java" | wc -l)"
echo "B. @Validated 使用数：$(grep -r '@Validated' src --include="*Controller.java" | wc -l)"
echo "C. 无校验 Controller 数：$(find src -name '*Controller.java' | xargs grep -L '@Valid\|@Validated' | wc -l)"
echo "总 Controller 数：$(find src -name '*Controller.java" | wc -l)"
```

## 质问内容

```markdown
## 质问 1/9：Controller 参数校验

**扫描发现**：
- A. @RequestBody @Valid XxxReq req     — {a} 个 ({a%}%)  ← 推荐
- B. @RequestBody XxxReq req（无校验）   — {b} 个 ({b%}%)
- C. @RequestBody @Validated XxxReq req  — {c} 个 ({c%}%)
- D. 其他（请说明）

**为什么 A 推荐**：
- @Valid 是 JSR-303 标准，最通用
- Spring MVC 原生支持
- 99% 团队使用 @Valid 做参数校验
- @Validated 主要用于"嵌套校验"和"分组校验"场景

**方式 A 示例**：
```java
@PostMapping("/create")
public Result<CreateOrderResp> createOrder(
    @Valid @RequestBody CreateOrderReq req  // ← @Valid 在 @RequestBody 前
) {
    return Result.success(orderService.create(req));
}
```

**方式 B 示例**（不推荐，容易漏校验）：
```java
@PostMapping("/create")
public Result<CreateOrderResp> createOrder(
    @RequestBody CreateOrderReq req  // ← 缺 @Valid，参数不会被校验
) {
    return Result.success(orderService.create(req));
}
```

**方式 C 示例**（用于特殊场景）：
```java
@PostMapping("/create")
public Result<CreateOrderResp> createOrder(
    @Validated @RequestBody CreateOrderReq req  // ← 用于嵌套校验
) {
    return Result.success(orderService.create(req));
}
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 边界情况

### 情况 1：项目没有 Controller
```markdown
⚠️ 项目中未发现 Controller，跳过本质问。
```

### 情况 2：项目全是 @Valid（100%）
```markdown
✅ 项目中所有 Controller 都用 @Valid（100% 一致）
直接采用 A，无需质问。
```

### 情况 3：项目同时混用 3 种以上
```markdown
⚠️ 项目混用多种方式，强烈建议统一为 A。
```

## 替换占位符

| 占位符 | 替换为 |
|--------|--------|
| `{a}` | @Valid 数量 |
| `{a%}` | @Valid 占比 |
| `{b}` | 无校验数量 |
| `{b%}` | 无校验占比 |
| `{c}` | @Validated 数量 |
| `{c%}` | @Validated 占比 |

## 选 A 后的 rule 内容

```markdown
## Controller 参数校验
- 必须使用 @Valid 注解（不允许无校验）
- 位置：@Valid 必须在 @RequestBody 之前
- 反例：`@RequestBody XxxReq req`（缺 @Valid，不允许）
```

## 选 B 后的 rule 内容

```markdown
## Controller 参数校验
- 暂不强制校验（业务自行保证）
- 不允许：未来如果加校验，统一用 @Valid
```

## 选 C 后的 rule 内容

```markdown
## Controller 参数校验
- 必须使用 @Validated 注解
- 位置：@Validated 必须在 @RequestBody 之前
- 理由：团队硬性要求（如有，请说明）
```
