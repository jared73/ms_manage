# context-generator 新项目输出示例

## 场景

```
你：帮我用 context-generator 给新项目配 context
AI：（执行 01-detect-project.md → 判定新项目）
    （问 1 个问题：项目元信息）
    你：A. 项目名 = 圆通订单中心
       定位 = 处理订单全生命周期
       产品线 = oc, lms, cmd
       业务 = 订单
       用户 = 内部
    （生成 6 个 context 文件）
```

## 生成的文件

```
.qoder/context/
├── project-overview.md
├── architecture.md
├── module-responsibility.md
├── business-map-oc.md
├── business-map-lms.md
├── business-map-cmd.md
├── tech-stack.md
└── glossary.md
```

## `.qoder/context/project-overview.md`（示例）

```markdown
# 圆通订单中心

> 处理订单全生命周期

## 一、项目定位

### 一句话定位
处理订单全生命周期

### 项目目标
- 业务目标 1：[待补充]
- 业务目标 2：[待补充]

## 二、项目阶段

**当前阶段**：[待补充]

## 三、关键时间节点

| 节点 | 日期 | 备注 |
|------|------|------|
| 立项 | [待补充] | |
| 首次上线 | [待补充] | |

## 四、核心指标

[待补充]

## 五、产品线

本项目包含 oc, lms, cmd 等产品线。

## 六、相关人/团队

[待补充]
```

## 下一步

```bash
# 1. review 生成的 6 个 context
cat .qoder/context/*.md

# 2. 跑 rules-generator
sdd gen-rules

# 3. 补充 [待补充] 字段
# （找团队成员、PM 补充）
```
