# 质问 1/6：产品线划分

## 收集证据

```bash
# 1.1 从 pom.xml 推断产品线
echo "=== 从 pom.xml 推断 ==="
grep "<artifactId>" $(find . -name "pom.xml" 2>/dev/null) 2>/dev/null | \
  sed -E 's/.*<artifactId>([a-z]+)-.*/\1/' | sort -u

# 1.2 从代码包路径推断
echo "=== 从代码包路径推断 ==="
find src -name "*.java" 2>/dev/null | head -10 | \
  xargs -I {} dirname {} 2>/dev/null | sort -u | \
  awk -F'/' '{print $5}' | sort -u

# 1.3 从模块名推断
echo "=== 从模块名推断 ==="
find . -maxdepth 3 -name "pom.xml" 2>/dev/null | \
  xargs grep -h "<artifactId>" 2>/dev/null | head -20

# 1.4 从 README 推断
echo "=== 从 README 推断 ==="
grep -i "产品线\|product\|业务线\|平台" README.md 2>/dev/null | head -5
```

## 质问内容

```markdown
## 质问 1/6：产品线划分

**现有证据**：
- pom.xml 模块：`oc-order-api`、`oc-order-service`、`lms-access-default-api`、`cmd-track-push`
- 推断：至少 3 条产品线（oc / lms / cmd）
- 也有可能只有 2 条

**请确认/补充**：

A. 3 条产品线（OC 订单中心 + LMS 物流接入 + CMD 推送平台）  ← 推荐（如材料支持）
B. 2 条产品线（OC + LMS）
C. 1 条产品线（OC）
D. 其他组合：______（请列出）

**补充信息**（如选 A 或 B）：
- 产品线 1 名称：______（如 订单中心 / 订单处理）
- 产品线 2 名称：______（如 物流接入 / 平台接入）
- 产品线 3 名称：______（如 推送平台 / 消息推送）
```

## 边界情况

### 情况 1：项目材料推断出 1 条产品线
```markdown
⚠️ 材料只显示 1 条产品线
- 可能是项目确实只有 1 条
- 也可能其他产品线没被扫到
- 请确认
```

### 情况 2：材料推断出 ≥ 4 条
```markdown
⚠️ 材料推断出 4 条以上产品线
- 建议只选最重要的 3 条写入 context
- 其他可以在 architecture.md 中提及
```

### 情况 3：项目是单体产品（无产品线）
```markdown
💡 单体产品线可以选 C（1 条）
```

## 替换占位符

| 占位符 | 替换为 |
|--------|--------|
| `{{PRODUCT_LINES}}` | 逗号分隔的产品线代号（如 "oc, lms, cmd"） |
| `{{PRODUCT_LINES_FULL}}` | 完整产品线名（如 "订单中心, 物流接入, 推送平台"） |

## 选 A 后的 context 内容

```markdown
## 产品线

本项目包含 3 条产品线：

| 代号 | 全称 | 范围 |
|------|------|------|
| oc | 订单中心 | 订单全生命周期管理 |
| lms | 物流接入层 | 对接拼多多/微信/支付宝等外部平台 |
| cmd | 推送平台 | 向各渠道推送订单/轨迹消息 |
```

## 选 B 后的 context 内容

```markdown
## 产品线

本项目包含 2 条产品线：

| 代号 | 全称 | 范围 |
|------|------|------|
| oc | 订单中心 | 订单全生命周期管理 |
| lms | 物流接入层 | 对接外部平台 |
```

## 后续联动

```yaml
# 这个问题的答案会影响：
# 1. business-map-{X}.md 生成数量（每条产品线一份）
# 2. spec-generator 阶段 1.3 的"产品线选项"
# 3. rules-generator 阶段 02 的"包名推断"
```
