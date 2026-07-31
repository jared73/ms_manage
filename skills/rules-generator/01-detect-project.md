# 01 · 项目类型识别

> 目标：在做任何事之前，先判断这是新项目还是老项目。

## 判定逻辑

按以下顺序检查，**任一为真 = 老项目**：

```bash
# 信号 1：项目根有 src/main/java 且有 .java 文件
[ -d "src/main/java" ] && find src/main/java -name "*.java" | head -1

# 信号 2：pom.xml 中 modules 不为空
grep -A 10 "<modules>" pom.xml

# 信号 3：.spec/ 已存在
[ -d ".spec" ]

# 信号 4：.qoder/ 已存在
[ -d ".qoder" ]

# 信号 5：.git 存在且有 >10 个 commits
[ -d ".git" ] && git log --oneline | wc -l
```

**全部为否 = 新项目**。

## AI 执行步骤

### Step 1.1：执行探测命令

AI 在项目根目录执行以下命令，收集信号：

```bash
echo "=== 信号 1：Java 源码 ==="
if [ -d "src/main/java" ]; then
  java_count=$(find src/main/java -name "*.java" 2>/dev/null | wc -l)
  echo "存在 src/main/java，包含 $java_count 个 .java 文件"
else
  echo "不存在 src/main/java"
fi

echo ""
echo "=== 信号 2：Maven modules ==="
if [ -f "pom.xml" ]; then
  modules=$(grep -A 20 "<modules>" pom.xml | grep "<module>" | wc -l)
  if [ "$modules" -gt 0 ]; then
    echo "pom.xml 包含 $modules 个 module（多模块项目）"
  else
    echo "pom.xml 无 module（单模块项目）"
  fi
else
  echo "无 pom.xml"
fi

echo ""
echo "=== 信号 3：.spec/ ==="
[ -d ".spec" ] && echo ".spec/ 存在" || echo ".spec/ 不存在"

echo ""
echo "=== 信号 4：.qoder/ ==="
[ -d ".qoder" ] && echo ".qoder/ 存在" || echo ".qoder/ 不存在"

echo ""
echo "=== 信号 5：Git 历史 ==="
if [ -d ".git" ]; then
  commits=$(git log --oneline 2>/dev/null | wc -l)
  echo "Git 存在，$commits 次提交"
else
  echo "无 .git"
fi
```

### Step 1.2：判定类型

| 判定结果 | 条件 |
|---------|------|
| **新项目** | 全部 5 个信号都为"否/不存在" |
| **老项目** | 任一信号为"是/存在" |

### Step 1.3：分流

- 新项目 → 跳到 **02-新项目路径**（见本文件末尾）
- 老项目 → 继续执行 `02-scan-codebase.md`

## 新项目路径（特殊情况）

**新项目不需要扫描**（无代码可扫），直接走简化流程：

### Step 1-A：问 1 个问题

```markdown
## 质问 1/1：包名风格

新项目需要确定包路径前缀，请选择：

A. cn.com.{公司}.{业务线}.{模块}.{层}        ← 推荐
B. cn.com.{公司}.{业务线}.{模块}
C. cn.com.{业务线}.{模块}.{层}
D. 自定义（请说明）

例如选择 A，业务线是 order，模块是 api，层是 controller，
则包路径为：cn.com.{公司}.order.api.controller
```

### Step 1-B：生成 rule

读取 `templates/always-on.md.tpl` 等 4 个模板，替换占位符：
- `{{COMPANY}}`：公司名
- `{{PRODUCT_LINE}}`：业务线代号
- `{{PACKAGE_STYLE}}`：包名风格

写入 `.qoder/rules/`。

### Step 1-C：输出

```markdown
✅ 新项目 rule 生成完毕

生成文件：
- .qoder/rules/always-on.md
- .qoder/rules/java-class.md
- .qoder/rules/spec-reference.md
- .qoder/rules/pom-xml.md

下一步建议：
1. 跑 rules-checker 验证当前代码（虽然新项目可能没代码）
2. 跑 spec-generator 生成 .spec/（可选）— 已归档（2026-06-28），详见 `.qoder/skills/_archived/ARCHIVED.md`
```

## 边缘情况处理

### 情况 1：项目是 Git 仓库但无任何 Java 代码
**判定**：老项目（Git 历史存在）
**处理**：仍按老项目流程走，扫描会得到空结果，质问环节会告知"无类可扫"

### 情况 2：项目只有 src/test/java，没有 src/main/java
**判定**：老项目（有代码）
**处理**：扫描会得到"无 main 源码"结果，告知用户

### 情况 3：项目有 pom.xml 但无 modules 配置
**判定**：老项目
**处理**：单模块项目，正常扫描

### 情况 4：项目是多语言混合（Java + Python）
**判定**：老项目
**处理**：当前 skill 只处理 Java 部分，Python 部分需另适配

### 情况 5：项目是 library/SDK 而非业务项目
**判定**：老项目
**处理**：扫描结果可能很特殊（无 Controller/Service），质问环节会跳过不适用项

## 探针脚本（可独立运行）

把以下脚本保存为 `detect-project.sh`，可独立运行：

```bash
#!/bin/bash
# 用法：./detect-project.sh

echo "================================"
echo "  项目类型识别"
echo "================================"

signals=0

if [ -d "src/main/java" ] && [ "$(find src/main/java -name '*.java' 2>/dev/null | wc -l)" -gt 0 ]; then
  echo "✓ 信号 1：存在 Java 源码"
  signals=$((signals+1))
fi

if [ -f "pom.xml" ] && grep -q "<modules>" pom.xml; then
  echo "✓ 信号 2：多模块 Maven 项目"
  signals=$((signals+1))
fi

[ -d ".spec" ] && { echo "✓ 信号 3：.spec/ 存在"; signals=$((signals+1)); }
[ -d ".qoder" ] && { echo "✓ 信号 4：.qoder/ 存在"; signals=$((signals+1)); }

if [ -d ".git" ] && [ "$(git log --oneline 2>/dev/null | wc -l)" -gt 10 ]; then
  echo "✓ 信号 5：Git 提交 > 10"
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
- [✓/✗] Java 源码
- [✓/✗] Maven modules
- [✓/✗] .spec/
- [✓/✗] .qoder/
- [✓/✗] Git 历史

**下一步**：
- 新项目 → 问 1 个问题 → 生成
- 老项目 → 进入 02-scan-codebase.md
```
