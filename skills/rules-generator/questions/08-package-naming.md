# 质问 8/9：包名风格

## 扫描命令

```bash
echo "=== 包路径样本 ==="
find src -name "*.java" -not -path "*/target/*" | head -20 | xargs -I {} dirname {} | sort -u

echo ""
echo "=== 包路径深度统计 ==="
find src -name "*.java" -not -path "*/target/*" | \
  xargs -I {} dirname {} | \
  awk -F'/' '{print NF}' | sort | uniq -c
```

## 质问内容

```markdown
## 质问 8/9：包名风格

**扫描发现**（基于现有包路径推断）：

假设扫描到包路径 `cn.com.yto56.oc.order.api.controller`

- A. cn.com.{公司}.{业务线}.{模块}.{层}  （5 层，主流）  ← 推荐
- B. cn.com.{公司}.{业务线}.{模块}        （4 层，简化）
- C. cn.com.{业务线}.{模块}.{层}          （4 层，无公司）
- D. 其他（请说明）

**为什么 A 推荐**：
- 5 层结构清晰（公司/业务线/模块/层）
- 适合多公司、多业务线、多模块的大型项目
- 避免包名冲突
- Spring Boot 启动类扫描要写全

**方式 A 示例**：
```
cn.com.yto56.oc.order.api.controller   ← Controller 层
cn.com.yto56.oc.order.api.service       ← Service 接口
cn.com.yto56.oc.order.api.service.impl  ← Service 实现
cn.com.yto56.oc.order.api.mapper        ← Mapper
cn.com.yto56.oc.order.api.entity        ← 实体
cn.com.yto56.oc.common.enums            ← 公共枚举
cn.com.yto56.oc.common.constants        ← 公共常量
```

**方式 B 示例**（4 层，简化）：
```
cn.com.yto56.oc.order.controller
cn.com.yto56.oc.order.service
cn.com.yto56.oc.order.mapper
```

**方式 C 示例**（4 层，无公司前缀）：
```
cn.com.oc.order.controller
cn.com.oc.order.service
```

**启动类扫描包**（按 A 风格）：
```java
@SpringBootApplication(scanBasePackages = {
    "cn.com.yto56.oc",        // 业务线
    "cn.com.yto56.client"     // 客户端
})
public class OrderApplication {
    public static void main(String[] args) {
        SpringApplication.run(OrderApplication.class, args);
    }
}
```

请选择：
- [ ] A
- [ ] B
- [ ] C
- [ ] D
```

## 边界情况

### 情况 1：项目有多种包路径风格（混乱）
```markdown
⚠️ 项目存在多种包路径风格：
- cn.com.yto56.oc.order.controller
- cn.com.yto56.order.controller
- com.yto56.order.controller
建议统一为 A。
```

### 情况 2：项目是单模块
```markdown
💡 单模块项目可以用 B（4 层）简化
```

## 选 A 后的 rule 内容

```markdown
## 包路径

### 命名规则
- 格式：cn.com.{公司}.{业务线}.{模块}.{层}
- 公司：固定（如 yto56）
- 业务线：{2-3 字母}（如 oc、lms）
- 模块：{业务名}（如 order、user）
- 层：controller / service / service.impl / mapper / entity / dto / config / util

### 启动类
- @SpringBootApplication(scanBasePackages = {"cn.com.{公司}.{业务线}", "cn.com.{公司}.client"})

### 反例
- 缺公司前缀（如 com.order.controller）
- 缺业务线前缀（如 cn.com.yto56.order.controller，缺 oc）
- 层名错乱（如 .serviceimpl、.controllerlayer）
```

## 选 B 后的 rule 内容

```markdown
## 包路径
- 格式：cn.com.{公司}.{业务线}.{模块}
- 简化版（4 层）
- 适合单模块 / 小项目
```
