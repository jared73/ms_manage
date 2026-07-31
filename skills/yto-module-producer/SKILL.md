---
name: yto-module-producer
description: 圆通订单中心Producer生产者模块创建，支持自动创建模块、POM注册、配置文件生成。当用户说"创建Producer"、"添加Producer"、"新建Producer模块"时触发，也可被其他Skill的执行步骤触发
---

# Producer 模块构建器
引导用户完成一次 Producer 模块的创建流程，包括模块创建、根POM注册、主应用POM引入。

## 强制规范
- 遵循 always-on.md 全局规则
- 每执行完一条执行步骤，必须明确输出当前步骤执行结果，禁止静默执行
- 流程结束后，必须严格按照输出格式固定模板输出

### 任务拆解强制规则

1. 每个步骤必须作为独立待办任务，不得合并
2. 每个步骤必须独立完成后才能标记为完成

## 所需参数

| 字段             | 类型 | 必填 | 说明 | 示例 |
|----------------|------|------|------|------|
| moduleDir      | String | 是 | 模块所属目录名称，固定为 `producer` | `producer` |
| productLine    | String | 是 | 业务产品线标识 | `lms` |
| functionModule | String | 是 | 业务功能模块标识 | `order` |
| businessName   | String | 是 | 业务功能模块下的业务名称标识 | `create` |
| uatNameServer  | String | 是 | UAT 环境 NameServer 地址 | `10.130.36.80:9876` |
| prodNameServer | String | 是 | PROD 环境 NameServer 地址 | `mq2-ns1-ord.prd.mid:9876;mq2-ns2-ord.prd.mid:9876` |
| appName        | String | 否 | 主应用工程名称，用于引入 Producer 模块依赖。为空则跳过主应用 POM 引入（由工作流 step5 统一引入） | `lms-order-create-ms` |

## 输入向导
### 第一轮：基础信息收集
- 检查用户是否输入了以下信息：
  - moduleDir
  - productLine
  - functionModule
  - businessName
  - uatNameServer
  - prodNameServer
  - appName（可选）
- 如果用户未输入完整信息，提示用户输入缺失的信息。
- 如果用户输入了完整信息，提示用户确认信息。
- 如果用户确认信息，继续下一轮。

## 执行流程

### 步骤1：提取参数
- 从用户输入或上下文自动提取「所需参数」表中定义的参数
- 输出：参数表格

### 步骤2：区分触发模式
- 被其他Skill的执行步骤触发：跳过输入向导，不交互式提问，参数任意缺失直接终止并提示缺失字段
- 用户直接触发：参数不全则进入输入向导收集并确认，补齐后再往下执行

### 步骤3：创建 moduleDir 文件夹，已存在则跳过，必须遵守规范：`.spec/oc-spec-content.md`
- 输出：✅ 步骤3完成，已创建 {moduleDir} 文件夹

### 步骤4：创建 Producer 模块，已存在则跳过，必须遵守规范：`.spec/oc-spec-module-producer.md`
- 输出：✅ 步骤4完成，Producer 模块 {已创建/已存在跳过}

### 步骤5：创建 AutoConfiguration 类，已存在则跳过，必须遵守规范：`.spec/oc-spec-module-producer.md`
- 输出：✅ 步骤5完成，AutoConfiguration 类 {已创建/已存在跳过}

### 步骤6：注册模块到根 POM，已注册则跳过，必须遵守规范：`.spec/oc-spec-pom-base.md`
- 输出：✅ 步骤6完成，根 POM {已注册/已注册跳过}

### 步骤7：在主应用 POM 引入依赖，若 appName 为空则跳过，必须遵守规范：`.spec/oc-spec-pom-module.md`
- 输出：✅ 步骤7完成，主应用 POM {已引入/已引入跳过}

### 步骤8：执行检查清单
- 详见 [checklist.md](checklist.md)，如果有任何项未通过，提示用户需要修复，修复后继续执行检查，最多检查 3 次
- 输出：检查清单结果

### 步骤9：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、应用信息收集结果
| 字段              | 值 |
|-----------------|-----|
| moduleDir       | {moduleDir} |
| productLine     | {productLine} |
| functionModule  | {functionModule} |
| businessName    | {businessName} |
| uatNameServer   | {uatNameServer} |
| prodNameServer  | {prodNameServer} |
| appName         | {appName 或 "未指定"} |

### 二、Producer 模块创建完成
模块路径：`producer/{productLine}-{functionModule}-{businessName}-producer/`

### 三、POM 注册结果
| 注册位置 | 变更内容 |
|---------|---------|
| 根 `pom.xml` \<modules\> | 新增 `<module>producer/{productLine}-{functionModule}-{businessName}-producer</module>` |
| 根 `pom.xml` \<dependencyManagement\> | 新增 `{productLine}-{functionModule}-{businessName}-producer` 依赖声明 |
| 主应用 `pom.xml` \<dependencies\> | 若 appName 不为空，新增 `{productLine}-{functionModule}-{businessName}-producer` 依赖引入 |
