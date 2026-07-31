# 质问 4/9：Controller 统一返回

## 扫描命令

```bash
echo "=== Controller 返回类型 ==="
result_count=$(grep -r "public Result<" src --include="*Controller.java" | wc -l)
response_entity=$(grep -r "public ResponseEntity<" src --include="*Controller.java" | wc -l)
plain=$(grep -r "public.*Object\|public.*Map\|public.*String" src --include="*Controller.java" | wc -l)
void=$(grep -r "public void" src --include="*Controller.java" | wc -l)

echo "A. Result<T>：$result_count"
echo "B. ResponseEntity<T>：$response_entity"
echo "C. 原始 Object/Map/String：$plain"
echo "D. void：$void"
```

## 质问内容

```markdown
## 质问 4/9：Controller 统一返回

**扫描发现**：
- A. 统一使用 Result<T>              — {a} 个 ({a%}%)  ← 推荐
- B. 使用 Spring ResponseEntity<T>  — {b} 个 ({b%}%)
- C. 返回原始 Object/Map/String      — {c} 个 ({c%}%)
- D. 其他（请说明）

**为什么 A 推荐**：
- Result<T> 是国内 Java 团队最常用的统一返回
- 包含 code、message、data 三个字段
- 前端约定一致，便于统一处理
- 比 ResponseEntity 简单（不需要处理 HttpStatus）

**方式 A 示例**：
```java
@PostMapping("/create")
public Result<CreateOrderResp> create(@Valid @RequestBody CreateOrderReq req) {
    return Result.success(orderService.create(req));
    // 或 return Result.error("ORDER_001", "订单不存在");
}
```

**方式 B 示例**：
```java
@PostMapping("/create")
public ResponseEntity<CreateOrderResp> create(@Valid @RequestBody CreateOrderReq req) {
    return ResponseEntity.ok(orderService.create(req));
    // 错误处理复杂
}
```

**方式 C 示例**（不推荐）：
```java
@PostMapping("/create")
public Map<String, Object> create(@Valid @RequestBody CreateOrderReq req) {
    Map<String, Object> result = new HashMap<>();
    result.put("code", 0);
    result.put("data", orderService.create(req));
    return result;
    // 每次都要手动包装
}
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 边界情况

### 情况 1：项目无统一返回类
```markdown
⚠️ 项目没有统一返回类
建议选 A，并定义 Result<T>：
{
  "code": 0,
  "message": "success",
  "data": {...}
}
```

### 情况 2：项目用自定义 Wrapper（如 ResponseData）
```markdown
✅ 团队使用自定义类 {XxxResponse}，效果等同 Result<T>
请选 D 并说明类名
```

## 选 A 后的 rule 内容

```markdown
## Controller 返回值
- 必须返回 Result<T>，不允许返回原始类型
- 成功：Result.success(data)
- 失败：Result.error(code, message)
- 反例：返回 Object/Map/String/ResponseEntity
```

## 选 B 后的 rule 内容

```markdown
## Controller 返回值
- 使用 Spring ResponseEntity<T>
- 成功：ResponseEntity.ok(data)
- 失败：ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR).body(...)
```
