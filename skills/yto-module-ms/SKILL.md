---
name: yto-module-ms
description: 圆通订单中心ms微服务模块创建，只负责创建ms主应用模块。当用户说"创建订单ms模块"、"添加订单ms模块"时触发，也可被工作流步骤触发
---

# MS 微服务模块构建器
引导用户完成一次 ms 微服务模块的创建流程，包括模块创建和根POM注册。

## 强制规范
- 遵循 always-on.md 全局规则
- 每执行完一条执行步骤，必须明确输出当前步骤执行结果，禁止静默执行
- 流程结束后，必须严格按照输出格式固定模板输出

### 任务拆解强制规则

1. 每个步骤必须作为独立待办任务，不得合并
2. 每个步骤必须独立完成后才能标记为完成

## 所需参数

| 字段                | 类型 | 必填 | 说明                                         | 示例 |
|-------------------|------|----|--------------------------------------------|------|
| moduleDir         | String | 是  | 模块目录名称 | `ms` |
| productLine       | String | 是  | 业务产品线标识   | `lms` |
| functionModule    | String | 是  | 功能模块名称   | `order` |
| businessName      | String | 是  | 业务名称 | `create` |
| appType           | String | 是  | 应用类别，枚举：api/ms/consumer/job | `ms` |

## 输入向导
### 第一轮：基础信息收集
- 检查用户是否输入了以下信息：
  - moduleDir
  - productLine
  - functionModule
  - businessName
  - appType
- 如果用户未输入完整信息，提示用户输入缺失的信息。
- 如果用户输入了完整信息，提示用户确认信息。
- 如果用户确认信息，继续下一步。

## 执行流程

### 步骤1：提取参数
- 从用户输入或上下文自动提取参数：moduleDir、productLine、functionModule、businessName、appType
- 输出格式包含以下字段：
  | 字段 | 值 |
  |------|-----|
  | moduleDir | {moduleDir} |
  | productLine | {productLine} |
  | functionModule | {functionModule} |
  | businessName | {businessName} |
  | appType | {appType} |

### 步骤2：区分触发模式
- 被工作流步骤触发：跳过输入向导，不交互式提问，参数任意缺失直接终止并提示缺失字段
- 用户直接触发：参数不全则进入 输入向导收集并确认，补齐后再往下执行

### 步骤3：读取应用所属模块规范，必须遵守文档：`.spec/oc-spec-content.md`
- 输出：✅ 步骤3完成，已读取应用所属模块规范

### 步骤4：创建 moduleDir 文件夹，已存在跳过
- 提示用户 moduleDir 不存在，创建模块文件夹
- 输出：✅ 步骤4完成，已创建 {moduleDir} 文件夹

### 步骤5：创建 ms 模块，已存在跳过，必须遵守文档：`.spec/oc-spec-module-ms.md`
- 根据 appName 创建模块
- 输出：✅ 步骤5完成，ms 模块 {appName} {已创建/已存在跳过}

### 步骤6：注册模块到根 POM，已注册跳过，必须遵守文档：`.spec/oc-spec-pom-base.md`
- 输出：✅ 步骤6完成，根 POM {已注册/已注册跳过}

### 步骤7：执行检查清单
- 详见 [checklist.md](checklist.md)，如果有任何项未通过，提示用户需要修复，修复后继续执行检查，最多检查 3 次
- 输出：检查清单结果

### 步骤8：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、应用信息收集结果
| 字段 | 类型 | 说明 | 示例 |
|------|------|------|------|
| appName | String | 由输入参数拼接：{productLine}-{functionModule}-{businessName}-{appType} | `lms-order-create-ms` |
| productLine | String | 透传输入参数 | `lms` |
| functionModule | String | 透传输入参数 | `order` |
| businessName | String | 透传输入参数 | `create` |
| appType | String | 透传输入参数 | `ms` |
| packagePath | String | cn.com.yto56.{productLine}.{functionModule}.{businessName} | `cn.com.yto56.lms.order.create` |
| modulePath | String | {moduleDir}/{appName} | `ms/lms-order-create-ms` |

### 二、ms 模块创建完成
模块路径：`ms/{productLine}-{functionModule}-{businessName}-{appType}/`

### 三、模块检查结果
- 所属模块 `{moduleDir}` 是否存在：是 / 否
- 已自动创建模块文件夹：{moduleDir}（如需要）