---
name: pom-xml
description: POM 文件修改硬约束
---

# POM 修改硬约束

> 适用于所有 `pom.xml` 修改。违反会导致依赖冲突、版本不一致、CI 失败。

## 1. 版本管理

### 强制规则
- ✅ 所有依赖版本在**父 POM** 的 `<dependencyManagement>` 统一管理
- ✅ 子 POM 只写 `<groupId>` + `<artifactId>`，**不写 `<version>`**
- ❌ 禁止子 POM 写死版本号

### 父 POM 示例
```xml
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>cn.com.{{COMPANY}}</groupId>
    <artifactId>{{PRODUCT_LINE}}-parent</artifactId>
    <version>1.0.0-SNAPSHOT</version>
    <packaging>pom</packaging>

    <properties>
        <java.version>17</java.version>
        <spring-boot.version>3.2.0</spring-boot.version>
        <mybatis-plus.version>3.5.5</mybatis-plus.version>
        <lombok.version>1.18.30</lombok.version>
    </properties>

    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-starter-web</artifactId>
                <version>${spring-boot.version}</version>
            </dependency>
            <dependency>
                <groupId>com.baomidou</groupId>
                <artifactId>mybatis-plus-spring-boot3-starter</artifactId>
                <version>${mybatis-plus.version}</version>
            </dependency>
            <!-- 更多依赖 -->
        </dependencies>
    </dependencyManagement>
</project>
```

### 子 POM 示例
```xml
<project>
    <parent>
        <groupId>cn.com.{{COMPANY}}</groupId>
        <artifactId>{{PRODUCT_LINE}}-parent</artifactId>
        <version>1.0.0-SNAPSHOT</version>
    </parent>
    <artifactId>{{PRODUCT_LINE}}-order-api</artifactId>

    <dependencies>
        <!-- 不写 version，继承父 POM -->
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
        <dependency>
            <groupId>com.baomidou</groupId>
            <artifactId>mybatis-plus-spring-boot3-starter</artifactId>
        </dependency>
    </dependencies>
</project>
```

## 2. 模块结构

### 命名规则
- 格式：`{{MODULE_STYLE}}`
- 详细规则见 `always-on.md` 第 1 节

### 父 POM 注册子模块
```xml
<modules>
    <module>{{PRODUCT_LINE}}-order-api</module>
    <module>{{PRODUCT_LINE}}-order-service</module>
    <module>{{PRODUCT_LINE}}-order-client</module>
    <module>{{PRODUCT_LINE}}-common</module>
</modules>
```

## 3. 依赖方向

### 强制规则（绝对不能违反）
- ✅ `api` → `service` → `client`（下游 → 上游）
- ✅ `service` → `common`
- ❌ 禁止反向依赖（如 `service` → `api`）
- ❌ 禁止循环依赖

### 依赖关系图
```
{{PRODUCT_LINE}}-order-api  ──→  {{PRODUCT_LINE}}-order-client  ──→  {{PRODUCT_LINE}}-order-service
                                          │
                                          ↓
                                {{PRODUCT_LINE}}-common
```

## 4. 常用依赖（已锁定版本）

| 依赖 | 版本 | 用途 |
|------|------|------|
| spring-boot-starter-parent | 3.2.0 | Spring Boot 父 POM |
| spring-cloud-dependencies | 2023.0.0 | Spring Cloud BOM |
| mybatis-plus-spring-boot3-starter | 3.5.5 | MyBatis Plus |
| mysql-connector-j | 8.0.33 | MySQL 驱动 |
| druid-spring-boot-3-starter | 1.2.20 | 数据库连接池 |
| lombok | 1.18.30 | 代码简化 |
| hutool-all | 5.8.25 | 工具库 |
| springdoc-openapi-starter-webmvc-ui | 2.3.0 | Swagger |
| fastjson2 | 2.0.43 | JSON |

## 5. 禁止项

| 禁止 | 原因 |
|------|------|
| ❌ 子 POM 写 `<version>` | 版本不一致风险 |
| ❌ 引入 SNAPSHOT 依赖 | 版本不稳定 |
| ❌ 重复声明父 POM 已管理的依赖 | 冗余 |
| ❌ 反向依赖 | 循环依赖风险 |
| ❌ 引入未审核的第三方库 | 安全风险 |
| ❌ 引入被官方废弃的库 | 长期维护风险 |
| ❌ 私自修改父 POM 版本号 | 团队不一致 |

## 6. 常见错误示例

### 错误 1：子 POM 写版本
```xml
<!-- ❌ 错误 -->
<dependency>
    <groupId>com.baomidou</groupId>
    <artifactId>mybatis-plus-spring-boot3-starter</artifactId>
    <version>3.5.5</version>  <!-- 不应该写 -->
</dependency>
```

### 错误 2：反向依赖
```xml
<!-- {{PRODUCT_LINE}}-service/pom.xml 中 -->
<!-- ❌ 错误：service 不应该依赖 api -->
<dependency>
    <groupId>cn.com.{{COMPANY}}</groupId>
    <artifactId>{{PRODUCT_LINE}}-order-api</artifactId>
</dependency>
```

### 错误 3：循环依赖
```xml
<!-- {{PRODUCT_LINE}}-A/pom.xml 中 -->
<dependency>
    <groupId>cn.com.{{COMPANY}}</groupId>
    <artifactId>{{PRODUCT_LINE}}-B</artifactId>
</dependency>

<!-- {{PRODUCT_LINE}}-B/pom.xml 中 -->
<dependency>
    <groupId>cn.com.{{COMPANY}}</groupId>
    <artifactId>{{PRODUCT_LINE}}-A</artifactId>  <!-- 循环！ -->
</dependency>
```

## 7. 修改 pom 流程

1. 先读 `.spec/{{PRODUCT_LINE}}-spec-pom-base.md` + `-pom-module.md`
2. 检查依赖是否在父 POM 已管理
3. 如果是新增依赖，先在父 POM `<dependencyManagement>` 加版本
4. 子 POM 只引用
5. 运行 `mvn dependency:tree` 检查依赖树
6. 运行 `mvn clean install` 确保编译通过
7. 提交 PR，CI 会自动跑 `rules-checker` + `mvn verify`

## 8. CI 检查

PR 提交时 CI 会检查：
- 子 POM 是否私自写版本号
- 依赖方向是否正确
- 是否引入未审核的第三方库
- `mvn dependency:tree` 是否健康

违反以上规则会导致 PR 红灯。
