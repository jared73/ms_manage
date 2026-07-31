# 质问 7/9：日志格式

## 扫描命令

```bash
echo "=== 日志配置 ==="
# 1. application.yml 中的 logging 配置
grep -A 5 "logging:" $(find . -name "application*.yml" 2>/dev/null | head -1) | head -10

# 2. logback 配置文件
ls -la src/main/resources/logback*.xml src/main/resources/logback*.yml 2>/dev/null

# 3. logback 中的 pattern
if [ -f "src/main/resources/logback-spring.xml" ]; then
  echo "logback-spring.xml 存在"
  grep "pattern\|encoder" src/main/resources/logback-spring.xml | head -5
fi

# 4. 日志使用
log_info=$(grep -r "log.info\|logger.info" src --include="*.java" | wc -l)
log_error=$(grep -r "log.error\|logger.error" src --include="*.java" | wc -l)
sysout=$(grep -r "System.out.println" src --include="*.java" | wc -l)

echo "log.info 使用：$log_info 处"
echo "log.error 使用：$log_error 处"
echo "System.out.println：$sysout 处"
```

## 质问内容

```markdown
## 质问 7/9：日志格式

**扫描发现**：
- A. JSON 格式（结构化，推荐用于 ELK）  — 配置中 pattern 含 %json  ← 推荐
- B. 文本格式（含时间/级别/类名）        — 配置中 pattern 为常规模式
- C. 默认（未自定义 pattern）            — 未找到 logback 配置
- D. 其他（请说明）

**为什么 A 推荐（如果是 ELK 场景）**：
- 字段化便于 ELK 检索
- 适合云原生 / K8s 环境
- Logstash 解析简单

**为什么 B 推荐（如果是传统部署）**：
- 人眼可读，调试方便
- 适合本地开发
- 传统部署（虚拟机）

**A 示例**（logback-spring.xml）：
```xml
<configuration>
    <appender name="STDOUT" class="ch.qos.logback.core.ConsoleAppender">
        <encoder class="ch.qos.logback.classic.encoder.JsonEncoder">
            <!-- 输出 JSON 格式 -->
            <timestampPattern>yyyy-MM-dd'T'HH:mm:ss.SSSXXX</timestampPattern>
        </encoder>
    </appender>
    <root level="INFO">
        <appender-ref ref="STDOUT" />
    </root>
</configuration>
```

**B 示例**（传统文本格式）：
```xml
<configuration>
    <appender name="STDOUT" class="ch.qos.logback.core.ConsoleAppender">
        <encoder>
            <pattern>%d{yyyy-MM-dd HH:mm:ss.SSS} [%thread] %-5level %logger{36} - %msg%n</pattern>
        </encoder>
    </appender>
    <root level="INFO">
        <appender-ref ref="STDOUT" />
    </root>
</configuration>
```

**代码使用**（两种格式都适用）：
```java
@Slf4j
@Service
public class OrderServiceImpl implements OrderService {
    @Override
    public CreateOrderResp create(CreateOrderReq req) {
        log.info("创建订单, userId={}, amount={}", req.getUserId(), req.getAmount());
        // ↑ 占位符 {}，不拼接字符串
        return orderMapper.insert(req);
    }
}
```

请选择：
- [ ] A. JSON
- [ ] B. 文本
- [ ] C. 默认
- [ ] D
```

## 边界情况

### 情况 1：项目无 logback 配置
```markdown
⚠️ 项目无 logback 配置
Spring Boot 默认是文本格式（建议选 B 或自定义）
```

### 情况 2：项目用 Log4j2
```markdown
💡 项目用 log4j2（pom.xml 含 log4j2 依赖）
对应配置是 log4j2.xml/log4j2-spring.xml
```

## 选 A 后的 rule 内容

```markdown
## 日志

### 框架
- SLF4J + Logback（必须）
- 不允许使用 System.out.println

### 格式
- JSON 格式（logback-spring.xml + JsonEncoder）
- 字段：timestamp、level、logger、thread、message、stack

### 使用
- 用 @Slf4j 注入
- 占位符 {}，不拼接字符串
- 业务日志：log.info("创建订单, 参数={}", req);
- 异常日志：log.error("订单创建失败", e);
```

## 选 B 后的 rule 内容

```markdown
## 日志

### 框架
- SLF4J + Logback（必须）
- 不允许使用 System.out.println

### 格式
- 文本格式（%d %level %logger %msg）
- 人眼可读

### 使用
- 用 @Slf4j 注入
- 占位符 {}，不拼接字符串
```
