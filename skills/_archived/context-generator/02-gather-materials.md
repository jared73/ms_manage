# 02 · 收集项目材料

> 目标：从项目中提取 5 类材料作为 context 的事实依据。
>
> **原则**：context 关注"业务"不关注"实现"，所以**不读代码**。
> **只读文档**——README、设计文档、产品文档、架构图、术语表。

## 收集维度总览

| 维度 | 材料来源 | 用途 |
|------|---------|------|
| 1. 项目元信息 | README.md | 定位、目标、团队 |
| 2. 架构设计 | 架构图、技术博客 | 技术架构、模块关系 |
| 3. 产品地图 | 产品需求文档 | 产品线、模块清单 |
| 4. 业务术语 | 业务文档、代码注释 | 术语解释 |
| 5. 技术栈 | pom.xml（极少量） | 仅用于 tech-stack.md |

---

## 维度 1：项目元信息（来自 README.md）

### 收集命令

```bash
# 1.1 读取 README.md
if [ -f "README.md" ]; then
  echo "=== README.md 全文 ==="
  cat README.md
fi

# 1.2 读取 README 之外的项目说明
find . -maxdepth 2 -name "*.md" -not -path "*/node_modules/*" -not -path "*/target/*" 2>/dev/null | head -20

# 1.3 查找 CONTRIBUTING、CHANGELOG、CODE_OF_CONDUCT
for f in CONTRIBUTING.md CHANGELOG.md CODE_OF_CONDUCT.md LICENSE; do
  [ -f "$f" ] && echo "找到 $f"
done
```

### 输出格式

```markdown
## 项目元信息

### README.md 摘要
- 项目名称：
- 一句话定位：
- 主要功能：
- 目标用户：
- 当前版本：
- 维护团队：
- 相关链接：

### 其他 markdown 文件
- [文件路径]：[简述]
```

---

## 维度 2：架构设计

### 收集命令

```bash
# 2.1 查找架构图
echo "=== 架构图 ==="
find . -maxdepth 4 -type f \( -name "*.drawio" -o -name "*architecture*" -o -name "*.png" -o -name "*.svg" \) 2>/dev/null | head -10

# 2.2 查找架构相关文档
echo ""
echo "=== 架构相关文档 ==="
find . -maxdepth 3 -name "*.md" 2>/dev/null | xargs grep -l -i "架构\|architecture" 2>/dev/null | head -5

# 2.3 查找 ADR（架构决策记录）
echo ""
echo "=== ADR 文档 ==="
find . -name "ADR*" -o -name "*decision*" -o -name "doc/adr" -type d 2>/dev/null | head -5

# 2.4 查找部署相关
echo ""
echo "=== 部署相关 ==="
find . -maxdepth 3 -name "docker-compose*" -o -name "Dockerfile" -o -name "k8s*" -o -name "deploy*" 2>/dev/null | head -5
```

### 输出格式

```markdown
## 架构设计

### 架构图
- [文件路径]：[描述]
- 整体架构：（单体/微服务/Serverless/混合）
- 关键组件：（列举 3–5 个）

### 架构决策记录（ADR）
- [文件路径]：[决策标题]

### 部署方式
- Docker / K8s / 虚拟机 / Serverless
- 数据库：MySQL / Oracle / PostgreSQL
- 缓存：Redis / Memcached
- 消息队列：RocketMQ / Kafka / RabbitMQ
```

---

## 维度 3：产品地图

### 收集命令

```bash
# 3.1 查找产品文档目录
echo "=== 产品文档目录 ==="
[ -d "product-docs" ] && find product-docs -name "*.md" -o -name "*.docx" 2>/dev/null
[ -d "prd" ] && find prd -type f 2>/dev/null
[ -d "docs/product" ] && find docs/product -type f 2>/dev/null

# 3.2 查找需求/产品相关文档
echo ""
echo "=== 产品相关文档 ==="
find . -maxdepth 4 -name "*.md" 2>/dev/null | xargs grep -l -i "需求\|prd\|产品\|user story" 2>/dev/null | head -10

# 3.3 查找业务地图
echo ""
echo "=== 业务地图 ==="
find . -maxdepth 4 -name "*business*map*" -o -name "*业务地图*" 2>/dev/null

# 3.4 查找用户故事
echo ""
echo "=== 用户故事 ==="
find . -maxdepth 4 -name "specs" -type d 2>/dev/null
[ -d "specs" ] && find specs -name "*.md" 2>/dev/null | head -20
```

### 输出格式

```markdown
## 产品地图

### 产品线
- [产品线 A]：[定位、覆盖范围、关键能力]
- [产品线 B]：[定位、覆盖范围、关键能力]
- ...

### 核心模块
- [模块 A]：[职责]
- [模块 B]：[职责]
- ...

### 已有的 specs（如果存在）
- [specs/xxx/]：[模块名]
```

---

## 维度 4：业务术语

### 收集命令

```bash
# 4.1 查找术语表
echo "=== 术语表 ==="
find . -maxdepth 3 -name "glossary*" -o -name "术语*" -o -name "abbreviation*" 2>/dev/null

# 4.2 从代码注释提取（仅限类/方法注释）
echo ""
echo "=== 注释中可能的术语 ==="
grep -rh "^/\*\*\|@author\|@Description" src --include="*.java" 2>/dev/null | head -20

# 4.3 从 pom.xml 看产品线代号
echo ""
echo "=== 产品线代号（从 pom.xml 推断）==="
grep "<artifactId>" pom.xml 2>/dev/null | head -20

# 4.4 提取业务模块名
echo ""
echo "=== 业务模块（从模块名推断）==="
find . -maxdepth 3 -name "pom.xml" 2>/dev/null | xargs grep "<artifactId>" 2>/dev/null | head -20
```

### 输出格式

```markdown
## 业务术语

### 显式术语表
- [文件路径]：[术语表内容]

### 推断的术语
- [产品线代号] → [全称]（如 LMS → 物流管理系统）
- [模块名] → [业务含义]（如 oc-order-api → 订单中心对前端 API）
- [缩写] → [全称]

### 业务名词
- [业务名词 1]：[含义]
- [业务名词 2]：[含义]
```

---

## 维度 5：技术栈（仅用于 tech-stack.md）

### 收集命令

```bash
# 5.1 从 pom.xml 看依赖
echo "=== Java/Spring 版本 ==="
grep -A 1 "spring-boot-starter-parent\|spring-cloud-dependencies" pom.xml 2>/dev/null | head -10

# 5.2 关键依赖
echo ""
echo "=== 关键依赖 ==="
for dep in "spring-boot-starter-web" "spring-cloud-starter" "mybatis" "druid" "rocketmq" "kafka" "redis" "lombok" "hutool" "swagger" "springdoc"; do
  count=$(grep -c "$dep" $(find . -name "pom.xml" 2>/dev/null) 2>/dev/null | head -1)
  [ "$count" -gt 0 ] && echo "✓ $dep"
done

# 5.3 从 application.yml 看中间件
echo ""
echo "=== application.yml ==="
if [ -f "src/main/resources/application.yml" ]; then
  head -50 src/main/resources/application.yml
fi
```

### 输出格式

```markdown
## 技术栈

| 技术 | 版本 | 用途 |
|------|------|------|
| Java | 17 | 主语言 |
| Spring Boot | 3.2.0 | 主框架 |
| Spring Cloud | 2023.0.0 | 微服务 |
| MyBatis Plus | 3.5.5 | ORM |
| MySQL | 8.0 | 数据库 |
| Redis | 6.x | 缓存 |
| RocketMQ | 5.x | 消息队列 |
| Nacos | 2022.x | 注册中心 |
| Lombok | 1.18.30 | 代码简化 |
```

---

## 材料收集报告输出

AI 在所有 5 个维度收集完后，输出完整报告：

```markdown
# context-generator 材料收集报告

**项目**：{项目名}
**收集时间**：{时间}
**材料总数**：{N} 份

---

## 1. 项目元信息

（README.md 摘要 + 其他 markdown 文件清单）

## 2. 架构设计

（架构图 + 架构文档 + 部署方式）

## 3. 产品地图

（产品线 + 核心模块 + 已有 specs）

## 4. 业务术语

（术语表 + 推断术语）

## 5. 技术栈

（依赖清单）

---

## 初步观察

- **充分**：{列出材料齐全的}
- **不足**：{列出材料缺失的}
- **模糊**：{列出有矛盾/不清的}

将进入步骤 03 推断模式 + 步骤 04 质问。
```

## 注意事项

1. **不读代码**——只看文档（README、设计文档、架构图）
2. **少量读 pom.xml**——仅用于 tech-stack.md
3. **找不到的材料要标注**——不要瞎编
4. **材料要充分**——少于 3 份文档视为"材料不足"
5. **保留原文**——不要总结性提取，保留原始材料供后续参考

## 与 rules-generator 的差异

| 维度 | rules-generator | context-generator |
|------|----------------|-------------------|
| 读取对象 | 代码（*.java、pom.xml） | 文档（*.md、架构图、Confluence） |
| 不读 | .qoder/context/、README | 项目代码（除 pom.xml） |
| 收集速度 | 慢（要扫很多文件） | 快（只看几个文档） |
| 材料可读性 | 低（要解析） | 高（人话文档） |
