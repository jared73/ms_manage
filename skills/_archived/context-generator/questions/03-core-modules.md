# 质问 3/6：核心模块清单

## 收集证据

```bash
# 3.1 从 pom.xml 看模块
echo "=== 模块清单 ==="
grep "<artifactId>" $(find . -name "pom.xml" 2>/dev/null) 2>/dev/null

# 3.2 从 Application 类看
echo ""
echo "=== Application 类 ==="
find . -name "*Application.java" 2>/dev/null | head -20

# 3.3 从 package 推断
echo ""
echo "=== 包路径推断 ==="
find src -maxdepth 6 -name "controller" -type d 2>/dev/null | head -10
```

## 质问内容

```markdown
## 质问 3/6：核心模块清单

**现有证据**（pom 推断）：
- order（订单）
- payment（支付）
- inventory（库存）
- track（轨迹）
- user（用户）

**请确认/补充**：

A. 上述 5 个模块都存在  ← 推荐
B. 只有 3 个核心模块：______（请列出）
C. 有更多模块：______（请补充）
D. 模块列表完全不同：______（请列出）

**说明**：
- 模块名会出现在 module-responsibility.md
- 业务地图会按模块组织
- 模块数量建议 3–10 个
```

## 边界情况

### 情况 1：模块名带产品线前缀
```markdown
⚠️ 注意：模块名如果带产品线前缀，请去掉前缀
- oc-order → order
- lms-access → access
- cmd-track → track
```

### 情况 2：模块数量 > 10
```markdown
⚠️ 模块超过 10 个，建议按"核心模块"和"辅助模块"分类
核心模块：3–5 个（重点描述）
辅助模块：在 module-responsibility.md 中简述
```

### 情况 3：模块数量 < 3
```markdown
💡 模块少于 3 个可能是单体应用
按"功能领域"拆分（如 order、payment、user）
```

## 替换占位符

| 占位符 | 替换为 |
|--------|--------|
| `{{CORE_MODULES}}` | 逗号分隔的模块名（如 "order, payment, inventory"） |
| `{{MODULE_COUNT}}` | 模块数量 |

## 选 A 后的 context 内容

```markdown
## 核心模块

| 模块名 | 职责 | 关键能力 |
|--------|------|---------|
| order | 订单管理 | 创建/修改/查询/取消 |
| payment | 支付管理 | 支付/退款 |
| inventory | 库存管理 | 锁定/释放 |
| track | 轨迹管理 | 上报/查询 |
| user | 用户管理 | 注册/认证 |
```

## 后续联动

```yaml
# 这个问题的答案会影响：
# 1. module-responsibility.md 主体
# 2. business-map-{X}.md 中的"核心模块"部分
# 3. spec-generator 阶段 1.3 的"模块归属选项"
# 4. rules-generator 的"模块路径推断"
```
