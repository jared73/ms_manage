# 质问 9/9：模块命名

## 扫描命令

```bash
echo "=== 模块列表 ==="
grep "<module>" pom.xml | sed 's/.*<module>\(.*\)<\/module>.*/\1/'

echo ""
echo "=== Application 类 ==="
find . -name "*Application.java" -not -path "*/target/*"

echo ""
echo "=== 模块命名样本 ==="
grep "<module>" pom.xml | head -10
```

## 质问内容

```markdown
## 质问 9/9：模块命名

**扫描发现**（基于现有模块名推断）：

假设扫描到模块 `oc-order-api`：

- A. {业务线}-{功能}-{业务}-{appType}            ← 推荐
- B. {业务线}-{appType}                          （简化）
- C. {业务}-{appType}                            （无业务线）
- D. 其他（请说明）

**为什么 A 推荐**：
- 4 段式命名，结构清晰
- 第一段：业务线（多产品线必须）
- 第二段：功能（access/order/pay）
- 第三段：业务（具体业务名）
- 第四段：appType（api/ms/consumer/job）

**方式 A 示例**：
```
lms-access-default-api        ← 平台接入-默认接入-默认-对前端
lms-access-default-ms         ← 平台接入-默认接入-默认-微服务间
oc-order-createorder-api      ← 订单中心-订单-下单-对前端
oc-order-updateorder-consumer ← 订单中心-订单-改单-MQ消费
oc-order-queryjob-job         ← 订单中心-订单-查询-定时任务
```

**方式 B 示例**（简化）：
```
lms-api
oc-ms
oc-consumer
```

**方式 C 示例**（无业务线）：
```
order-api
order-ms
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 边界情况

### 情况 1：项目是单模块
```markdown
💡 单模块项目可以选 C（无业务线）
```

### 情况 2：项目是多业务线（必须 A）
```markdown
✅ 项目含多个业务线（lms/oc/cmd）
强烈建议选 A，避免命名冲突
```

## 选 A 后的 rule 内容

```markdown
## 模块命名

### 命名规则
- 格式：{productLine}-{function}-{business}-{appType}
- productLine：业务线（lms / oc / cmd）
- function：功能（access / order / pay）
- business：业务（default / createorder）
- appType：应用类型（api / ms / consumer / job）

### appType 说明
- api：与前端交互
- ms：微服务间内部调用
- consumer：仅消费 MQ
- job：定时任务

### 反例
- ❌ oc-api（缺 function 和 business）
- ❌ orderApi（驼峰，不符合）
- ❌ oc_order_api（下划线，应该用短横线）
```

## 选 B 后的 rule 内容

```markdown
## 模块命名
- 格式：{业务线}-{appType}
- 简化版（2 段）
- 适合模块较少的项目
```

## 选 C 后的 rule 内容

```markdown
## 模块命名
- 格式：{业务}-{appType}
- 无业务线前缀
- 适合单业务线 / 单模块项目
```
