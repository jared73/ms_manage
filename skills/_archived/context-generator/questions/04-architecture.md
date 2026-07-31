# 质问 4/6：技术架构

## 收集证据

```bash
# 4.1 架构图
echo "=== 架构图 ==="
find . -maxdepth 4 \( -name "*.drawio" -o -name "*architecture*" \) 2>/dev/null | head -5

# 4.2 微服务数量（从 pom.xml 推断）
echo ""
echo "=== 微服务数量 ==="
apps=$(find . -name "*Application.java" 2>/dev/null | wc -l)
echo "Application 类数：$apps"

# 4.3 网关/注册中心
echo ""
echo "=== 网关/注册中心 ==="
grep -l "spring-cloud-starter-gateway\|spring-cloud-starter-alibaba-nacos" $(find . -name "pom.xml") 2>/dev/null

# 4.4 docker/k8s
echo ""
echo "=== 容器化 ==="
find . -maxdepth 3 \( -name "Dockerfile" -o -name "docker-compose*" -o -name "k8s*" \) 2>/dev/null | head -5
```

## 质问内容

```markdown
## 质问 4/6：技术架构

**现有证据**：
- Application 类数：5（说明有 5 个微服务）
- 有 Nacos 依赖（说明用 Nacos 做注册中心）
- 有 Dockerfile（说明容器化部署）
- 推断：**微服务架构**

**请确认**：

A. 微服务架构（5+ 个独立部署的服务）  ← 推荐
B. 单体应用（一个 jar 包含所有功能）
C. 模块化单体（一个 jar，但内部模块化）
D. 其他：______

**补充问题**（选 A 时必答）：
- 是否用 API 网关？[是/否]
- 是否用配置中心（Nacos/Apollo）？[是/否]
- 是否上 K8s？[是/否]
- 数据库是？[MySQL/Oracle/混合]
```

## 边界情况

### 情况 1：项目是 Serverless
```markdown
⚠️ 选 D，并说明"Serverless"
请补充：函数计算平台（AWS Lambda / 阿里云 FC / 腾讯云 SCF）
```

### 情况 2：项目是混合架构
```markdown
⚠️ 选 D，并说明"混合"
典型：核心服务微化 + 边缘功能单体
```

## 替换占位符

| 占位符 | 替换为 |
|--------|--------|
| `{{ARCHITECTURE_TYPE}}` | 微服务 / 单体 / 模块化单体 |
| `{{SERVICE_COUNT}}` | 服务数量 |
| `{{USE_GATEWAY}}` | 是 / 否 |
| `{{USE_CONFIG_CENTER}}` | 是 / 否 |
| `{{USE_K8S}}` | 是 / 否 |

## 选 A 后的 context 内容

```markdown
## 技术架构

### 整体架构
- 架构类型：**微服务**
- 服务数量：5+
- API 网关：是
- 配置中心：Nacos
- 部署方式：K8s

### 架构图
[待补充]

### 关键决策
- 决策 1：选 Nacos 而非 Eureka（团队熟悉 + 阿里系生态）
- 决策 2：服务间调用用 OpenFeign 而非 Dubbo
```

## 后续联动

```yaml
# 这个问题的答案会影响：
# 1. architecture.md 主体
# 2. tech-stack.md 中的部署相关章节
# 3. rules-generator 的"模块类型推断"
```
