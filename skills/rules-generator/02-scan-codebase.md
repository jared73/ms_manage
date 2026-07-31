# 02 · 代码扫描

> 目标：从代码中提取 6 个维度的客观数据，作为后续推断和质问的依据。
>
> **原则**：只看事实，不做主观判断。所有"哪种实现更好"的判断留给步骤 03 和用户。

## 扫描维度总览

| 维度 | 输入 | 输出 |
|------|------|------|
| 1. 模块结构 | pom.xml + 目录树 | 模块清单 |
| 2. 类后缀分布 | `find . -name "*.java"` | 类后缀频次表 |
| 3. 命名模式 | grep 包路径/类名 | 命名样本 |
| 4. 注解模式 | grep 注解 | 注解频次表 |
| 5. 技术栈 | pom.xml + application.yml | 依赖清单 |
| 6. 配置文件 | application.yml/properties | 配置样本 |

---

## 维度 1：模块结构

### 扫描命令

```bash
# 1.1 检查是否为多模块项目
if grep -q "<modules>" pom.xml 2>/dev/null; then
  echo "多模块项目"
  grep -A 30 "<modules>" pom.xml | grep "<module>" | sed 's/.*<module>\(.*\)<\/module>.*/\1/'
else
  echo "单模块项目"
fi

# 1.2 列出所有 pom.xml
find . -name "pom.xml" -not -path "*/target/*" -not -path "*/node_modules/*" | sort

# 1.3 对每个模块，看 application 类
find . -name "*Application.java" -not -path "*/target/*" | sort
```

### 输出格式

```markdown
## 模块清单

| 模块名 | 类型 | Application 类 | 估计职责 |
|--------|------|---------------|---------|
| oc-order-api | ms | OrderApiApplication | API 网关 |
| oc-order-service | ms | OrderServiceApplication | 订单服务 |
| oc-order-client | client | - | Feign 客户端 |
| oc-common | common | - | 公共类 |
```

**类型判断规则**（启发式）：
- 含 `*Application.java` → ms
- 含 `*Client.java` 或 `@FeignClient` → client
- 含 `*Plugin.java` 或 `extends AbstractPlugin` → plugin
- 含 `application.yml` + `*Mapper.java` → service
- 无 main 启动类 + 含工具类 → common
- 含 `*Producer.java` + RocketMQ 依赖 → producer

---

## 维度 2：类后缀分布

### 扫描命令

```bash
# 2.1 列出所有 Java 文件
find src -name "*.java" -not -path "*/target/*" | wc -l

# 2.2 按后缀统计
find src -name "*.java" -not -path "*/target/*" | \
  sed -n 's/.*\([A-Z][A-Za-z0-9]*\)\.java$/\1/p' | \
  sort | uniq -c | sort -rn
```

### 输出格式

```markdown
## 类后缀分布

| 后缀 | 数量 | 占比 | 推断类型 |
|------|------|------|---------|
| Controller | 23 | 8.5% | Web 控制器 |
| Service | 45 | 16.7% | 服务接口 |
| Impl | 45 | 16.7% | 服务实现 |
| DTO | 67 | 24.8% | 数据传输 |
| Req | 89 | 33.0% | 请求对象 |
| Resp | 78 | 28.9% | 响应对象 |
| Client | 12 | 4.4% | Feign 客户端 |
| Plugin | 8 | 3.0% | 插件 |
| Producer | 6 | 2.2% | MQ 生产者 |
| Enum | 15 | 5.6% | 枚举 |
| Exception | 4 | 1.5% | 异常 |
| Util | 18 | 6.7% | 工具类 |
| ... | ... | ... | ... |

**注意**：百分比可能 > 100%，因为一个类可能同时含多个后缀（如 OrderServiceImpl）
```

---

## 维度 3：命名模式

### 扫描命令

```bash
# 3.1 包路径风格
find src -name "*.java" -not -path "*/target/*" | \
  head -20 | \
  xargs -I {} dirname {} | sort -u

# 3.2 类名样本
find src -name "*Controller.java" | head -5
find src -name "*Service.java" | head -5
find src -name "*ServiceImpl.java" | head -5
find src -name "*DTO.java" | head -5

# 3.3 方法名样本（Controller）
grep -h "public.*Mapping\|public Result<" $(find src -name "*Controller.java") | head -10
```

### 输出格式

```markdown
## 命名样本

### 包路径风格
- cn.com.yto56.oc.order.api.controller
- cn.com.yto56.oc.order.service.impl
- cn.com.yto56.oc.common.enums

**判断**：
- 公司前缀：cn.com.yto56
- 业务线代号：oc
- 模块名：order
- 层名：controller/service/impl/...
- 风格：{公司}.{业务线}.{模块}.{层}

### 类名样本
| 类型 | 样本 | 推断命名规则 |
|------|------|------------|
| Controller | OrderCreateController | {业务}Controller |
| Service | OrderService | {业务}Service |
| Impl | OrderServiceImpl | {业务}ServiceImpl |
| DTO | OrderDTO | {业务}DTO |
| Req | CreateOrderReq | {动作}{业务}Req |
| Resp | CreateOrderResp | {动作}{业务}Resp |
```

---

## 维度 4：注解模式（重点）

> **这是推断"多种实现方式"的核心维度**。每个注解用法都要统计频次。

### 4.1 Controller 注解

```bash
# 4.1.1 统计 @RestController / @Controller
echo "=== @RestController vs @Controller ==="
grep -c "@RestController" $(find src -name "*Controller.java") | grep -v ":0$" | wc -l
echo "@RestController 使用数：$(grep -r "@RestController" src --include="*Controller.java" | wc -l)"
echo "@Controller 使用数：$(grep -r "^@Controller" src --include="*Controller.java" | wc -l)"

# 4.1.2 统计 @RequestMapping 位置
echo "=== @RequestMapping 位置 ==="
echo "类级：$(grep -B1 "class.*Controller" $(find src -name "*Controller.java") | grep -c "@RequestMapping")"
echo "方法级：$(grep -A2 "public.*Mapping(" $(find src -name "*Controller.java") | grep -c "@RequestMapping\|@GetMapping\|@PostMapping")"

# 4.1.3 统计 @Valid / @Validated
echo "=== 参数校验注解 ==="
echo "@Valid：$(grep -r "@Valid" src --include="*Controller.java" | wc -l) 处"
echo "@Validated：$(grep -r "@Validated" src --include="*Controller.java" | wc -l) 处"
echo "无校验：$(find src -name "*Controller.java" | xargs grep -L "@Valid\|@Validated" | wc -l) 个 Controller"

# 4.1.4 统计 Swagger 注解
echo "=== Swagger 注解 ==="
echo "@Api：$(grep -r "@Api" src --include="*Controller.java" | wc -l) 处"
echo "@ApiOperation：$(grep -r "@ApiOperation" src --include="*Controller.java" | wc -l) 处"
echo "无 Swagger：$(find src -name "*Controller.java" | xargs grep -L "@Api" | wc -l) 个 Controller"
```

### 4.2 Service 注解

```bash
# 4.2.1 统计 @Service
echo "=== @Service 分布 ==="
echo "Service 接口：$(find src -name "*Service.java" ! -name "*Impl.java" | xargs grep -L "@Service" | wc -l) 个无注解（正常）"
echo "Impl 类：$(find src -name "*Impl.java" | xargs grep -c "@Service" | grep -v ":0$" | wc -l) 个有 @Service"
echo "Impl 无 @Service：$(find src -name "*Impl.java" | xargs grep -L "@Service" | wc -l) 个"

# 4.2.2 统计 @Transactional
echo "=== @Transactional 分布 ==="
echo "类级：$(grep -B5 "class.*Impl" $(find src -name "*Impl.java") | grep -c "^@Transactional$\|@Transactional(" )"
echo "方法级：$(grep -A3 "public" $(find src -name "*Impl.java") | grep -c "@Transactional")"
echo "无事务：$(find src -name "*Impl.java" | xargs grep -L "@Transactional" | wc -l) 个 Impl"
```

### 4.3 其他注解

```bash
# 4.3.1 @Component / @Repository / @Configuration
echo "=== 通用 Spring 注解 ==="
for ann in "@Component" "@Repository" "@Configuration" "@Bean" "@Autowired" "@Resource"; do
  count=$(grep -r "$ann" src --include="*.java" 2>/dev/null | wc -l)
  echo "$ann：$count 处"
done

# 4.3.2 Lombok
echo "=== Lombok ==="
for ann in "@Data" "@Builder" "@NoArgsConstructor" "@AllArgsConstructor" "@Slf4j"; do
  count=$(grep -r "$ann" src --include="*.java" 2>/dev/null | wc -l)
  echo "$ann：$count 处"
done

# 4.3.3 日志框架
echo "=== 日志框架 ==="
echo "slf4j：$(grep -r "import org.slf4j" src --include="*.java" | wc -l) 处"
echo "log4j：$(grep -r "import org.apache.log4j" src --include="*.java" | wc -l) 处"
echo "logback：$(grep -r "import ch.qos.logback" src --include="*.java" | wc -l) 处"
```

### 输出格式

```markdown
## 注解模式

### Controller 注解分布
| 注解 | 频次 | 占比 | 备注 |
|------|------|------|------|
| @RestController | 23/23 | 100% | ✓ 一致 |
| @Controller | 0/23 | 0% | ✓ |
| @RequestMapping（类级） | 8/23 | 35% | ⚠️ 不一致 |
| @RequestMapping（方法级） | 23/23 | 100% | ✓ |
| @Valid | 18/23 | 78% | ⚠️ 不一致 |
| @Validated | 2/23 | 9% | ⚠️ 不一致 |
| 无校验 | 3/23 | 13% | ⚠️ 不一致 |
| @Api | 12/23 | 52% | ⚠️ 不一致 |
| @ApiOperation | 12/23 | 52% | ⚠️ 不一致 |
| 无 Swagger | 11/23 | 48% | ⚠️ 不一致 |

### Service 注解分布
| 注解 | 频次 | 占比 | 备注 |
|------|------|------|------|
| @Service（在 Impl） | 45/45 | 100% | ✓ |
| @Transactional（类级） | 20/45 | 44% | ⚠️ 不一致 |
| @Transactional（方法级） | 15/45 | 33% | ⚠️ 不一致 |
| 无 @Transactional | 10/45 | 22% | ⚠️ 不一致 |

### 其他注解
| 注解/特性 | 频次 | 备注 |
|----------|------|------|
| @Autowired | 23 处 | |
| @Resource | 89 处 | ⚠️ 两种混用 |
| Lombok @Data | 234 处 | ✓ 主流 |
| Lombok @Slf4j | 189 处 | ✓ 主流 |
| slf4j 日志 | 189 处 | ✓ 统一 |
```

---

## 维度 5：技术栈

### 扫描命令

```bash
# 5.1 Maven 依赖
echo "=== Spring Boot 版本 ==="
grep -A 2 "spring-boot-starter-parent" pom.xml | grep version

echo ""
echo "=== Spring Cloud 版本 ==="
grep -A 2 "spring-cloud-dependencies" pom.xml | grep version

echo ""
echo "=== Java 版本 ==="
grep -A 2 "java.version\|maven.compiler.source" pom.xml | head -5

echo ""
echo "=== 关键依赖 ==="
for dep in "spring-boot-starter-web" "spring-boot-starter-data-jpa" "spring-boot-starter-data-redis" "mybatis-plus" "rocketmq" "kafka" "mysql-connector-java" "lombok" "hutool" "fastjson" "jackson" "swagger" "springdoc"; do
  count=$(grep -c "$dep" $(find . -name "pom.xml"))
  if [ "$count" -gt 0 ]; then
    version=$(grep -A 3 "$dep" $(find . -name "pom.xml" | head -1) | grep version | head -1)
    echo "✓ $dep $version"
  fi
done
```

### 输出格式

```markdown
## 技术栈清单

| 技术 | 版本 | 用途 |
|------|------|------|
| Java | 8/11/17 | 主语言 |
| Spring Boot | 2.7.18 | 主框架 |
| Spring Cloud | 2021.0.8 | 微服务 |
| MyBatis Plus | 3.5.3 | ORM |
| MySQL Driver | 8.0.33 | 数据库驱动 |
| Druid | 1.2.16 | 连接池 |
| Redis (Lettuce) | 6.x | 缓存 |
| RocketMQ | 5.x | 消息队列 |
| Nacos | 2022.x | 注册/配置中心 |
| Sentinel | 1.8.x | 熔断限流 |
| Lombok | 1.18.30 | 代码简化 |
| Hutool | 5.8.x | 工具库 |
| FastJSON | 2.0.x | JSON |
| Swagger (springdoc) | 2.x | 接口文档 |
```

---

## 维度 6：配置文件

### 扫描命令

```bash
# 6.1 列出配置文件
find . -name "application*.yml" -o -name "application*.properties" -o -name "bootstrap*.yml" 2>/dev/null | grep -v target

# 6.2 抽样第一个配置文件
if [ -f "src/main/resources/application.yml" ]; then
  echo "=== application.yml 摘要 ==="
  head -50 src/main/resources/application.yml
fi

# 6.3 统计配置项
echo "=== 数据库配置 ==="
grep -A 2 "datasource:" $(find . -name "application*.yml" | head -1)

echo "=== Redis 配置 ==="
grep -A 2 "redis:" $(find . -name "application*.yml" | head -1)

echo "=== 日志格式 ==="
grep -A 5 "logging:" $(find . -name "application*.yml" | head -1) | head -10

echo "=== 日志格式（pattern）==="
grep "pattern" $(find . -name "logback*.xml" -o -name "logback*.yml" 2>/dev/null) | head -5
```

### 输出格式

```markdown
## 配置文件

### 配置风格
- 主配置：application.yml
- 环境配置：application-dev.yml / application-prod.yml
- 启动配置：bootstrap.yml

### 关键配置
| 配置 | 当前值 | 备注 |
|------|--------|------|
| 数据库 | MySQL 8.0 + Druid | |
| 缓存 | Redis (单节点) | |
| MQ | RocketMQ 5.x | |
| 日志 | JSON 格式 / 文本格式 | ⚠️ 待确认 |
| 编码 | UTF-8 | |
| 时区 | GMT+8 | |
```

---

## 扫描报告输出模板

AI 在所有 6 个维度扫完后，必须输出完整报告：

```markdown
# 代码扫描报告

**项目**：{项目名}
**扫描时间**：{时间}
**扫描范围**：src/main/ 下所有文件
**总 Java 类数**：{N} 个

---

## 1. 模块结构

（模块清单表格）

## 2. 类后缀分布

（类后缀频次表）

## 3. 命名模式

（命名样本 + 推断规则）

## 4. 注解模式

（Controller / Service / 其他注解分布）

## 5. 技术栈

（依赖清单）

## 6. 配置文件

（配置风格 + 关键配置）

---

## 初步观察

- **一致项**：{列出 100% 一致的}
- **不一致项**：{列出 50%–99% 的}
- **缺失项**：{列出 < 50% 的}

将进入步骤 03 推断模式 + 步骤 04 质问。
```

## 注意事项

1. **只看事实，不做判断**——不要说"@Valid 用得不好"
2. **统计数据要精确**——百分比保留 1 位小数
3. **样本要充分**——至少抽样 5 个，不要只举 1 个例子
4. **跳过 target/、node_modules/**——这些是构建产物
5. **找不到的命令要用兜底**——比如 `find` 在 Windows Git Bash 下可能略有差异
