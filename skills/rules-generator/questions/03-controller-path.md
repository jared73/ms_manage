# 质问 3/9：Controller 路径前缀

## 扫描命令

```bash
echo "=== @RequestMapping 位置分布 ==="
total=$(find src -name "*Controller.java" | wc -l)
class_level=$(grep -B5 "class.*Controller" $(find src -name "*Controller.java") | grep -c "@RequestMapping\|@GetMapping\|@PostMapping")
class_only=$(grep -B5 "class.*Controller" $(find src -name "*Controller.java") | grep -c "@RequestMapping")
method_level=$(find src -name "*Controller.java" | xargs grep -A 1 "public.*Result\|public.*ResponseEntity" | grep -c "@GetMapping\|@PostMapping\|@PutMapping\|@DeleteMapping\|@RequestMapping")

echo "A. 类级 @RequestMapping 完整路径：$class_only 个"
echo "B. 方法级 @GetMapping/@PostMapping：$method_level 个"
echo "C. 类级 + 方法级 组合：待分析"
echo "总 Controller：$total"
```

## 质问内容

```markdown
## 质问 3/9：Controller 路径前缀

**扫描发现**：
- A. 类级 @RequestMapping（统一前缀）   — {a} 个 ({a%}%)  ← 推荐
- B. 仅方法级 @GetMapping/@PostMapping — {b} 个 ({b%}%)
- C. 类级前缀 + 方法级路径（组合）       — {c} 个 ({c%}%)
- D. 其他（请说明）

**为什么 A 推荐**：
- 类级统一前缀，避免重复
- 路径结构清晰（`/order/create`、`/order/update`）
- 便于权限控制（按前缀拦截）
- 团队最容易理解和维护

**方式 A 示例**（类级统一前缀）：
```java
@RestController
@RequestMapping("/order")  // ← 类级统一前缀
public class OrderController {

    @PostMapping("/create")  // ← 实际路径：/order/create
    public Result<CreateOrderResp> create(...) { ... }

    @PostMapping("/update")  // ← 实际路径：/order/update
    public Result<UpdateOrderResp> update(...) { ... }
}
```

**方式 B 示例**（方法级独立路径）：
```java
@RestController
public class OrderController {

    @PostMapping("/order/create")  // ← 完整路径写在方法上
    public Result<CreateOrderResp> create(...) { ... }

    @PostMapping("/order/update")  // ← 每个方法都要重复
    public Result<UpdateOrderResp> update(...) { ... }
}
```

**方式 C 示例**（组合）：
```java
@RestController
@RequestMapping("/api/v1")  // ← 公共前缀
public class OrderController {

    @PostMapping("/order/create")  // ← 每个方法再写路径
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

### 情况 1：所有 Controller 都用 A（100%）
```markdown
✅ 项目 100% 使用类级 @RequestMapping，直接采用 A。
```

### 情况 2：项目用网关统一前缀
```markdown
💡 提示：如果网关层已经统一加了 /api 前缀，
Controller 可以省略 /api 部分。
```

## 选 A 后的 rule 内容

```markdown
## Controller 路径
- 类级必须 @RequestMapping("/{模块名}")，提供统一前缀
- 方法级用 @GetMapping/@PostMapping 等，只写相对路径
- 反例：方法级写完整路径（如 /order/create），应改为 /create
```

## 选 C 后的 rule 内容

```markdown
## Controller 路径
- 类级 @RequestMapping 提供版本前缀（如 /api/v1）
- 方法级提供业务路径
- 完整路径 = 类级前缀 + 方法级路径
```
