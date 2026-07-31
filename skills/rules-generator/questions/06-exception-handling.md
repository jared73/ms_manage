# 质问 6/9：异常处理

## 扫描命令

```bash
echo "=== 异常处理方式 ==="
advice=$(find src -name "*ControllerAdvice.java" -o -name "*ExceptionHandler.java" | wc -l)
controller_try=$(grep -c "try {" $(find src -name "*Controller.java") 2>/dev/null || echo 0)
throw_custom=$(grep -r "throw new BizException\|throw new BusinessException\|throw new ServiceException" src --include="*Service*.java" | wc -l)
throw_runtime=$(grep -r "throw new RuntimeException" src --include="*Service*.java" | wc -l)

echo "A. 全局 @ControllerAdvice：$advice 个"
echo "B. Controller 内 try-catch：$controller_try 处"
echo "C. 抛自定义业务异常：$throw_custom 处"
echo "D. 抛 RuntimeException：$throw_runtime 处"
```

## 质问内容

```markdown
## 质问 6/9：异常处理

**扫描发现**：
- A. 全局 @ControllerAdvice + 业务异常类  — {a} 处  ← 推荐
- B. 每个 Controller 内 try-catch           — {b} 处
- C. 抛自定义业务异常（无全局处理）          — {c} 处
- D. 其他（请说明）

**为什么 A 推荐**：
- 统一异常入口，便于日志、监控、国际化
- 减少 Controller 中的 try-catch 样板代码
- 业务异常结构化（带错误码）
- Spring 官方推荐方式

**方式 A 示例**：
```java
// 1. 业务异常类
public class BizException extends RuntimeException {
    private String code;
    private String message;
    public BizException(String code, String message) {
        super(message);
        this.code = code;
    }
}

// 2. 全局处理器
@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(BizException.class)
    public Result<Void> handleBiz(BizException e) {
        log.warn("业务异常：{}", e.getMessage());
        return Result.error(e.getCode(), e.getMessage());
    }

    @ExceptionHandler(Exception.class)
    public Result<Void> handleException(Exception e) {
        log.error("系统异常", e);
        return Result.error("SYSTEM_ERROR", "系统繁忙");
    }
}

// 3. Service 抛异常
@Service
public class OrderServiceImpl implements OrderService {
    @Override
    public OrderDetail getById(String id) {
        Order order = orderMapper.selectById(id);
        if (order == null) {
            throw new BizException("ORDER_001", "订单不存在");
            // ↑ 全局处理器自动捕获，返回统一格式
        }
        return OrderDetail.from(order);
    }
}
```

**方式 B 示例**（不推荐，代码冗余）：
```java
@PostMapping("/create")
public Result<CreateOrderResp> create(@Valid @RequestBody CreateOrderReq req) {
    try {
        return Result.success(orderService.create(req));
    } catch (BizException e) {
        return Result.error(e.getCode(), e.getMessage());
    } catch (Exception e) {
        log.error("系统异常", e);
        return Result.error("SYSTEM_ERROR", "系统繁忙");
    }
    // ↑ 每个 Controller 都要写一遍
}
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 边界情况

### 情况 1：项目已有全局异常处理
```markdown
✅ 项目已有 GlobalExceptionHandler，扫描到 $advice 个
直接采用 A
```

### 情况 2：项目用 hutool 的 Exceptions 工具
```markdown
💡 A 选中后，可以用 hutool 的 Exceptions 工具简化
```

## 选 A 后的 rule 内容

```markdown
## 异常处理

### 异常体系
- 自定义 BizException（继承 RuntimeException）
- 全局 GlobalExceptionHandler（@RestControllerAdvice）
- 错误码：{项目代号}_{4位数字}，如 LMS_0001

### 处理流程
- Service 抛 BizException
- Controller 不写 try-catch
- 全局处理器统一处理 + 日志 + 监控

### 禁止
- ❌ Controller 内 try-catch
- ❌ 抛原始 RuntimeException
- ❌ 吞掉异常（catch 后什么都不做）
```

## 选 B 后的 rule 内容

```markdown
## 异常处理
- Controller 内 try-catch
- 不使用全局处理器
- 适用：项目较小 / 异常处理逻辑不统一
```
