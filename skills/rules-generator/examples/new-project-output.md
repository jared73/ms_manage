# rules-generator 新项目输出示例

> 给团队成员看"新项目跑完 rules-generator 后会长什么样"。

## 场景

```
你：帮我用 rules-generator 给新项目配 rule
AI：（执行 01-detect-project.md → 判定新项目）
    （问 1 个问题：包名风格）
    你：A
    （生成 4 个 rule 文件）
```

## 用户选择

```yaml
company: yto56
product_line: oc
package_style: cn.com.yto56.oc.{module}.{layer}
module_style: "{productLine}-{function}-{business}-{appType}"
controller_valid: "@Valid"
controller_swagger: "@Api + @ApiOperation"
controller_path: "类级 @RequestMapping"
service_transaction: "类级 @Transactional"
exception_handling: "全局 @ControllerAdvice"
log_format: "JSON"
```

## 生成的文件

### `.qoder/rules/always-on.md`（节选）

```markdown
# 项目硬约束

## 命名
- 包路径：`cn.com.yto56.oc.{module}.{layer}`
- 模块命名：`{productLine}-{function}-{business}-{appType}`

## Controller
- 必须 @RestController
- 参数校验：@Valid
- Swagger 注解：@Api + @ApiOperation
- 路径前缀：类级 @RequestMapping

## Service
- Impl 必须 @Service
- 事务注解：类级 @Transactional

## 异常处理
全局 @ControllerAdvice 统一处理

## 日志
- 格式：JSON
```

### `.qoder/rules/java-class.md`（节选）

```markdown
# Java 类通用规则

## 文件结构
1. package 声明
2. import（按字母排序）
3. 类注释
4. 类注解
5. 类声明
...
```

### `.qoder/rules/spec-reference.md`（节选）

```markdown
# 规范加载规则

| 任务 | 必读规范 |
|------|---------|
| 写 Controller | .spec/oc-spec-class-Controller.md |
| 写 Service | .spec/oc-spec-class-Service.md |
...
```

### `.qoder/rules/pom-xml.md`（节选）

```markdown
# POM 修改硬约束

## 版本管理
- 所有版本在父 POM 统一管理
- 子 POM 不写版本号
...
```

## 下一步

```bash
# 1. review 生成的 4 个 rule
cat .qoder/rules/*.md

# 2. 跑 spec-generator（按需，已归档 2026-06-28，详见 `.qoder/skills/_archived/ARCHIVED.md`）
# （生成 .spec/）

# 3. 开始写第一个 Controller
# （AI 会自动加载对应规范）
```
