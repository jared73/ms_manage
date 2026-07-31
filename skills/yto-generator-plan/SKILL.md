---
name: yto-generator-plan
description: 基于已确认的 spec.md 和项目业务地图，生成技术实现方案 plan.md。
  触发词包括"生成方案"、"技术方案"、"生成设计"，也可被其他 Skill 的 执行步骤触发。
---

# 技术方案生成器

## 强制规范
- 遵循 always-on.md 全局规则
- 禁止读取本步骤指定规范文档之外的任何文件
- **【强制】按产品线拆分输出**，每个产品线生成独立的 plan-{productLine}.md 文件
- **【强制】businessLogic 禁止使用简略描述，必须包含完整执行步骤**
- **【强制】所有清单必须使用表格格式**

### 任务拆解强制规则

**在生成待办任务列表（Todo）时，必须严格遵循以下规则：**

1. **每个步骤必须作为独立的一条待办任务**，共 11 条，不得合并、不得省略
2. **每条待办必须包含完整的步骤名称**：
   - `步骤 1: 读取 spec.md 和原始需求`
   - `步骤 2: 匹配业务规范`
   - `步骤 3: 读取产品线地图`
   - `步骤 4: 加载项目地图`
   - `步骤 5: 模块决策 — 新建/复用/不需要`
   - `步骤 6: 设计类清单 — 含完整 businessLogic`
   - `步骤 7: 设计 Plugin 调度策略`
   - `步骤 8: 确定依赖关系和中间件需求`
   - `步骤 9: 按产品线拆分生成 plan-{productLine}.md`
   - `步骤 10: 生成汇总索引文件（可选）`
   - `步骤 11: 阻塞等待用户确认`
3. **每个步骤必须独立完成后才能标记为完成**

---

## 所需参数

| 字段                 | 类型     | 必填 | 说明                                 |
| ------------------ | ------ | -- | ---------------------------------- |
| specFilePath       | String | 是  | 已确认的 spec.md 文件路径                  |
| originalDemandPath | String | 否  | 原始需求文档路径，若提供则直接从中提取接口定义、系统交互图等技术信息 |

## 执行流程

### 步骤1：读取spec.md和原始需求
- 输出：✅ 步骤1完成，已读取 spec.md和需求文档。

### 步骤2：匹配业务规范：`.spec/oc-business-*.md`
- 严格遵循业务规范。
- 若未匹配到：跳过本步骤。
- 输出：✅ 步骤2完成，已读取业务规范：{匹配到的文件名列表}（或跳过）。

### 步骤3：读取产品线地图：`.spec/oc-project-business-map.md`
- 严格遵循项目业务地图规范。
- 输出：✅ 步骤3完成，已读取业务地图。

### 步骤4：加载项目地图：`.doc/{产品线}-business-map.md`
- 严格遵守项目业务地图规范。
- 输出：✅ 步骤4完成，已加载项目地图。

### 步骤5：模块决策，严格遵守规范，输出前必须检查模块职责
- 规范文件：
  - `.spec/oc-spec-application-responsibility.md`
  - `.spec/oc-project-business-map.md`
  - `.doc/{产品线}-business-map.md`
- 模块决策：新建/复用/不需要，给出理由
- 决策范围：ms、plugin、client、service、producer、common、dto
- 模块验证：标注「复用」的模块是否真实存在，标注「新建」的模块是否确实不存在
- 输出：模块决策结果

### 步骤6：设计类清单，严格遵守规范，必须包含完整字段 
- 严格遵循类命名规范：`.spec/oc-spec-name.md`
- 严格遵循类编写规范：`.spec/oc-spec-class-*.md`
#### 类清单必须包含的字段
| 字段 | 说明 | 示例 |
|------|------|------|
| className | 类名称 | ZhuanZhuanVerifyPlugin |
| productLine | 产品线 | oc |
| targetModule | 目标模块名 | oc-opt-terminal-plugin |
| targetPackage | 完整包路径 | cn.com.yto56.oc.plugin.plugins |
| classType | 类类型 | Plugin |
| apiEndpoint | Controller的API端点（仅Controller填） | @PostMapping("/zhuanzhuan/order/verify") |
| **businessLogic** | **详细业务逻辑（见下方规范）** | **见下方模板** |
#### businessLogic 编写规范【强制】
- 每个类的 businessLogic 必须包含以下维度：
  | 类类型 | 必须包含的内容 |
  |--------|--------------|
  | **Controller** | 1. @PostMapping路径<br>2. 接收哪个Req对象<br>3. 调用哪个Service方法<br>4. 返回什么Resp |
  | **Service接口** | 1. 接口职责<br>2. 方法签名（方法名、入参、返回值） |
  | **ServiceImpl** | 1. 调用哪些依赖（Plugin/Client/Service）<br>2. 执行顺序<br>3. 数据转换逻辑 |
  | **Plugin** | 1. 从context获取什么对象<br>2. 调用哪个Feign/Client方法<br>3. 异常处理逻辑<br>4. 结果放入context的key |
  | **Feign** | 1. 调用目标URL<br>2. @PostMapping路径<br>3. 入参类型<br>4. 出参类型 | 
  | **DTO/Req/Resp** | 1. 继承哪个基类<br>2. 所有字段列表（字段名、类型、是否必填、说明） |

#### businessLogic 模板示例

**Plugin类示例**：
```markdown
businessLogic: |
  1. 从请求获取参数：从 context.getRequest() 获取 ZhuanZhuanOrderVerifyReq 请求对象
  2. 调用下游服务：调用 zhuanZhuanOrderVerifyFeign.verify(req) 发送核销请求
  3. 解析返回结果：获取返回的 ZhuanZhuanBaseResp，校验 code 字段
  4. 异常处理：若 code≠0，抛出 BusinessException(code, message)
  5. 成功处理：调用 context.put(ZhuanZhuanBaseResp.class, resp) 放入上下文
```

**Controller类示例**：
```markdown
businessLogic: |
  1. 接收请求：@PostMapping("/zhuanzhuan/order/verify") 接收 ZhuanZhuanOrderVerifyReq
  2. 参数校验：参数校验逻辑由 @Valid 注解触发
  3. 调用服务：调用 zhuanZhuanStdTerminalOptService.verifyOrder(request)
  4. 返回响应：返回 UnifiedOptResp，其中 data 包含操作结果
```

**Feign类示例**：
```markdown
businessLogic: |
  1. 调用目标：POST https://api.zhuanzhuan.com/zhuanzhuan/order/verify
  2. 入参类型：ZhuanZhuanOrderVerifyReq（包含 ytoOrderNo、orderNo、verifyCode、verifyTime 等）
  3. 出参类型：ZhuanZhuanBaseResp（包含 code、message）
  4. 异常处理：若响应超时或网络异常，抛出 FeignException
```

**DTO类示例**：
```markdown
businessLogic: |
  1. 继承基类：extends BaseRequest
  2. 字段列表：
     - ytoOrderNo: String, 必填, 圆通订单号
     - orderNo: String, 必填, 转转订单号
     - verifyCode: String, 必填, 核销码
     - verifyTime: String, 必填, 核销时间(yyyy-MM-dd HH:mm:ss)
```

- 输出：类清单（每个类必须包含上述所有字段）

### 步骤7：设计 Plugin 调度策略（若涉及）
- 基于责任链模式来理解和设计Plugin调度策略
- 严格遵循规范：`.spec/oc-spec-plugin-dispatcher.md`
- 严格遵循该规范中的四阶段结构、条件分支、配置语法等约束设计调度策略

#### Plugin调度配置必须包含的字段

| 字段 | 说明 | 示例 |
|------|------|------|
| engineName | 引擎名称 | zzes-std-access |
| configFileName | 配置文件名 | bootstrap-plugin-zzes-std.yml |
| dispatchers[].key | 调度key | ZZES_STD_01 |
| dispatchers[].description | 描述 | 转转二手下单 |
| dispatchers[].subItems[].description | 阶段描述 | 参数初始化 |
| dispatchers[].subItems[].beanName | Plugin列表 | BaseInitParamPlugin |

- 输出：调度策略设计（或跳过）

### 步骤8：确定依赖关系和中间件需求

- 确定 dependency_mapping
- 确定 redis_list、consumer_list

#### 中间件配置必须包含的字段

**Redis配置**：
| 字段 | 说明 | 示例 |
|------|------|------|
| redisName | Redis实例名 | default |
| belongApp | 所属应用 | oc-opt-terminal-ms |
| uat_appId | UAT环境appId | 10053 |
| prod_appId | 生产环境appId | 20053 |

**MQ Consumer配置**：
| 字段 | 说明 | 示例 |
|------|------|------|
| consumerName | 消费者名称 | OrderCreateConsumer |
| belongApp | 所属应用 | lms-order-create-ms |
| messageType | 消息类型 | OrderCreateMo |
| uat_nameServer | UAT NameServer | 10.130.36.80:9876 |
| prod_nameServer | 生产NameServer | mq2-ns1-ord.prd.mid:9876 |
| consumerGroup | 消费者组 | lms-order-create-consumer-group |
| topic | Topic | lms-order-create-topic |

**依赖关系配置**：
| 字段 | 说明 | 示例 |
|------|------|------|
| belongApp | 所属应用 | oc-opt-terminal-ms |
| dependencies | 依赖列表 | [oc-opt-terminal-plugin, zz-es-client] |

- 输出：依赖关系和中间件需求

### 步骤9：按产品线拆分生成 plan-{productLine}.md【关键步骤】

#### 9.1 确定涉及的产品线

根据步骤5「模块决策」的结果，自动识别涉及的产品线。

**产品线判断规则**：
- 若涉及 `lms-access-default-*`、`lms-common-*` 模块 → 产品线 = **lms**
- 若涉及 `oc-opt-platform-*`、`oc-opt-terminal-*` 模块 → 产品线 = **oc**
- 若涉及 `cmd-unified-*`、`cmd-push-*` 模块 → 产品线 = **cmd**

#### 9.2 按产品线分组

将步骤5-8的所有输出按产品线进行分组：

| 产品线 | 包含内容 |
|-------|---------|
| **lms** | lms-access-default-plugin、lms-common-plugin、lms-base-producer 相关类、调度策略、依赖 |
| **oc** | oc-opt-platform-plugin、oc-opt-terminal-plugin、oc相关Service/Controller、Feign、DTO、依赖 |
| **cmd** | cmd相关模块... |

#### 9.3 生成各产品线 plan 文件

**【重要】每个产品线生成独立的 plan-{productLine}.md 文件**

每个文件包含以下内容：

**plan-{productLine}.md 内容结构**：

1. **应用交互mermaid图**（仅包含该产品线涉及的模块）
2. **模块决策表**（仅该产品线的模块）
3. **新建模块清单**（仅该产品线的新建模块）
4. **复用模块清单**（仅该产品线的复用模块）
5. **业务类清单**（仅该产品线的类，按 className 排序）
6. **关键类mermaid业务流程图**（仅该产品线的关键类）
7. **Plugin调度策略**（仅该产品线的调度配置）
8. **中间件配置**（仅该产品线使用的Redis和Consumer）
9. **依赖关系**（仅该产品线主模块的依赖）

**输出文件命名**：
```
plan-lms.md    → LMS产品线技术方案
plan-oc.md     → OC产品线技术方案
plan-cmd.md    → CMD产品线技术方案（若有）
```

**文件保存路径**：统一保存至当前需求目录下

- 输出：生成的文件列表，如 `plan-lms.md`、`plan-oc.md`

### 步骤10：生成汇总索引文件 plan-index.md【可选】

当涉及多个产品线时，生成汇总索引文件：

```markdown
# 技术方案索引

## 需求概述
{简要描述}

## 产物清单

| 文件 | 产品线 | 内容说明 |
|------|--------|---------|
| [plan-lms.md](plan-lms.md) | lms | 平台接入：枚举、Plugin、调度策略 |
| [plan-oc.md](plan-oc.md) | oc | 终端操作：Controller、Service、Feign |

## 交互流程

{跨产品线的整体交互mermaid图}
```

### 步骤11：阻塞等待用户确认
- 提示用户：请按照以下格式提示用户确认：
  |产品线|文档|
  |------|------|
  |lms|plan-lms.md|
  |oc|plan-oc.md|
  |cmd|plan-cmd.md|
  
- **必须暂停执行，等待用户输入**
- 用户回复"确认"→ 继续
- 用户提出修改 → 修改后重新等待确认
- **禁止在未获得确认前执行任何后续步骤**

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 文件输出要求
- **【强制】按产品线拆分输出**，每个产品线生成独立的 plan-{productLine}.md 文件
- **【强制】businessLogic 禁止使用简略描述，必须包含完整执行步骤**
- **【强制】所有清单必须使用表格格式**
- 每个 plan-{productLine}.md 必须包含：
  - 模块决策表（新建/复用/不需要及理由）
  - 新建模块清单（含 type、moduleDir、productLine、functionModule、businessName、appType、appName）
  - 复用模块清单
  - 业务类清单（含 className、productLine、targetModule、targetPackage、classType、apiEndpoint、businessLogic）
  - Plugin 调度策略（含 engineName、configFileName、dispatchers）
  - 中间件配置（含 redis_list、consumer_list）
  - 依赖关系（含 dependency_mapping）
- 当涉及多产品线时，生成 plan-index.md 汇总索引文件

### 输出文件清单
- plan-{productLine1}.md
- plan-{productLine2}.md
- plan-index.md（可选，当≥2个产品线时生成）
