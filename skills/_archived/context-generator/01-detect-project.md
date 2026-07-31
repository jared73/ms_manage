# 01 · 项目类型识别

> 目标：在做任何事之前，先判断这是新项目还是老项目。

## 判定逻辑

按以下顺序检查，**任一为真 = 老项目**：

```bash
# 信号 1：存在 README.md
[ -f "README.md" ] && [ $(wc -l < README.md) -gt 10 ]

# 信号 2：存在 docs/ 目录且有内容
[ -d "docs" ] && find docs -name "*.md" -o -name "*.docx" 2>/dev/null | head -1

# 信号 3：存在 Confluence/Wiki 导出（常见目录）
[ -d "wiki" ] || [ -d "docs/wiki" ] || [ -d "confluence-export" ]

# 信号 4：存在架构图（drawio/png/svg）
find . -maxdepth 3 -name "*.drawio" -o -name "*architecture*.png" -o -name "*architecture*.svg" 2>/dev/null | head -1

# 信号 5：存在产品文档目录
[ -d "product-docs" ] || [ -d "prd" ]

# 信号 6：项目运行历史
[ -d ".git" ] && [ $(git log --oneline 2>/dev/null | wc -l) -gt 50 ]
```

**全部为否 = 新项目**。

## AI 执行步骤

### Step 1.1：执行探测命令

```bash
echo "=== 信号 1：README.md ==="
if [ -f "README.md" ]; then
  lines=$(wc -l < README.md)
  if [ $lines -gt 10 ]; then
    echo "README.md 存在，$lines 行（视为老项目）"
  else
    echo "README.md 存在但太短（$lines 行）"
  fi
else
  echo "无 README.md"
fi

echo ""
echo "=== 信号 2：docs/ 目录 ==="
if [ -d "docs" ]; then
  count=$(find docs -name "*.md" -o -name "*.docx" -o -name "*.pdf" 2>/dev/null | wc -l)
  echo "docs/ 存在，包含 $count 个文档"
else
  echo "无 docs/"
fi

echo ""
echo "=== 信号 3：wiki/confluence ==="
[ -d "wiki" ] && echo "wiki/ 存在" || echo "无 wiki/"
[ -d "confluence-export" ] && echo "confluence-export/ 存在" || echo "无 confluence-export/"

echo ""
echo "=== 信号 4：架构图 ==="
arch=$(find . -maxdepth 3 \( -name "*.drawio" -o -name "*architecture*.png" -o -name "*architecture*.svg" \) 2>/dev/null | head -3)
if [ -n "$arch" ]; then
  echo "找到架构图："
  echo "$arch"
else
  echo "无架构图"
fi

echo ""
echo "=== 信号 5：产品文档 ==="
[ -d "product-docs" ] && echo "product-docs/ 存在" || echo "无 product-docs/"
[ -d "prd" ] && echo "prd/ 存在" || echo "无 prd/"

echo ""
echo "=== 信号 6：Git 历史 ==="
if [ -d ".git" ]; then
  commits=$(git log --oneline 2>/dev/null | wc -l)
  echo "Git 存在，$commits 次提交"
  if [ $commits -gt 50 ]; then
    echo "提交 > 50，视为老项目"
  fi
else
  echo "无 .git"
fi
```

### Step 1.2：判定类型

| 判定结果 | 条件 |
|---------|------|
| **新项目** | 全部 6 个信号都为"否/不存在" |
| **老项目** | 任一信号为"是/存在" |

### Step 1.3：分流

- 新项目 → 跳到 **02-新项目路径**（见本文件末尾）
- 老项目 → 继续执行 `02-gather-materials.md`

## 新项目路径（特殊情况）

**新项目不需要收集材料**（无文档可读），直接走简化流程：

### Step 1-A：问 1 个问题

```markdown
## 质问 1/1：项目元信息

新项目需要确定基本信息，请选择/填写：

A. 项目一句话定位：[请补充]
B. 产品线代号：[请补充，如 oc / lms / cmd]
C. 业务领域：[请补充，如 订单 / 支付 / 物流]
D. 目标用户：[请补充，如 内部 / 外部 / 客户]
```

### Step 1-B：生成 context

读取 `templates/` 下 6 个模板，替换占位符：
- `{{PROJECT_NAME}}`：项目名
- `{{PROJECT_TAGLINE}}`：一句话定位
- `{{PRODUCT_LINE}}`：产品线代号
- `{{BUSINESS_DOMAIN}}`：业务领域
- `{{TARGET_USERS}}`：目标用户
- `{{COMPANY}}`：公司名

写入 `.qoder/context/`。

### Step 1-C：输出

```markdown
✅ 新项目 context 生成完毕

生成文件：
- .qoder/context/project-overview.md
- .qoder/context/architecture.md
- .qoder/context/module-responsibility.md
- .qoder/context/business-map-{productLine}.md
- .qoder/context/tech-stack.md
- .qoder/context/glossary.md

下一步建议：
1. 跑 rules-generator（如果还没跑）
2. 补充项目架构图、团队成员清单
3. 项目进展中持续更新 context
```

## 边缘情况处理

### 情况 1：项目是 Git 仓库但无任何文档
**判定**：老项目（Git 历史存在）
**处理**：仍按老项目流程走，材料收集会得到"无文档"结果，质问环节会告知"无材料可参考"

### 情况 2：项目有 README 但无其他文档
**判定**：老项目
**处理**：从 README 提取信息，其他 context 用默认模板 + 质问补充

### 情况 3：项目文档分散在多个目录
**判定**：老项目
**处理**：步骤 02 会汇总所有文档路径

### 情况 4：项目是 library/SDK 而非业务项目
**判定**：老项目
**处理**：business-map 文件可能较薄，可选生成

### 情况 5：context 文件已存在
**判定**：老项目
**处理**：备份现有 context 到 `.qoder/context/.backup-{时间戳}/`，再生成新文件

## 探针脚本（可独立运行）

```bash
#!/bin/bash
# 用法：./detect-context-project.sh

echo "================================"
echo "  context-generator 项目类型识别"
echo "================================"

signals=0

if [ -f "README.md" ] && [ $(wc -l < README.md) -gt 10 ]; then
  echo "✓ 信号 1：README.md 存在且 > 10 行"
  signals=$((signals+1))
fi

if [ -d "docs" ] && [ $(find docs -type f 2>/dev/null | wc -l) -gt 0 ]; then
  echo "✓ 信号 2：docs/ 存在且有内容"
  signals=$((signals+1))
fi

if [ -d "wiki" ] || [ -d "confluence-export" ]; then
  echo "✓ 信号 3：wiki/confluence 存在"
  signals=$((signals+1))
fi

if find . -maxdepth 3 \( -name "*.drawio" -o -name "*architecture*" \) -type f 2>/dev/null | head -1 | grep -q .; then
  echo "✓ 信号 4：架构图存在"
  signals=$((signals+1))
fi

if [ -d "product-docs" ] || [ -d "prd" ]; then
  echo "✓ 信号 5：产品文档存在"
  signals=$((signals+1))
fi

if [ -d ".git" ] && [ $(git log --oneline 2>/dev/null | wc -l) -gt 50 ]; then
  echo "✓ 信号 6：Git 提交 > 50"
  signals=$((signals+1))
fi

echo "================================"
if [ $signals -eq 0 ]; then
  echo "判定：新项目"
else
  echo "判定：老项目（命中 $signals 个信号）"
fi
echo "================================"
```

## 输出格式

执行完本步骤，AI 必须输出：

```markdown
## 项目类型识别结果

**类型**：新项目 / 老项目
**命中信号**：
- [✓/✗] README.md
- [✓/✗] docs/
- [✓/✗] wiki/
- [✓/✗] 架构图
- [✓/✗] 产品文档
- [✓/✗] Git 历史

**下一步**：
- 新项目 → 问 1 个问题 → 生成
- 老项目 → 进入 02-gather-materials.md
```

## 与 rules-generator 的差异

| 维度 | rules-generator | context-generator |
|------|----------------|-------------------|
| 判定信号 | 代码相关（Java 源、pom.xml） | 文档相关（README、wiki、架构图） |
| 关注 | "项目有代码吗" | "项目有文档吗" |
| 新项目比例 | 多数为老项目 | 新项目更多（context 起步晚） |
