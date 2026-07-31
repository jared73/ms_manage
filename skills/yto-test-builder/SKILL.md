---
name: yto-test-builder
description: 圆通订单中心自动生成单元测试类。在业务代码生成完成后自动触发，为Controller/Consumer/Service/ServiceImpl生成标准JUnit4单元测试。
---

# 订单中心单元测试生成器
根据已生成的业务Java类，自动创建符合项目规范的单元测试用例。

## 适用场景
- 为订单中心已生成的Java类创建单元测试，必须遵循 .spec 目录下的测试规范。
- 两种使用场景：
    - 由全链路开发工作流执行到测试阶段自动触发，代码生成后立即生成测试。
    - 用户直接触发，为已存在的业务类补充单元测试。

## 强制规范
- 遵循 always-on.md 全局规则
- 测试类必须与业务类在同一包路径下，放在 src/test/java 目录
- **允许扫描的范围**：被测试的业务类代码、业务类依赖的 DTO/Entity/Enum/Feign/Producer 类
- **禁止扫描的范围**：项目内其他业务应用的测试代码

### 任务拆解强制规则

**在生成待办任务列表（Todo）时，必须严格遵循以下规则：**

1. **每个步骤必须作为独立的一条待办任务**，共 9 条，不得合并、不得省略
2. **每条待办必须包含完整的步骤名称**：
   - `步骤 1: 提取参数 — 从上下文提取测试类信息`
   - `步骤 2: 区分触发模式 — 工作流触发或用户触发`
   - `步骤 3: 读取单元测试规范 — 读取 .spec/oc-spec-test.md`
   - `步骤 4: 框架版本校验 — 确认 JUnit 版本`
   - `步骤 5: 读取被测试业务类 — 读取源代码`
   - `步骤 6: 读取业务类依赖类 — 读取 DTO/Entity/Enum 等`
   - `步骤 7: 判断被测试类类型 — Controller/Consumer/ServiceImpl/Service`
   - `步骤 8: 生成测试类 — 根据类型生成对应测试`
   - `步骤 9: 执行检查清单 — 校验测试类规范`
3. **每个步骤必须独立完成后才能标记为完成**

---

## 所需参数

| 字段 | 类型 | 必填 | 说明 | 示例 |
|------|------|------|------|------|
| productLine | String | 是 | 业务产品线标识 | `lms`,`test` |
| sourceClassName | String | 是 | 被测试的业务Java类全名 | `OrderController`、`OrderServiceImpl` |
| targetModule | String | 是 | 测试类所属模块 | `lms-order-ms` |
| targetPackage | String | 是 | 测试类包路径（与业务类保持一致） | `cn.com.yto56.lms.order.controller` |

## 输入向导

与步骤 2 合并执行：
- 被工作流触发：跳过交互，参数缺失直接终止
- 用户直接触发：引导补齐参数后执行

## 执行流程

### 步骤1：提取参数
- 从上下文或工作流参数中提取：productLine、sourceClassName、targetModule、targetPackage
- 参数任意缺失直接终止并提示缺失字段
- 输出：`productLine={productLine}, sourceClassName={sourceClassName}, targetModule={targetModule}, targetPackage={targetPackage}`

### 步骤2：区分触发模式
- 被工作流触发：跳过交互，参数缺失直接终止
- 用户直接触发：引导补齐参数后执行
- 测试类名称固定规则：{sourceClassName} + Test
- 测试模块固定规则：与业务类保持同一模块，目录为 src/test/java

### 步骤3：读取单元测试规范
- 读取文件：`.spec/oc-spec-test.md`
- **禁止读取本步骤指定文件之外的任何文件**
- 输出：✅ 步骤3完成，已读取单元测试规范

### 步骤4：框架版本校验
- 读取项目 pom.xml 中 surefire 插件版本和 JUnit 依赖
- 确认 JUnit 版本（4 或 5），若无法确认则默认 JUnit 4
- 将确认结果作为后续步骤的框架选择依据
- **禁止不校验就默认使用 JUnit 5**
- 输出：✅ 步骤4完成，JUnit 版本确认为 {JUnit4/JUnit5}

### 步骤5：读取被测试业务类
- 读取被测试的业务类源代码：`{targetModule}/src/main/java/{targetPackage转路径}/{sourceClassName}.java`
- 输出：✅ 步骤5完成，已读取 {sourceClassName}.java

### 步骤6：读取业务类依赖类
- 读取业务类依赖的DTO、Entity、Enum等数据对象类（仅允许扫描范围内列出的类）
- 输出：✅ 步骤6完成，已读取 {N} 个依赖类

### 步骤7：判断被测试类类型
- 根据 sourceClassName 后缀判断类类型：
  - 以 `Controller` 结尾 → Controller 测试
  - 以 `Consumer` 结尾 → Consumer 测试
  - 以 `ServiceImpl` 结尾 → ServiceImpl 测试
  - 以 `Service` 结尾且不以 `ServiceImpl` 结尾 → Service 测试
- 输出：被测试类类型判定为 {Controller/Consumer/ServiceImpl/Service}

### 步骤8：生成测试类
- 根据步骤7判定的类类型生成对应测试：
  - Controller：MockMvc 接口测试
  - Consumer：MQ 消费者测试
  - Service/ServiceImpl：Mockito 单元测试
- 必须遵守文档：`.spec/oc-spec-test.md`
- 测试类名：{sourceClassName}Test
- 输出：✅ 步骤8完成，{sourceClassName}Test.java 已生成

### 步骤9：写入测试类文件
- 在 targetModule 的 src/test/java 对应包路径下写入测试类
- 文件路径：`{targetModule}/src/test/java/{targetPackage转路径}/{sourceClassName}Test.java`
- 输出：✅ 步骤9完成，测试类已写入

### 步骤10：执行检查清单
- 详见 [checklist.md](checklist.md)，如果有任何项未通过，提示用户需要修复，修复后继续执行检查，最多检查 3 次
- 输出：检查清单结果

### 步骤11：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、测试类信息收集结果

| 字段 | 值 |
|------|-----|
| 被测试类名 | {sourceClassName} |
| 测试类名 | {sourceClassName}Test |
| 目标模块 | {targetModule} |
| 测试包路径 | {targetPackage} |
| 对应规范文件 | .spec/oc-spec-test.md |

### 二、测试类创建完成
| 变更位置 | 变更内容 |
|---------|---------|
| 测试类 | 创建 `{targetModule}/src/test/java/{targetPackage}.{sourceClassName}Test.java` |
