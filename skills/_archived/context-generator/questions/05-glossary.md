# 质问 5/6：业务术语

## 收集证据

```bash
# 5.1 从 README 看
echo "=== README 中的术语 ==="
grep -E "^[A-Z]{2,}|缩写|abbreviation" README.md 2>/dev/null | head -10

# 5.2 从产品线代号
echo ""
echo "=== 产品线代号 ==="
grep "<artifactId>" pom.xml 2>/dev/null | grep -oE "^[a-z]+-" | sort -u

# 5.3 从代码注释
echo ""
echo "=== 代码注释中的术语 ==="
grep -rh "^\s*\*\s*[A-Z]{2,}" src --include="*.java" 2>/dev/null | head -10

# 5.4 从 spec
echo ""
echo "=== specs 中的术语 ==="
[ -d "specs" ] && find specs -name "*.md" 2>/dev/null | xargs grep -E "^[A-Z]{2,}" 2>/dev/null | head -10
```

## 质问内容

```markdown
## 质问 5/6：业务术语

**现有证据**：
- 产品线代号：LMS / OC / CMD
- README 提到：拼多多 / 微信 / 支付宝（外部平台名）
- 业务名词：订单 / 轨迹 / 推送

**请补充业务术语**（每行一个，格式：`缩写 = 全称`）：

示例格式：
LMS = 物流管理系统
OC = 订单中心
CMD = 推送平台
PINDUODUO = 拼多多
WXPAY = 微信支付

**你的回答**（请补充）：
```
LMS = ______
OC = ______
CMD = ______
PINDUODUO = ______
______ = ______
```

**说明**：
- 术语会进入 glossary.md
- 后续 AI 写文档时必须用全称或加注
- 团队新人看 glossary.md 即可理解项目
```

## 边界情况

### 情况 1：项目术语 < 5 个
```markdown
⚠️ 术语少于 5 个可能不全
建议补充：模块名缩写、业务名词、外部平台名
```

### 情况 2：项目术语 > 30 个
```markdown
⚠️ 术语超过 30 个，建议分两组
- 核心术语（必填）：10–15 个
- 扩展术语（选填）：其余
```

## 替换占位符

| 占位符 | 替换为 |
|--------|--------|
| `{{GLOSSARY}}` | 用户回答的术语列表 |

## 选 A 后的 context 内容

```markdown
## 术语表

| 术语 | 全称 | 含义 |
|------|------|------|
| LMS | Logistics Management System | 物流管理系统（平台接入层） |
| OC | Order Center | 订单中心 |
| CMD | Command Dispatch | 推送平台 |
| PINDUODUO | 拼多多 | 外部电商平台 |
| WXPAY | 微信支付 | 外部支付平台 |
| T_ORDER | t_order | 订单表 |
| ... | ... | ... |
```

## 后续联动

```yaml
# 这个问题的答案会影响：
# 1. glossary.md 主体
# 2. spec-generator 阶段 1.3 的"角色"选项
# 3. rules-generator 的命名规范
# 4. api-contract-generator 的接口文档页头
```

## 选填建议

**核心术语建议**（10 个左右）：
1. 项目/产品线代号（LMS / OC / CMD）
2. 核心业务名词（订单 / 轨迹 / 推送）
3. 主要外部平台（拼多多 / 微信 / 支付宝）
4. 关键模块名（order / payment / inventory）
5. 常用错误码前缀（OC_0001）

**扩展术语**（按需）：
- 内部系统名
- 业务角色名
- 数据表名
