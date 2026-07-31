# 质问 2/9：Controller Swagger 注解

## 扫描命令

```bash
echo "=== Swagger 注解分布 ==="
total=$(find src -name "*Controller.java" | wc -l)
full=$(find src -name "*Controller.java" | xargs grep -l '@Api(' 2>/dev/null | xargs grep -l '@ApiOperation' 2>/dev/null | wc -l)
op_only=$(find src -name "*Controller.java" | xargs grep -l '@ApiOperation' 2>/dev/null | xargs grep -L '@Api(' 2>/dev/null | wc -l)
none=$(find src -name "*Controller.java" | xargs grep -L '@Api\|@ApiOperation' 2>/dev/null | wc -l)

echo "A. @Api + @ApiOperation 双注解：$full"
echo "B. 仅 @ApiOperation：$op_only"
echo "C. 无 Swagger 注解：$none"
echo "总：$total"
```

## 质问内容

```markdown
## 质问 2/9：Controller Swagger 注解

**扫描发现**：
- A. @Api + @ApiOperation 双注解    — {a} 个 ({a%}%)  ← 推荐
- B. 仅 @ApiOperation（无 @Api）    — {b} 个 ({b%}%)
- C. 无任何 Swagger 注解             — {c} 个 ({c%}%)
- D. 其他（请说明）

**为什么 A 推荐**：
- @Api 用于"Controller 分组"，@ApiOperation 用于"方法描述"
- 两者配合，Swagger UI 才完整
- 单独用 @ApiOperation 没有分组，UI 杂乱
- 完全不用 Swagger 无法自动生成接口文档

**方式 A 示例**：
```java
@Api(tags = "订单管理")                  // ← Controller 分组
@RestController
@RequestMapping("/order")
public class OrderController {

    @ApiOperation("创建订单")             // ← 方法描述
    @PostMapping("/create")
    public Result<CreateOrderResp> create(
        @Valid @RequestBody CreateOrderReq req) {
        return Result.success(orderService.create(req));
    }
}
```

**方式 B 示例**（UI 杂乱）：
```java
@RestController  // ← 缺 @Api
@RequestMapping("/order")
public class OrderController {

    @ApiOperation("创建订单")  // ← 只有方法描述
    @PostMapping("/create")
    public Result<CreateOrderResp> create(...) { ... }
}
```

**方式 C 示例**（无文档）：
```java
@RestController
@RequestMapping("/order")
public class OrderController {
    // 无任何 Swagger 注解，Swagger UI 看不到
    @PostMapping("/create")
    public Result<CreateOrderResp> create(...) { ... }
}
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 边界情况

### 情况 1：项目使用 springdoc（替代 springfox）
```markdown
⚠️ 项目使用 springdoc-openapi（application.yml 有 springdoc 配置）
对应注解应该是 @Tag + @Operation（springdoc 风格）
```

### 情况 2：项目完全不用 Swagger
```markdown
⚠️ 项目无任何 Swagger 注解
- 可能团队选择用 knife4j / apifox / yapi 等替代
- 建议选 C（无 Swagger）
```

### 情况 3：项目用 springdoc 但注解写错
```markdown
⚠️ 项目用 springdoc，但部分 Controller 用了 springfox 的 @Api
建议统一为 springdoc 注解
```

## 选 A 后的 rule 内容

```markdown
## Controller Swagger
- 类级必须 @Api(tags = "{分组名}")
- 方法级必须 @ApiOperation("{方法描述}")
- 反例：缺 @Api 或 @ApiOperation
```

## 选 C 后的 rule 内容

```markdown
## Controller Swagger
- 暂不要求 Swagger 注解
- 后续如果要引入，统一用 springdoc-openapi
```

## 选 D（springdoc）后的 rule 内容

```markdown
## Controller Swagger（springdoc 风格）
- 类级必须 @Tag(name = "{分组名}")
- 方法级必须 @Operation(summary = "{方法描述}")
- 不允许使用 springfox 的 @Api/@ApiOperation
```
