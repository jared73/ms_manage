---
name: yto-dispatcher-plugin
description: 圆通订单中心Plugin插件策略配置。当用户说"配置Plugin策略"、"新建调度策略"时触发，也可被其他Skill的执行步骤触发
---

# Plugin 策略配置构建器
引导用户完成一次 Plugin 插件策略的配置流程，包括策略创建。

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
| pluginModule | String | 是 | Plugin 模块名称 | `lms-order-valid-plugin` |
| engineName | String | 是 | 引擎名称，格式为 `{业务}-{功能}` | `order-valid` |
| configFileName | String | 是 | 配置文件名称 | `bootstrap-plugin-order.yml` |
| dispatchers | Array | 是 | 调度策略列表，每项包含 key、description、chain（stage + plugins 列表） | 见 pipeline 输出 |

## 执行流程
### 步骤1：提取参数
- 从上下文自动提取参数：pluginModule、engineName、configFileName、dispatchers
- 参数任意缺失直接终止并提示缺失字段
- 输出格式：表格
  | 字段 | 值 |
  |------|-----|
  | pluginModule | {pluginModule}
  | engineName | {engineName} |
  | configFileName | {configFileName} |
  | dispatchers | {dispatchers 数量} |  

### 步骤2：定位资源文件目录
- 定位 pluginModule 的资源文件目录：`{pluginModule}/src/main/resources/`
- 输出：✅ 步骤2完成，资源文件目录已定位

### 步骤3：检查配置文件是否已存在，若存在则合并 dispatchers，若不存在则创建新文件
- 输出：{新建/合并追加}

### 步骤4：生成调度策略配置文件，必须遵守文档：`.spec/oc-spec-plugin-dispatcher.md`
- 根据 dispatchers 生成调度策略配置文件
- 配置文件名：{configFileName}
- 配置结构：framework.dispatcher.engineName + dispatchers 映射
- 每条 dispatcher 的 chain 必须遵循四阶段结构：参数初始化 → 参数校验 → 业务处理 → 生成返回值
- 输出：✅ 步骤4完成，调度策略配置文件已生成

### 步骤5：执行检查清单
- 详见 [checklist.md](checklist.md)，如果有任何项未通过，提示用户需要修复，修复后继续执行检查，最多检查 3 次
- 输出：检查清单结果

### 步骤6：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、调度策略配置结果
| 字段 | 值 |
|------|-----|
| pluginModule | {pluginModule} |
| engineName | {engineName} |
| configFileName | {configFileName} |
| 策略数量 | {dispatchers 数量} |

### 二、配置文件变更
| 变更位置 | 变更内容 |
|---------|---------|
| `{pluginModule}/src/main/resources/{configFileName}` | 新增/合并调度策略配置 |
