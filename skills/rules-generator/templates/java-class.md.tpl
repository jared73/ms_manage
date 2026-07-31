---
name: java-class
description: Java 类通用规则（结构、注释、import）
---

# Java 类通用规则

> 适用于所有 `*.java` 文件，无论类类型。

## 1. 文件结构（按顺序）

```java
// 1. license/package-info（可选）
// 2. package 声明
package cn.com.{{COMPANY}}.{{PRODUCT_LINE}}.order.api.controller;

// 3. import（按字母排序，去重，不使用通配符 *）
import cn.com.{{COMPANY}}.{{PRODUCT_LINE}}.order.api.service.OrderService;
import cn.com.{{COMPANY}}.{{PRODUCT_LINE}}.common.result.Result;
import lombok.extern.slf4j.Slf4j;
import org.springframework.web.bind.annotation.*;
import javax.validation.Valid;

// 4. 类注释
/**
 * 订单控制器
 *
 * @author 张三
 * @since 2025-01-15
 */
@Slf4j  // 5. 类注解
@RestController
@RequestMapping("/order")
public class OrderController {  // 6. 类声明

    // 7. 字段（按可见性：public > protected > private）
    private final OrderService orderService;

    // 8. 构造方法 / 注入
    public OrderController(OrderService orderService) {
        this.orderService = orderService;
    }

    // 9. 公共方法
    /**
     * 创建订单
     */
    @PostMapping("/create")
    public Result<CreateOrderResp> create(@Valid @RequestBody CreateOrderReq req) {
        return Result.success(orderService.create(req));
    }

    // 10. 私有方法
    private void doSomething() {
        ...
    }
}
```

## 2. import 规则

### 顺序（强制）
1. `java.*` / `javax.*`
2. 第三方库（`org.springframework.*`、`com.baomidou.*` 等）
3. 项目内部（`cn.com.{{COMPANY}}.*`）
4. 每组之间空一行

### 规则
- ✅ 显式 import（不要 `import java.util.*;`）
- ✅ 按字母顺序排序
- ✅ 未使用的 import 必须删除
- ❌ 禁止通配符 import

## 3. 注释

### 类注释（必须）
```java
/**
 * 类简述
 *
 * 一句话详细描述（可选）
 *
 * @author 作者
 * @since 日期
 */
```

### 方法注释（必须）
```java
/**
 * 方法简述
 *
 * @param param1 参数说明
 * @param param2 参数说明
 * @return 返回值说明
 * @throws BizException 异常说明（可选）
 */
```

### 字段注释
- 简单字段可不加注释
- 业务字段、配置字段必须加 `// 说明`

### 行内注释
- 复杂逻辑必须有 `// 解释为什么`
- 简单逻辑（一眼能懂）不加注释

## 4. 命名规范

### 强制规则
- 类名：`UpperCamelCase`（如 `OrderController`）
- 方法名：`lowerCamelCase`（如 `createOrder`）
- 常量：`UPPER_SNAKE_CASE`（如 `MAX_PAGE_SIZE`）
- 包名：`lowercase`（如 `controller`、`service.impl`）
- 不要用拼音（如 `DingdanController`）
- 不要用缩写（除通用缩写：`DTO`、`Req`、`Resp`、`Impl`、`Util`）

### 命名长度
- 类名不超过 50 字符
- 方法名不超过 50 字符
- 变量名不超过 30 字符
- 常量名不超过 50 字符

## 5. 方法

### 方法长度
- 单一方法不超过 **50 行**（不含注释、空行）
- 超过 50 行必须拆方法

### 方法参数
- 参数不超过 **5 个**
- 超过 5 个用 `XxxRequest` 包装

### 返回值
- 不要返回 `null`（返回 `Optional<T>` 或空集合）
- 集合返回空集合（如 `Collections.emptyList()`），不要 `null`

## 6. 禁止项

| 类别 | 禁止 | 原因 |
|------|------|------|
| 代码 | `System.out.println` | 必须用 `log.info` |
| 代码 | `System.err.println` | 必须用 `log.error` |
| 代码 | `Thread.sleep(...)` 在业务代码 | 阻塞线程 |
| 代码 | `new Thread()` 启动线程 | 必须用线程池 |
| 代码 | 魔术数字（`if (status == 3)`） | 必须用枚举 |
| 代码 | 字符串拼接 SQL | 必须用参数化 |
| 代码 | 注释掉的代码 | 直接删除（Git 有历史） |
| 代码 | TODO 长期不处理 | 立即处理或建 ticket |
| 异常 | `catch (Exception e) {}` | 至少要 `log.error` |
| 异常 | `throw new RuntimeException(msg)` | 用 `BizException` |
| 性能 | 循环里查数据库 | 批量查 |
| 性能 | N+1 查询 | 用 `join` 或批量 |

## 7. Lombok 使用

### 允许
- `@Data`（实体类）
- `@Builder`（链式构造）
- `@NoArgsConstructor` / `@AllArgsConstructor`
- `@Slf4j`（日志）
- `@RequiredArgsConstructor`（构造注入）

### 禁止
- `@SneakyThrows`（隐藏异常）
- `@Cleanup`（自动关闭流，不如 try-with-resources）
- `@Synchronized`（性能差，用 ReentrantLock）

## 8. 测试类

- 测试类名：`{被测类}Test`（如 `OrderServiceTest`）
- 测试方法名：`{方法名}_{场景}_{预期}`（如 `create_orderNotFound_throwException`）
- 用 JUnit 5 + Mockito
- 不依赖 Spring 容器（用 `@Mock` 替代 `@Autowired`）

## 9. 静态检查

CI 会自动检查：
- `checkstyle`（命名、import、注释）
- `spotbugs`（潜在 bug）
- `pmd`（代码异味）
- `rules-checker`（本项目自定义规则）

违反以上任一规则都会导致 PR 红灯。
