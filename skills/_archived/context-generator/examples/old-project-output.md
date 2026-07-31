# context-generator 老项目输出示例

## 场景

```
项目：oc-order（已存在 4 年，含 README、设计文档、3 条产品线）

你：用 context-generator 补全 context
AI：（执行 01 → 判定老项目）
    （执行 02 → 收集 README.md / 架构图 / PRD 文档）
    （执行 03 → 发现 6 个模糊点）
    （执行 04 → 依次质问 6 个问题）
    你：批量回答
    （执行 05 → 生成 6+3=9 个 context + 1 份报告）
```

## 步骤 02 材料收集报告

```markdown
## 已收集材料

- README.md（1,234 行，包含项目介绍 + 部署说明 + 团队成员）
- docs/architecture.md（架构设计文档）
- docs/prd/order.md（订单 PRD）
- docs/prd/payment.md（支付 PRD）
- 架构图：docs/architecture.drawio（存在）
- 部署文档：docs/deploy.md

共 6 份核心材料
```

## 步骤 04 批量质问

```markdown
## 质问 1/6：产品线划分

A. 3 条产品线（OC 订单中心 + LMS 物流接入 + CMD 推送平台）  ← 推荐
B. 2 条
C. 1 条
D. 其他

你：A  ✅

## 质问 2/6：项目一句话定位

A. 处理订单全生命周期  ← 推荐
B. 物流接入层
C. 推送平台
D. 其他：______

你：A  ✅

## 质问 3/6：核心模块清单

A. order, payment, inventory, track, user（5 个）
B. 3 个：______
C. 更多
D. 不同

你：A  ✅

## 质问 4/6：技术架构

A. 微服务架构  ← 推荐
B. 单体
C. 模块化单体
D. 其他

你：A  ✅

## 质问 5/6：业务术语

请补充（每行一个）：
LMS = ______
OC = ______
CMD = ______
PINDUODUO = ______

你：LMS = 物流管理系统
   OC = 订单中心
   CMD = 推送平台
   PINDUODUO = 拼多多
   ✅

## 质问 6/6：项目成熟度

A. 稳定期  ← 推荐
B. 成长期
C. 起步期
D. 重构期

你：A  ✅
```

## 步骤 05 生成结果

### 生成的文件

```
.qoder/context/
├── project-overview.md         (1.2 KB)
├── architecture.md              (1.5 KB)
├── module-responsibility.md     (1.0 KB)
├── business-map-oc.md           (1.8 KB)
├── business-map-lms.md          (1.5 KB)
├── business-map-cmd.md          (1.2 KB)
├── tech-stack.md                (0.8 KB)
├── glossary.md                  (0.5 KB)
└── GENERATION-REPORT.md         (3.5 KB)
```

### `.qoder/context/GENERATION-REPORT.md`（节选）

```markdown
# context-generator 生成报告

## 1. 项目类型
老项目

## 2. 材料收集

### 2.1 已收集材料
- README.md
- docs/architecture.md
- docs/prd/order.md
- docs/prd/payment.md
- docs/architecture.drawio
- docs/deploy.md

共 6 份核心材料

### 2.2 缺失材料
- 没有用户故事文档
- 没有术语表（从 README 推断）
- 没有架构决策记录（ADR）

## 3. 模式推断

### 3.1 充分项
- ✅ 项目名称（README）
- ✅ 技术栈（pom.xml 推断）
- ✅ 团队成员（README 提及）

### 3.2 模糊项
- ⚠️ 一句话定位（README 无明确描述）
- ⚠️ 业务指标（无 SLA/DAU 数据）
- ⚠️ 业务术语（README 散落）

## 4. 用户回答

| # | 主题 | 回答 | 备注 |
|---|------|------|------|
| 1 | 产品线 | oc, lms, cmd | 3 条 |
| 2 | 定位 | 处理订单全生命周期 | |
| 3 | 模块 | order, payment, inventory, track, user | 5 个 |
| 4 | 架构 | 微服务 | |
| 5 | 术语 | LMS=物流, OC=订单, CMD=推送, PINDUODUO=拼多多 | 4 个 |
| 6 | 成熟度 | 稳定期 | 已运行 4 年 |

## 5. 生成的 context
- 8 份 context 文件
- 1 份生成报告

## 6. 已知不足
- 业务指标不完整
- ADR 缺失
- 用户故事未整理

## 7. 下一步
- 跑 rules-generator
- 补充业务指标
- 创建 ADR 目录
```

## 下一步行动

```bash
# 1. review 生成的 context
cat .qoder/context/*.md
cat .qoder/context/GENERATION-REPORT.md

# 2. 跑 rules-generator
sdd gen-rules

# 3. 补充缺失信息
# - 业务指标（找 PMO）
# - ADR（找架构师）
# - 用户故事（从 specs/ 抽取）

# 4. 提交到 Git
git add .qoder/context/
git commit -m "chore: 补全 context 文档"
git push
```
