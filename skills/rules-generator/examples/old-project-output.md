# rules-generator 老项目输出示例

> 给团队成员看"老项目跑完 rules-generator 后会长什么样"。

## 场景

```
项目：oc-order（已存在 1 年，含 50 个 Controller、100 个 Service、3 个模块）

你：用 rules-generator 扫描这个项目
AI：（执行 01 → 判定老项目）
    （执行 02 → 扫描 6 维度）
    （执行 03 → 推断模式，发现 7 个不一致点）
    （执行 04 → 依次质问 7 个问题）
    你：全部选 A
    （执行 05 → 生成 4 个 rule + 1 份报告）
```

## 步骤 02 扫描报告（节选）

```markdown
## 1. 模块结构

| 模块名 | 类型 | Application 类 |
|--------|------|---------------|
| oc-order-api | ms | OrderApiApplication |
| oc-order-service | ms | OrderServiceApplication |
| oc-order-client | client | - |
| oc-common | common | - |

## 2. 类后缀分布

| 后缀 | 数量 | 占比 |
|------|------|------|
| Controller | 50 | 14.7% |
| Service | 100 | 29.4% |
| Impl | 100 | 29.4% |
| DTO | 90 | 26.5% |

## 3. 命名模式

- 包路径：cn.com.yto56.oc.{module}.{layer}（100% 一致）
- 类名：{业务}Controller / {业务}Service / {业务}ServiceImpl（100% 一致）

## 4. 注解模式

### Controller 注解
| 注解 | 频次 | 占比 | 备注 |
|------|------|------|------|
| @RestController | 50/50 | 100% | ✅ |
| @Valid | 38/50 | 76% | ⚠️ 不一致 |
| @Validated | 4/50 | 8% | ⚠️ 不一致 |
| 无校验 | 8/50 | 16% | ⚠️ 不一致 |
| @Api | 30/50 | 60% | ⚠️ 不一致 |
| @ApiOperation | 30/50 | 60% | ⚠️ 不一致 |

### Service 注解
| 注解 | 频次 | 占比 | 备注 |
|------|------|------|------|
| @Service | 100/100 | 100% | ✅ |
| @Transactional（类级） | 50/100 | 50% | ⚠️ 不一致 |
| @Transactional（方法级） | 30/100 | 30% | ⚠️ 不一致 |
| 无 @Transactional | 20/100 | 20% | ⚠️ 不一致 |

## 5. 技术栈

- Spring Boot 2.7.18
- Spring Cloud 2021.0.8
- MyBatis Plus 3.5.3
- MySQL 8.0
- Redis (Lettuce)
- RocketMQ 5.x
```

## 步骤 04 质问环节

```markdown
## 质问 1/7：包名风格

A. cn.com.{公司}.{业务线}.{模块}.{层}    ← 推荐
B. cn.com.{公司}.{业务线}.{模块}
C. cn.com.{业务线}.{模块}.{层}
D. 其他

你：A  ✅ 收到

## 质问 2/7：Controller @Valid

A. @Valid（推荐）
B. 无校验
C. @Validated
D. 其他

你：A  ✅ 收到

## 质问 3/7：Controller Swagger

A. @Api + @ApiOperation（推荐）
B. 仅 @ApiOperation
C. 无 Swagger
D. 其他

你：A  ✅ 收到

## 质问 4/7：Service 事务注解

A. 类级 @Transactional（推荐）
B. 方法级 @Transactional
C. 无 @Transactional
D. 其他

你：A  ✅ 收到

## 质问 5/7：异常处理

A. 全局 @ControllerAdvice（推荐）
B. Controller 内 try-catch
C. 抛自定义业务异常
D. 其他

你：A  ✅ 收到

## 质问 6/7：日志格式

A. JSON 格式（推荐）
B. 文本格式
C. 默认
D. 其他

你：A  ✅ 收到

## 质问 7/7：依赖注入

A. 构造器注入（推荐）
B. @Autowired 字段注入
C. @Resource 字段注入
D. 其他

你：A  ✅ 收到
```

## 步骤 05 生成结果

### 生成的文件

```
.qoder/rules/
├── always-on.md
├── java-class.md
├── spec-reference.md
├── pom-xml.md
└── GENERATION-REPORT.md  ← 老项目特有
```

### `.qoder/rules/GENERATION-REPORT.md`（节选）

```markdown
# rules-generator 生成报告

> 自动生成，记录扫描 + 推断 + 质问 + 生成的完整过程。

## 1. 项目类型
老项目

## 2. 扫描统计

### 2.1 基础数据
- Java 类总数：340
- 模块数：4
- 配置文件数：6
- 技术栈依赖：42 个

### 2.2 类后缀分布
| 后缀 | 数量 |
|------|------|
| Service | 100 |
| Impl | 100 |
| DTO | 90 |
| Controller | 50 |
| Req | 80 |
| Resp | 70 |

### 2.3 技术栈
| 技术 | 版本 |
|------|------|
| Spring Boot | 2.7.18 |
| Java | 8 |
| MyBatis Plus | 3.5.3 |
| MySQL | 8.0.33 |

## 3. 模式推断

### 3.1 完全一致项（直接采用）
- ✅ 包路径风格 cn.com.yto56.oc.{module}.{layer}（100%）
- ✅ 类命名 {业务}Controller / {业务}Service（100%）
- ✅ @RestController（100%）
- ✅ @Service（100%）

### 3.2 不一致项（已质询）
- ⚠️ Controller @Valid 用法（不一致度 24%）
- ⚠️ Controller Swagger 注解（不一致度 40%）
- ⚠️ Service 事务注解（不一致度 50%）
- ⚠️ 异常处理（不一致度 X%）
- ⚠️ 日志格式（不一致度 X%）

## 4. 用户选择

| # | 主题 | 选择 | 备注 |
|---|------|------|------|
| 1 | 包名风格 | A | 5 层结构 |
| 2 | Controller @Valid | A | 标准 @Valid |
| 3 | Controller Swagger | A | @Api + @ApiOperation |
| 4 | Service 事务 | A | 类级 @Transactional |
| 5 | 异常处理 | A | 全局 @ControllerAdvice |
| 6 | 日志格式 | A | JSON 格式 |
| 7 | 依赖注入 | A | 构造器注入 |

## 5. 生成的 rule

- .qoder/rules/always-on.md
- .qoder/rules/java-class.md
- .qoder/rules/spec-reference.md
- .qoder/rules/pom-xml.md

## 6. 已知差距

> 基于扫描结果，以下代码与新 rule 不一致，需要后续改造：

| 类别 | 不一致数 | 占比 | 建议 |
|------|---------|------|------|
| Controller 缺 @Valid | 12 | 24% | 跑 code-fixer |
| Controller 缺 Swagger | 20 | 40% | 跑 code-fixer |
| Impl 缺类级 @Transactional | 50 | 50% | 跑 code-fixer（注意 readOnly） |
| 异常处理不规范 | X | X% | 跑 code-fixer |
| 字段注入（@Autowired） | Y | Y% | 改为构造器注入 |

## 7. 下一步建议

1. **跑 rules-checker** — 验证当前代码合规度
2. **跑 code-fixer** — 批量修复（建议分模块分批）
3. **跑 spec-generator** — 生成 .spec/（已归档 2026-06-28，详见 `.qoder/skills/_archived/ARCHIVED.md`）
4. **提交 rule 到 Git** — 团队共享
5. **CI 集成 rules-checker** — 拦新代码不合规

## 8. 反馈

如果生成的 rule 有问题：
1. 直接编辑 .qoder/rules/*.md
2. 在本报告末尾加备注
3. 反馈给 skill 维护者
```

## 下一步行动

```bash
# 1. review 生成的 rule
cat .qoder/rules/*.md
cat .qoder/rules/GENERATION-REPORT.md

# 2. 跑 rules-checker 看哪些代码不合规
sdd check

# 3. 跑 code-fixer 自动修复（按模块分批）
sdd fix --module=oc-order-api

# 4. 跑 spec-generator（已归档）
sdd gen-spec

# 5. 提交到 Git
git add .qoder/
git commit -m "chore: 引入 SDD 规则集"
git push
```
