---
name: yto-dev-execute-pipeline
description: 订单中心全链路开发流水线（完整版）。触发词："开发执行"、"文档执行"。
version: 1.0.0
author: YTO-OC-Team
---

# 订单中心开发文档执行

## 强制规范
- 遵循 always-on.md 全局规则
- 禁止读取除.spec目录外的任何文件（检查文件是否存在类操作不受此限制）
- 调用子 Skill 必须严格按照其执行步骤流程执行，不允许跳过任何步骤
- 所有规范索引在 `.spec/00-base.md`，执行阶段子 Skill 按需读取
- 所有代码/配置的生成与写入必须通过指定的子 Skill 完成，流水线自身不得创建、修改任何文件
- 严格串行，阻塞步骤等用户“确认”后继续

### 任务拆解强制规则（禁止合并阶段）

**在生成待办任务列表（Todo）时，必须严格遵循以下规则：**

1. **每个阶段必须作为独立的一条待办任务**，共 10 条，不得合并、不得省略
2. **每条待办必须包含完整的阶段名称和目标描述**，格式如下：
   - `阶段 1: 准备 — 解析 plan.md 提取模块清单、类清单、中间件配置等`
   - `阶段 2: 模块创建 — 调用 yto-module-{type} 创建新模块`
   - `阶段 3: 配置 Redis — 调用 yto-middleware-redis`
   - `阶段 4: 配置 Consumer — 调用 yto-middleware-consumer`
   - `阶段 5: 添加依赖 — 调用 yto-pom-dependency`
   - `阶段 6: 插件 Plugin — 调用 yto-dispatcher-plugin`
   - `阶段 7: Java 类 — 调用 yto-class-builder`
   - `阶段 8: 检查代码 — 校验代码规范并自动修正`
   - `阶段 9: 测试 — 调用 yto-test-builder`
   - `阶段 10: 验证 — 调用 yto-test-runner，失败调用 yto-code-fixer`
3. **严禁出现以下合并写法**：
   - ❌ `阶段 2-6: 模块/中间件/依赖/插件`
   - ❌ `后续阶段（阶段 7-10）`
4. **每个阶段必须独立完成后才能标记为完成**，前一个阶段未完成不得开始后一个阶段

---

## 所需参数

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| planFilePath | String | 是 | 已确认的 plan.md 文件路径 |

---

## 执行流程

### 阶段 1：准备
1. 确定当前产品线 `productLine`（从 plan.md 的业务类清单中提取第一个类的 productLine）
2. 读取 `{planFilePath}` 文件，输出摘要
3. **解析 plan.md 结构**，提取以下章节：
   - 新建模块清单（对应 module_list）
   - 复用模块清单
   - 业务类清单（对应 class_list）
   - 中间件配置（redis_list、consumer_list）
   - 依赖关系（dependency_mapping）
   - Plugin调度策略（plugin_dispatcher_list）
4. 输出解析结果摘要

### 阶段 2：模块创建，调用对应的 Skill：`yto-module-{type}` 执行
1. 从「新建模块清单」中读取每个模块的 type 字段
3. 调用时传入以下字段：
   | 字段 | 来源 |
   |------|------|
   | type | 新建模块清单的 type 列 |
   | moduleDir | 新建模块清单的 moduleDir 列 |
   | productLine | 新建模块清单的 productLine 列 |
   | functionModule | 新建模块清单的 functionModule 列 |
   | businessName | 新建模块清单的 businessName 列 |
   | appType | 新建模块清单的 appType 列 |
   | appName | 新建模块清单的 appName 列 |

### 阶段 3：配置REDIS,调用 Skill：`yto-middleware-redis`
- 从「中间件配置」的 redis_list 中读取每个配置项

### 阶段 4：配置CONSUMER,调用 Skill：`yto-middleware-consumer`
- 从「中间件配置」的 consumer_list 中读取每个配置项

### 阶段 5：添加依赖,调用 Skill：`yto-pom-dependency`
- 从「依赖关系」中读取每个依赖项
   | 字段 | 来源 |
   |------|------|
   | belongApp | dependency_mapping 的 belongApp 列 |
   | dependencies | dependency_mapping 的 dependencies 列（逗号分隔） |

### 阶段 6：插件-PLUGIN,调用 Skill：`yto-dispatcher-plugin`
- 从「Plugin调度策略」中读取每个配置项:
   | 字段 | 来源 |
   |------|------|
   | pluginModule | 从 configFileName 中提取 |
   | engineName | dispatchers 的 engineName 列 |
   | configFileName | dispatchers 的 configFileName 列 |
   | dispatchers | dispatchers 的完整配置 |

### 阶段 7：Java类-CLASS，调用 Skill：`yto-class-builder`
- 每个类的生成必须基于 skill 内置的 `.spec/oc-spec-class-*.md` 规范模板
- businessLogic 必须包含完整执行步骤，否则提示用户 plan.md 不完整
- 从「业务类清单」中读取每个类配置项:
   | 字段 | 来源 |
   |------|------|
   | className | 业务类清单的 className 列 |
   | productLine | 业务类清单的 productLine 列 |
   | targetModule | 业务类清单的 targetModule 列 |
   | targetPackage | 业务类清单的 targetPackage 列 |
   | classType | 业务类清单的 classType 列 |
   | apiEndpoint | 业务类清单的 apiEndpoint 列（仅Controller有值） |
   | businessLogic | 业务类清单的 businessLogic 列 |

### 阶段 8：检查代码
- 检查代码是否符合规范：`.spec/oc-spec-class-*.md` 规范模板
- 检查每一个类的import是否正确，是否缺失或重复
- 检查完成后，自动修正所有错误

### 阶段 9：测试-TEST，调用 Skill：`yto-test-builder`
- 仅测试新类
- 测试代码必须基于 skill 内置的 `.spec/oc-spec-test-*.md` 规范模板   

### 阶段 10：验证-VERIFY，调用 Skill：`yto-test-runner`
- 运行测试，失败调用 Skill：`yto-code-fixer` 修復，最多 3 次

## 检查清单

详见 [checklist.md](checklist.md)

## plan.md 解析规范

execute pipeline 必须从 plan.md 的以下章节提取数据：

### 新建模块清单（对应 module_list）
| 列名 | 说明 |
|------|------|
| type | 模块类型（ms/client/plugin/service/producer/common） |
| moduleDir | 模块目录（ms/client/plugin等） |
| productLine | 产品线（lms/oc等） |
| functionModule | 功能模块（order/access等） |
| businessName | 业务名称（create/terminal等） |
| appType | 应用类型（ms/consumer/job） |
| appName | 应用名（productLine-functionModule-businessName-appType） |
| description | 模块描述 |

### 业务类清单（对应 class_list）
| 列名 | 说明 |
|------|------|
| className | 类名称 |
| productLine | 产品线 |
| targetModule | 目标模块名 |
| targetPackage | 完整包路径 |
| classType | 类类型（Controller/Service/ServiceImpl/Plugin/Feign/DTO/Req/Resp/Enum） |
| apiEndpoint | Controller的API端点（仅Controller填） |
| businessLogic | 详细业务逻辑（必须包含完整执行步骤） |

### 中间件配置（redis_list + consumer_list）

Redis配置：
| 列名 | 说明 |
|------|------|
| redisName | Redis实例名 |
| belongApp | 所属应用 |
| uat_appId | UAT环境appId |
| prod_appId | 生产环境appId |

MQ Consumer配置：
| 列名 | 说明 |
|------|------|
| consumerName | 消费者名称 |
| belongApp | 所属应用 |
| messageType | 消息类型 |
| uat_nameServer | UAT NameServer |
| prod_nameServer | 生产NameServer |
| consumerGroup | 消费者组 |
| topic | Topic |

### 依赖关系（对应 dependency_mapping）
| 列名 | 说明 |
|------|------|
| belongApp | 所属应用 |
| dependencies | 依赖列表（逗号分隔） |

### Plugin调度策略（对应 plugin_dispatcher_list）
| 列名 | 说明 |
|------|------|
| engineName | 引擎名称 |
| configFileName | 配置文件名 |
| dispatchers | YAML格式的调度配置 |

## 输出格式
阶段完成状态表 + 产物路径清单
