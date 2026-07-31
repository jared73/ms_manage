---
name: yto-pom-dependency
description: 批量为指定微服务的 pom.xml 增加 Maven 依赖,当用户说"添加模块依赖"或"引入模块依赖"时触发，也可被其他Skill的执行步骤触发
---

# POM 依赖注入器
批量为指定微服务的 pom.xml 增加 Maven 依赖。

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
| dependencyMapping | Object | 是 | 依赖映射关系，key 为微服务模块名，value 为需要引入的依赖模块列表 | 见下方示例 |

### dependencyMapping 结构示例

```json
{
  "lms-order-create-ms": [
    "lms-order-common",
    "lms-order-dto",
    "lms-order-create-plugin",
    "lms-order-create-mysql-service"
  ],
  "lms-order-query-ms": [
    "lms-order-common",
    "lms-order-dto",
    "lms-order-query-plugin",
    "lms-order-query-es-service"
  ]
}
```

## 执行流程

### 步骤1：提取参数 dependencyMapping
- 参数缺失直接终止并提示缺失字段
- 输出：`dependencyMapping 包含 {N} 个微服务，共 {M} 条依赖关系`

### 步骤2：读取依赖规范 `.spec/oc-spec-pom-module.md`
- 输出：✅ 步骤2完成，已读取 POM 依赖规范

### 步骤3：遍历 dependencyMapping，对 dependencyMapping 中的每个微服务模块执行步骤4-6
- 输出：开始遍历 dependencyMapping

### 步骤4：定位并读取微服务 pom.xml
- 定位当前微服务模块的 pom.xml 文件路径
- 输出：✅ 步骤4完成，已定位 {微服务名}/pom.xml

### 步骤5：逐个引入依赖，检查嵌套依赖，已存在依赖则跳过，必须遵守规范：`.spec/oc-spec-pom-module.md`
- 对当前微服务的依赖列表中的每个依赖模块：
  - 检查 pom.xml 中是否已存在该依赖
  - 若已存在 → 跳过
  - 若不存在 → 按规范添加依赖声明
- 输出：✅ 步骤5完成，{微服务名} 依赖注入结果：{新增N个/全部已存在跳过}

### 步骤6：回到步骤3继续遍历
- 若还有未处理的微服务，回到步骤3
- 若全部处理完毕，进入步骤7

### 步骤7：执行检查清单
- 详见 [checklist.md](checklist.md)，如果有任何项未通过，提示用户需要修复，修复后继续执行检查，最多检查 3 次
- 输出：检查清单结果

### 步骤8：输出最终结果
- 按照 输出格式固定模板输出

## 检查清单

详见 [checklist.md](checklist.md)

## 输出格式
### 一、依赖注入汇总

| 微服务模块 | 新增依赖 | 跳过（已存在） |
|-----------|---------|--------------|
| {微服务1} | {依赖A}, {依赖B} | {依赖C} |
| {微服务2} | {依赖D} | {依赖E}, {依赖F} |

### 二、详细变更记录

| 变更位置 | 变更内容 |
|---------|---------|
| {微服务1}/pom.xml \<dependencies\> | 新增 {依赖A} 依赖引入 |
| {微服务1}/pom.xml \<dependencies\> | 新增 {依赖B} 依赖引入 |
| {微服务2}/pom.xml \<dependencies\> | 新增 {依赖D} 依赖引入 |
