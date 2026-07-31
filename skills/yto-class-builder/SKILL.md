---
name: yto-class-builder
description: 圆通订单中心创建java类。当用户说"创建java类"时触发，也可被其他Skill的执行步骤触发。
---

# Java 类构建器

## 强制规范
- 遵循 always-on.md 全局规则
- 每执行完一条执行步骤，必须明确输出当前步骤执行结果，禁止静默执行
- 流程结束后，必须严格按照输出格式固定模板输出，不得随意改动格式、增减字段

### 任务拆解强制规则

**在生成待办任务列表（Todo）时，必须严格遵循以下规则：**

1. **每个步骤必须作为独立的一条待办任务**，共 5 条，不得合并、不得省略
2. **每条待办必须包含完整的步骤名称**：
   - `步骤 1: 提取参数 — 从上下文自动提取所需参数`
   - `步骤 2: 计算 classType — 根据类名拆分规则确定类型`
   - `步骤 3: 创建 Java 类 — 读取规范并生成类文件`
   - `步骤 4: 执行检查清单 — 逐项校验代码规范`
   - `步骤 5: 输出最终结果 — 按固定模板输出`
3. **每个步骤必须独立完成后才能标记为完成**

---

## 所需参数
| 字段 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| className | String | 是 | 类名称 | `OrderCreateController` |
| productLine | String | 是 | 业务产品线标识 | `lms` |
| targetModule | String | 是 | 目标模块名称 | `lms-order-create-ms` |
| targetPackage | String | 是 | 目标包路径 | `com.yto.hermes.lms.order.create.controller` |
| businessLogic | String | 是 | 业务逻辑说明，描述该类需要实现的业务功能 | "接收大张订单创建请求，调用Plugin引擎处理" |

## 执行流程
### 步骤1：提取参数
- 从上下文自动提取参数：className、productLine、targetModule、targetPackage、businessLogic
- 参数任意缺失直接终止并提示缺失字段
- 输出：按照下面表格格式输出参数
  | className | productLine | targetModule | targetPackage |
  |------|------|------|------|
  | {className} | {productLine} | {targetModule} | {targetPackage} |

### 步骤2：计算classType，规则如下
- classType规则：将驼峰类名按单词边界拆分，取最后一个单词作为 classType（例如 `OrderCreateController` → `Controller`，`OrderDto` → `Dto`）。由于项目禁止连续大写，拆分直接以单个大写字母为单词起始即可
- 输出：✅ 步骤2完成，已计算出 classType 为 {classType}

### 步骤3：创建 Java 类，已存在则跳过，必须遵守规范：`oc-spec-content.md`和`oc-spec-class-{classType}.md`
- 必须实现businessLogic描述的业务功能
- 输出：✅ 步骤3完成，{className}.java 已创建

### 步骤4：执行检查清单
- 逐项检查代码是否符合以下清单，任何一项未通过则输出失败项及修复建议，等待用户修复后重新执行本步骤（最多重复检查 3 次）
- 输出：检查清单结果

### 步骤5：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、类创建结果
| className | classType | targetModule | targetPackage | 文件路径 | 状态 | businessLogic |
|------|------|------|------|------|------|------|
| {className} | {classType} | {targetModule} | {targetPackage} | {完整文件路径} | {已创建/已存在跳过} | {businessLogic} |
