---
name: always-on
description: 项目级硬约束（永不关闭）
---

# 项目硬约束（always-on）

> 本规则**永远生效**，AI 写代码时必须遵守。违反任何一条都会导致 CI 红灯。

## 1. 命名

### 包路径
- 格式：`{{PACKAGE_STYLE}}`
- 公司：{{COMPANY}}
- 业务线：{{PRODUCT_LINE}}
- 启动类扫描：`scanBasePackages = {"cn.com.{{COMPANY}}.{{PRODUCT_LINE}}", "cn.com.{{COMPANY}}.client"}`

### 模块命名
- 格式：`{{MODULE_STYLE}}`

### 类命名
- Controller：`{业务}Controller`
- Service 接口：`{业务}Service`
- Service 实现：`{业务}ServiceImpl`
- DTO：`{业务}DTO`
- Req：`{动作}{业务}Req`（如 `CreateOrderReq`）
- Resp：`{动作}{业务}Resp`（如 `CreateOrderResp`）
- Feign Client：`{业务}Client` 或 `{业务}Feign`
- 枚举：`{业务}Enum`
- 异常：`{业务}Exception`（业务异常）或 `BizException`（通用）
- 工具类：`{业务}Util`（不要 `Utils`）

## 2. Controller 约束

### 必须
- ✅ `@RestController`（不允许 `@Controller`）
- ✅ `@RequestMapping` 类级，提供统一前缀
- ✅ 方法用 `@GetMapping` / `@PostMapping` / `@PutMapping` / `@DeleteMapping`
- ✅ 入参用 `@Valid @RequestBody XxxReq req`（或 `@RequestParam`）
- ✅ Swagger：{{CONTROLLER_SWAGGER}}
- ✅ 返回 `Result<T>`（或团队统一的 `XxxResponse<T>`）

### 禁止
- ❌ `@Controller`（必须 `@RestController`）
- ❌ 缺 `{{CONTROLLER_VALID}}` 校验
- ❌ Controller 内 `try-catch`（异常交给全局处理器）
- ❌ 在 Controller 写业务逻辑
- ❌ 返回原始 `Object` / `Map` / `String` / `ResponseEntity`

## 3. Service 约束

### Service 接口（`*Service.java`）
- ✅ 纯接口，不加任何 Spring 注解
- ✅ 方法签名清晰，参数用 `Req/Resp/DTO`
- ✅ Javadoc 注释业务含义

### Service 实现（`*ServiceImpl.java`）
- ✅ 类级 `@Service`
- ✅ 类级 `@Transactional`（{{SERVICE_TRANSACTION}}）
- ✅ 实现接口，不要乱加 public 方法
- ✅ 抛 `BizException` 而不是 `RuntimeException`
- ✅ 复杂业务用 Plugin 责任链模式（参考 `.spec/{productLine}-spec-plugin-dispatcher.md`）

## 4. 异常处理（{{EXCEPTION_HANDLING}}）

### 全局处理（A 模式）
- ✅ 自定义 `BizException extends RuntimeException`
- ✅ 全局 `@RestControllerAdvice` 处理
- ✅ 错误码格式：`{{PRODUCT_LINE_UPPER}}_{4位数字}`（如 `OC_0001`）
- ✅ Controller 不写 try-catch

### 禁止
- ❌ 抛原始 `RuntimeException`（必须用 `BizException`）
- ❌ Controller 内 try-catch
- ❌ 吞掉异常（catch 后什么都不做）
- ❌ 异常信息写死中文（用错误码 + 国际化）

## 5. 日志（{{LOG_FORMAT}}）

### 必须
- ✅ SLF4J + Logback（不允许 `System.out.println`）
- ✅ `@Slf4j` 注解注入
- ✅ 占位符 `{}` 写法（如 `log.info("创建订单, userId={}", userId)`）
- ❌ 禁止字符串拼接（`log.info("创建订单, userId=" + userId)`）

### 业务日志模板
```java
// 入参日志
log.info("[业务名] 开始, 参数={}", req);

// 关键节点
log.info("[业务名] 数据库操作, orderId={}", orderId);

// 异常日志
log.error("[业务名] 失败, orderId={}", orderId, e);
```

## 6. 数据库

- ✅ MyBatis Plus（推荐）或 MyBatis
- ✅ 表名：`t_{业务名}`（如 `t_order`）
- ✅ 主键：`id BIGINT AUTO_INCREMENT` 或 `id VARCHAR(32)`（雪花算法）
- ✅ 必备字段：`create_time` / `update_time` / `create_by` / `update_by` / `deleted`（逻辑删除）
- ✅ 事务在 Service 层（不是 Controller，不是 Mapper）

## 7. 注释与文档

### 类注释
```java
/**
 * 订单服务实现
 *
 * @author 张三
 * @since 2025-01-15
 */
@Service
public class OrderServiceImpl implements OrderService {
    ...
}
```

### 方法注释
```java
/**
 * 创建订单
 *
 * @param req 创建订单请求
 * @return 订单详情
 * @throws BizException 业务异常
 */
@Override
public CreateOrderResp create(CreateOrderReq req) {
    ...
}
```

## 8. 反例清单（绝对禁止）

| 类别 | 反例 | 正确做法 |
|------|------|---------|
| Controller | `@Controller` | `@RestController` |
| Controller | `public Object create(...)` | `public Result<CreateOrderResp> create(...)` |
| Controller | `@RequestBody XxxReq req` | `@Valid @RequestBody XxxReq req` |
| Service | `@Service public class OrderService` | 接口不加注解，Impl 加 `@Service` |
| Service | 抛 `throw new RuntimeException("订单不存在")` | 抛 `throw new BizException("OC_0001", "订单不存在")` |
| 日志 | `System.out.println("xxx")` | `log.info("xxx")` |
| 日志 | `log.info("userId=" + userId)` | `log.info("userId={}", userId)` |
| 命名 | `class OrderController2` | `class OrderController`（按业务命名，不按数字） |
| 命名 | `class OrderUtils` | `class OrderUtil`（不要 `Utils`） |

## 9. 触发 CI 检查

PR 提交时，CI 会运行 `rules-checker` 检查以上所有约束。
任何一条违反都会导致 PR 无法合并。
