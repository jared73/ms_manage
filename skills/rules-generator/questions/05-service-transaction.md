# 质问 5/9：Service 事务注解

## 扫描命令

```bash
echo "=== @Transactional 分布 ==="
total=$(find src -name "*ServiceImpl.java" -o -name "*Impl.java" | wc -l)
class_level=$(grep -B5 "class.*Impl" $(find src -name "*Impl.java") | grep -c "^@Transactional$\|@Transactional(")
method_level=$(grep -A 2 "public" $(find src -name "*Impl.java") | grep -c "@Transactional")
none=$(find src -name "*Impl.java" | xargs grep -L "@Transactional" | wc -l)

echo "A. 类级 @Transactional：$class_level"
echo "B. 方法级 @Transactional：$method_level"
echo "C. 无 @Transactional：$none"
echo "总 Impl：$total"
```

## 质问内容

```markdown
## 质问 5/9：Service 事务注解

**扫描发现**：
- A. 类级 @Transactional（推荐粒度）  — {a} 个 ({a%}%)  ← 推荐
- B. 方法级 @Transactional（按需开启）— {b} 个 ({b%}%)
- C. 无 @Transactional（业务层不管事务）— {c} 个 ({c%}%)
- D. 其他（请说明）

**为什么 A 推荐**：
- 类级注解统一管理，所有 public 方法都自动开启事务
- 减少漏写 @Transactional 的风险
- 适合"业务方法都需要事务"的场景
- 读操作可以用 @Transactional(readOnly = true) 优化

**方式 A 示例**（类级）：
```java
@Service
@Transactional  // ← 类级：所有 public 方法自动开启事务
public class OrderServiceImpl implements OrderService {

    @Override
    public CreateOrderResp create(CreateOrderReq req) {
        // 自动有事务
    }

    @Override
    public OrderDetail getById(String id) {
        // 也自动有事务（可优化为 readOnly）
    }
}
```

**方式 B 示例**（方法级，按需）：
```java
@Service
public class OrderServiceImpl implements OrderService {

    @Override
    @Transactional  // ← 方法级：只有写方法有事务
    public CreateOrderResp create(CreateOrderReq req) {
        // 有事务
    }

    @Override
    public OrderDetail getById(String id) {
        // 无事务（读操作）
    }
}
```

**方式 C 示例**（不推荐）：
```java
@Service
public class OrderServiceImpl implements OrderService {

    @Override
    public CreateOrderResp create(CreateOrderReq req) {
        // 没有事务，调用方控制
        // 风险：调用方忘记开事务 → 数据不一致
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

### 情况 1：项目使用 @Transactional(rollbackFor = Exception.class)
```markdown
💡 提示：A 选中后，可以加 rollbackFor 配置：
@Transactional(rollbackFor = Exception.class)
默认只回滚 RuntimeException，加上这个回滚所有异常
```

### 情况 2：项目使用 Seata / 分布式事务
```markdown
⚠️ 项目使用 Seata @GlobalTransactional
事务规则在分布式事务规范中单独定义
```

## 选 A 后的 rule 内容

```markdown
## Service 事务
- ServiceImpl 类级必须 @Transactional
- 读操作建议 @Transactional(readOnly = true)
- 反例：Impl 类缺 @Transactional
- 特殊方法可方法级 @Transactional 覆盖
```

## 选 B 后的 rule 内容

```markdown
## Service 事务
- ServiceImpl 不加类级 @Transactional
- 写操作（增删改）必须方法级 @Transactional
- 读操作（查询）不加事务
- 反例：写操作漏 @Transactional
```

## 选 C 后的 rule 内容

```markdown
## Service 事务
- ServiceImpl 不强制事务注解
- 事务由调用方（Controller / 外部 Feign）控制
- 风险提示：调用方必须显式开启事务
```
