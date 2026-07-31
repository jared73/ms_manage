---
name: yto-project-map-init
description: 圆通订单中心项目地图初始化与增量更新。当用户说"初始化项目地图"、"更新项目地图"、"扫描项目模块"时触发。
---

# 项目地图初始化

扫描指定产品线的全量模块目录和文件结构，提取详细信息生成项目地图文件。

## 强制规范
- 遵循 always-on.md 全局规则
- 禁止读取本步骤指定规范文档之外的任何文件
- 每执行完一条执行步骤，必须明确输出当前步骤执行结果，禁止静默执行
- 流程结束后，必须严格按照输出格式固定模板输出

### 任务拆解强制规则

1. 每个步骤必须作为独立待办任务，不得合并
2. 每个步骤必须独立完成后才能标记为完成

## 所需参数
| 字段 | 类型 | 必填 | 说明 | 示例 |
| productLine | String | 是 | 指定产品线，必填 | `lms` |

## 扫描范围与内容

| 层次 | 目录 | 扫描内容 |
|------|------|---------|
| 微服务 | ms/ | Controller类名、@RequestMapping路径、调用链路 |
| 客户端 | client/ | 模块名、Feign接口名、目标微服务名、目标URL、中文描述 |
| 插件 | plugin/ | 所属模块、插件类名、插件类型(create/update/notify/cancel)、中文描述 |
| 生产者 | producer/ | 模块名、地址、说明 |
| 数据库 | service/ | 模块名、数据库类型、数据库名、链路 |
| 公共 | common/ | 模块artifactId |
| DTO | dto/ | 模块artifactId |
| 枚举 | 各模块下 | 枚举类名、枚举值列表、中文描述 |
| 常量类 | 各模块下 | 常量类名、中文描述 |

**产品线归类规则：**
- `lms` → 模块名前缀为 `lms-` 或 `stl-` 或 `plat-`
- `oc` → 模块名前缀为 `oc-`
- `cmd` → 模块名前缀为 `cmd-`

## 执行流程

### 步骤1：扫描 ms 模块 - 获取 Controller 和链路
- 扫描 `ms/{productLine}-*/src/main/java` 下所有 `*Controller.java`
- 提取类名、@RequestMapping 路径
- 分析方法上的 @GetMapping/@PostMapping/@PutMapping/@DeleteMapping
- 分析代码中的 Service 注入，获取调用链路 Controller → Service → Producer/Client
- 输出：Controller 清单及链路

### 步骤2：扫描 Client 模块 - 获取 Feign 详情
- 扫描 `client/{productLine}-*/src/main/java` 下包含 `@FeignClient` 的接口
- 提取：接口类名、name/value 属性（目标微服务名）、url 属性
- 从类注释或包名推断中文描述
- 输出：Client 模块 Feign 详情列表

### 步骤3：扫描 Plugin 模块 - 获取插件详情
- 扫描 `plugin/{productLine}-*/src/main/java` 下所有 `*Plugin.java`（不含 $ 内部类）
- 提取：插件类名、插件类型（create/update/notify/cancel）
- 从类注释推断中文描述
- 输出：Plugin 详情列表

### 步骤4：扫描 Producer 模块 - 获取地址信息
- 扫描 `producer/{productLine}-*/src/main/java` 下配置了 @Configuration 的类
- 提取：模块名、配置的消息队列地址或 HTTP 地址
- 从类注释推断说明
- 输出：Producer 模块详情

### 步骤5：扫描 Service 模块 - 获取数据库信息
- 扫描 `service/{productLine}-*/src/main/java` 下配置了 @Mapper 或extends JpaRepository 的类
- 扫描 application.yml/application.properties 获取数据库配置
- 提取：模块名、数据库类型、数据库名
- 输出：Service 模块数据库详情

### 步骤6：扫描 common 和 dto 模块
- 扫描 `common/{productLine}-*/pom.xml` 获取 artifactId
- 扫描 `dto/{productLine}-*/pom.xml` 获取 artifactId
- 输出：模块列表

### 步骤7：扫描枚举和常量类
- 在各模块 `src/main/java` 下搜索 `*Enum.java`、`*Constants.java`、`*ApplicationNames.java`
- 读取枚举值列表（所有 public static final 或 Enum 类型的值）
- 读取类注释推断中文描述
- 输出：枚举和常量类详情

### 步骤8：生成项目地图
- 按以下格式生成 markdown 内容
- 写入 `.spec/{productLine}-business-map.md`

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式

```
## {产品线}产品线 - 项目模块地图

### 微服务（ms）

| 模块名 | Controller | Mapping | 调用链路 |
|--------|-----------|---------|---------|
| {artifactId} | {类名} | {路径} | {Service} → {Producer/Client} |

### 客户端（client）

| 模块名 | 中文描述 | Feign接口 | 目标微服务 | 目标URL |
|--------|---------|-----------|-----------|---------|
| {artifactId} | {描述} | {接口名} | {服务名} | {url} |

### 插件（plugin）

| 所属模块 | 插件名称 | 类型 | 中文描述 |
|---------|---------|------|---------|
| {模块名} | {类名} | {类型} | {描述} |

### 生产者（producer）

| 模块名 | 地址 | 说明 |
|--------|------|------|
| {artifactId} | {地址} | {说明} |

### 数据库（service）

| 模块名 | 数据库类型 | 数据库名 | 链路 |
|--------|----------|---------|------|
| {artifactId} | {类型} | {库名} | {调用关系} |

### 公共（common）

| 模块名 |
|--------|
| {artifactId} |

### DTO（dto）

| 模块名 |
|--------|
| {artifactId} |

### 枚举 & 常量

| 所属模块 | 类型 | 类名 | 枚举值/常量 | 中文描述 |
|---------|------|------|-----------|---------|
| {模块} | 枚举 | {类名} | {值1},{值2}... | {描述} |
| {模块} | 常量 | {类名} | - | {描述} |
```
