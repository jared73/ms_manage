# 质问 6/6：项目成熟度

## 收集证据

```bash
# 6.1 Git 历史
echo "=== Git 历史 ==="
if [ -d ".git" ]; then
  total=$(git log --oneline 2>/dev/null | wc -l)
  first=$(git log --reverse --format="%ai" 2>/dev/null | head -1)
  echo "总提交数：$total"
  echo "首次提交：$first"
fi

# 6.2 团队规模（从 package-info 推断）
echo ""
echo "=== 作者统计 ==="
git log --format="%an" 2>/dev/null | sort -u | head -10

# 6.3 文档完整度
echo ""
echo "=== 文档完整度 ==="
docs=$(find . -maxdepth 3 -name "*.md" 2>/dev/null | wc -l)
echo "Markdown 文档数：$docs"

# 6.4 测试覆盖
echo ""
echo "=== 测试覆盖 ==="
tests=$(find src/test -name "*.java" 2>/dev/null | wc -l)
mains=$(find src/main -name "*.java" 2>/dev/null | wc -l)
echo "测试类：$tests / 主类：$mains"
```

## 质问内容

```markdown
## 质问 6/6：项目成熟度

**现有证据**：
- Git 提交数：2,345
- 首次提交：2020-03-15（已运行 4 年）
- 贡献者：12 人
- 文档数：45
- 测试类/主类比：0.3（覆盖率中等）

**请选择项目所处阶段**：

A. 稳定期（已上线 2+ 年，业务稳定，迭代放缓）  ← 推荐
B. 成长期（上线 1–2 年，业务快速迭代）
C. 起步期（< 1 年，MVP 阶段）
D. 重构期（业务转型中，技术栈调整）

**说明**：
- 不同阶段对应不同的 SDD 推广策略
- 稳定期：推广要慢，避免影响生产
- 成长期：推广要快，跟上业务
- 起步期：直接按 SDD 来
- 重构期：可以借机重写规范
```

## 边界情况

### 情况 1：项目跨多个阶段
```markdown
💡 可以选"主要阶段"，其他在 project-overview.md 中说明
```

### 情况 2：项目即将下线
```markdown
⚠️ 可以选 D（重构期），并标注"迁移至新系统"
```

## 替换占位符

| 占位符 | 替换为 |
|--------|--------|
| `{{MATURITY}}` | 稳定期 / 成长期 / 起步期 / 重构期 |
| `{{PROJECT_AGE}}` | 项目运行时间 |
| `{{CONTRIBUTOR_COUNT}}` | 贡献者数 |

## 选 A 后的 context 内容

```markdown
## 项目阶段

**当前阶段**：稳定期

**判断依据**：
- 已运行 4 年
- 提交数 2,345+
- 贡献者 12 人
- 业务趋于稳定

**SDD 推广策略**：
- 慢节奏，避免影响生产
- 优先做软引导（推荐、培训）
- 不强推硬规范
- 通过 code review 渐进合规
```

## 选 C 后的 context 内容

```markdown
## 项目阶段

**当前阶段**：起步期

**判断依据**：
- < 1 年
- 提交数 < 500
- 团队 3–5 人

**SDD 推广策略**：
- 直接按 SDD 来
- 一开始就建规范
- 工具优先（rule + context + spec）
- 培训 1–2 次即可
```

## 后续联动

```yaml
# 这个问题的答案会影响：
# 1. project-overview.md 中的"项目阶段"章节
# 2. SDD 推广策略
# 3. CI 强制程度（稳定期弱化，起步期强化）
```
