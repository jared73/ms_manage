---
name: spec-reference
description: 何时加载哪份 .spec 规范（强制关联）
---

# 规范加载规则

> **核心约束**：写任何代码前，AI 必须主动加载对应的 `.spec/` 文档。
> 这是"先 spec 后 code"的核心机制。

## 1. 强制加载规则

### 写代码前必读规范

| 任务类型 | 必读规范 | 不读不允许写 |
|---------|---------|------------|
| 写 Controller | `.spec/{{PRODUCT_LINE}}-spec-class-Controller.md` | ❌ |
| 写 Service 接口 | `.spec/{{PRODUCT_LINE}}-spec-class-Service.md` | ❌ |
| 写 Service 实现 | `.spec/{{PRODUCT_LINE}}-spec-class-Impl.md` | ❌ |
| 写 DTO | `.spec/{{PRODUCT_LINE}}-spec-class-Dto.md` | ❌ |
| 写 Req | `.spec/{{PRODUCT_LINE}}-spec-class-Req.md` | ❌ |
| 写 Resp | `.spec/{{PRODUCT_LINE}}-spec-class-Resp.md` | ❌ |
| 写 Feign Client | `.spec/{{PRODUCT_LINE}}-spec-class-Feign.md` | ❌ |
| 写 Plugin | `.spec/{{PRODUCT_LINE}}-spec-class-Plugin.md` + `-plugin-dispatcher.md` | ❌ |
| 写 MQ 消息体 | `.spec/{{PRODUCT_LINE}}-spec-class-Mo.md` | ❌ |
| 写 MQ 生产者 | `.spec/{{PRODUCT_LINE}}-spec-class-Producer.md` | ❌ |
| 写枚举 | `.spec/{{PRODUCT_LINE}}-spec-class-Enum.md` | ❌ |
| 写业务异常 | `.spec/{{PRODUCT_LINE}}-spec-class-Exception.md` + `-ExceptionEnum.md` | ❌ |
| 写常量类 | `.spec/{{PRODUCT_LINE}}-spec-class-Constant.md` | ❌ |
| 写工具类 | `.spec/{{PRODUCT_LINE}}-spec-class-Util.md` | ❌ |
| 新建模块 | `.spec/{{PRODUCT_LINE}}-spec-module-{ms\|client\|plugin\|service\|producer\|common}.md` | ❌ |
| 改 pom.xml | `.spec/{{PRODUCT_LINE}}-spec-pom-base.md` + `-pom-module.md` | ❌ |
| 加中间件（Redis） | `.spec/{{PRODUCT_LINE}}-spec-middleware-redis.md` | ❌ |
| 加 MQ 消费 | `.spec/{{PRODUCT_LINE}}-spec-middleware-consumer.md` | ❌ |
| 写测试 | `.spec/{{PRODUCT_LINE}}-spec-test.md` | ❌ |

## 2. 加载顺序

```
1. 先读 .spec/{{PRODUCT_LINE}}-spec-content.md（项目结构总览）
2. 再读对应的 class-spec 或 module-spec
3. 写代码
4. 写完后用 .spec 规则自检
```

## 3. AI 工作流

每次用户说"写一个 XxxController"时，AI 必须按以下顺序执行：

```
Step 1：询问业务背景（如果不清楚）
  - 这个 Controller 做什么？
  - 涉及哪个业务模块？
  - 入参/出参是什么？

Step 2：加载规范
  - 读取 .spec/{{PRODUCT_LINE}}-spec-class-Controller.md
  - 读取相关 .spec/{{PRODUCT_LINE}}-spec-class-Req.md / -Resp.md / -Service.md / -Impl.md
  - 读取 .spec/{{PRODUCT_LINE}}-spec-content.md（如果项目结构不熟）

Step 3：生成代码
  - 严格按规范中的注解、命名、注释要求
  - 引用规范中的反例清单，避免违规

Step 4：自检
  - 对照规范中的"反例清单"检查
  - 列出所有可能不合规的点
  - 提醒用户 review
```

## 4. 反例（不允许）

| 反例 | 后果 |
|------|------|
| 不读规范直接写代码 | 生成的代码不合规，CI 红灯 |
| 读完规范但忽略约束 | 同上 |
| 业务举例时编造（无规范依据） | 团队风格不一致 |
| 修改规范后不通知团队 | 规范形同虚设 |

## 5. 规范缺失怎么办

如果 `.spec/` 下没有对应文档：

```
1. 不要瞎写
2. 询问团队（架构师 / Tech Lead）
3. 临时参考 always-on.md + java-class.md
4. 在 PR 中说明"规范缺失，建议补充"
5. 补充规范到 .spec/
```

## 6. 规范变更流程

修改 `.spec/` 下任何文件，必须：

```
1. 创建 PR（标题：docs(spec): 修改 Xxx 规范）
2. @ 架构师 + Tech Lead review
3. 在团队群通知
4. 通过后合并
5. 同步更新相关代码（如有）
```

## 7. 触发条件示例

| 用户说 | AI 必须先读 |
|--------|------------|
| "写一个创建订单的 Controller" | `oc-spec-class-Controller.md` + `-Req.md` + `-Resp.md` |
| "新增订单查询模块" | `oc-spec-module-ms.md` + `-spec-content.md` |
| "改一下 pom 依赖" | `oc-spec-pom-base.md` + `-pom-module.md` |
| "加个 Redis 缓存" | `oc-spec-middleware-redis.md` |
| "写个单元测试" | `oc-spec-test.md` + 对应 class-spec |
