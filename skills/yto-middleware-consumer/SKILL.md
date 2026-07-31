---
name: yto-middleware-consumer
description: 圆通订单中心MQ Consumer消费者依赖添加，支持POM依赖引入、配置文件生成和Consumer类创建。当用户说"添加Consumer"、"配置Consumer"、"引入Consumer"时触发，也可被其他Skill的执行步骤触发
---

# Consumer 中间件配置器
- 引导用户完成一次 MQ Consumer 的配置流程，包括 POM 依赖引入、配置文件生成和 Consumer 类创建。

## 强制规范
- 遵循 always-on.md 全局规则
- 每执行完一条执行步骤，必须明确输出当前步骤执行结果，禁止静默执行
- 流程结束后，必须严格按照输出格式固定模板输出

### 任务拆解强制规则

1. 每个步骤必须作为独立待办任务，不得合并
2. 每个步骤必须独立完成后才能标记为完成

## 所需参数

| 字段 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| appName | String | 是 | 主应用工程名称 | `lms-order-create-ms` |
| consumerGroup | String | 是 | Consumer 消费组名称 | `lms_order_create_consumer_group` |
| topic | String | 是 | 消费 Topic 名称 | `ORDER_CREATE_TOPIC` |
| tags | String | 是 | 消费 Tag，多个用 \|\| 分隔 | `order_create\|\|order_cancel` |
| uatNameServer | String | 是 | UAT 环境 NameServer 地址 | `10.130.36.80:9876` |
| prodNameServer | String | 是 | PROD 环境 NameServer 地址 | `mq2-ns1-ord.prd.mid:9876;mq2-ns2-ord.prd.mid:9876` |
| consumerClassName | String | 是 | Consumer 类名称 | `OrderCreateConsumer` |
| targetPackage | String | 是 | Consumer 类所在包路径 | `com.yto.hermes.lms.order.create.consumer` |

## 输入向导

与步骤 2 合并执行：
- 被其他 Skill 触发：跳过交互，参数缺失直接终止
- 用户直接触发：参数不全则引导补齐

## 执行流程

### 步骤1：提取参数
- 从用户输入或上下文自动提取「所需参数」表中定义的参数
- 输出：参数表格

### 步骤2：区分触发模式
- 被其他Skill的执行步骤触发：跳过输入向导，不交互式提问，参数任意缺失直接终止并提示缺失字段
- 用户直接触发：参数不全则进入 输入向导收集并确认，补齐后再往下执行

### 步骤3：读取 POM 依赖规范，必须遵守文档：`.spec/oc-spec-pom-module.md`
- 输出：✅ 步骤3完成，已读取 POM 依赖规范

### 步骤4：主应用POM引入Consumer依赖-已引入则跳过，必须遵守文档：`.spec/oc-spec-middleware-consumer.md`
- 输出：✅ 步骤4完成，主应用 POM Consumer 依赖 {已引入/已引入跳过}

### 步骤5：添加Consumer 配置（uat），必须遵守文档：`.spec/oc-spec-middleware-consumer.md`
- 添加 uat 环境 Consumer 配置（若已存在则跳过）
- 输出：✅ 步骤5完成，uat Consumer 配置 {已添加/已存在跳过}

### 步骤6：添加Consumer 配置（prod），必须遵守文档：`.spec/oc-spec-middleware-consumer.md`
- 添加 prod 环境 Consumer 配置（若已存在则跳过）
- 输出：✅ 步骤6完成，prod Consumer 配置 {已添加/已存在跳过}

### 步骤7：创建 Consumer 类-存在则跳过，必须遵守文档：`.spec/oc-spec-middleware-consumer.md`
- 输出：✅ 步骤7完成，Consumer 类 {已创建/已存在跳过}

### 步骤8：执行检查清单
- 详见 [checklist.md](checklist.md)，如果有任何项未通过，提示用户需要修复，修复后继续执行检查，最多检查 3 次
- 输出：检查清单结果

### 步骤9：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、应用信息收集结果
| 字段 | 值 |
|------|-----|
| appName | {appName} |
| consumerGroup | {consumerGroup} |
| topic | {topic} |
| tags | {tags} |
| consumerClassName | {consumerClassName} |
| targetPackage | {targetPackage} |

### 二、Consumer 配置完成

| 变更位置 | 变更内容 |
|---------|---------|
| 主应用 `pom.xml` \<dependencies\> | 新增 Consumer 相关依赖引入 |
| `{appName}/src/main/resources/bootstrap-consumer-uat.yml` | 新增 UAT 环境 Consumer 配置 |
| `{appName}/src/main/resources/bootstrap-consumer-prod.yml` | 新增 PROD 环境 Consumer 配置 |
| `{targetPackage}/{consumerClassName}.java` | 新增 Consumer 类 |
