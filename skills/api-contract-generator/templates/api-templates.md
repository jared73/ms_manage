# API 契约与数据库模板集

---

## 模板一：API_SPEC.md 单模块段落

```markdown
# {模块中文名称} API 契约

> 模块标识: {module_name} | 版本: V1.0.0 | 创建日期: {当前日期}
> 关联 Spec: specs/{module}/01-overview.spec.md

---

## 1. 接口总览

| 序号 | 接口路径 | 方法 | 说明 | 关联 REQ-ID | 状态 |
| --- | --- | --- | --- | --- | --- |
| 1 | `/api/{resources}` | GET | 列表查询（分页） | REQ-001 | 已实现 ✅ / 待实现 |
| 2 | `/api/{resources}/{id}` | GET | 详情查询 | REQ-002 | 已实现 ✅ / 待实现 |
| 3 | `/api/{resources}` | POST | 创建 | REQ-003 | 已实现 ✅ / 待实现 |
| 4 | `/api/{resources}/{id}` | PUT | 更新 | REQ-004 | 已实现 ✅ / 待实现 |
| 5 | `/api/{resources}/{id}` | DELETE | 删除 | REQ-005 | 已实现 ✅ / 待实现 |

---

## 2. 接口详细定义

### 2.X {METHOD} {PATH} — {说明}

**请求参数** / **请求体** / **响应结构** / **错误响应**

（详细格式见 SKILL.md 中的模板）

---

## 3. DTO 定义

### Create{Resource}Request

| 字段 | 类型 | 校验 | 说明 |
| --- | --- | --- | --- |

### {Resource}VO

| 字段 | 类型 | 说明 |
| --- | --- | --- |

---

## 4. 状态码枚举

| 枚举名 | 值 | 说明 |
| --- | --- | --- |

---

## 5. 全局错误码

| 错误码 | 消息 | 说明 | 适用场景 |
| --- | --- | --- | --- |
```

---

## 模板二：DB_SCHEMA.md 单模块段落

```markdown
# {模块中文名称} 数据库设计

> 模块标识: {module_name} | 版本: V1.0.0 | 创建日期: {当前日期}
> 关联 Spec: specs/{module}/01-overview.spec.md

---

## 1. 表结构定义

### {table_name} ✅ 已有 / 新增

| 字段名 | 类型 | 约束 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |

**索引**:

| 索引名 | 类型 | 字段 | 说明 |
| --- | --- | --- | --- |

---

## 2. 关系定义

（Mermaid ER 图）

---

## 3. SQL DDL

（CREATE TABLE + CREATE INDEX）
```

---

## 填写注意事项

1. **已有接口/表标注"已实现 ✅"**，对照 REFERENCE.md 中的清单
2. **路径使用 kebab-case**：如 `/api/invocation-logs` 而非 `/api/invocationLogs`
3. **错误码必须标注关联 EC-ID**：与 04-edge-cases.md 一一对应
4. **DTO 校验规则必须具体**：注解 + 参数值（如 `@Size(max=100)`）
5. **数据库字段类型必须使用 PostgreSQL 语法**：如 `BIGSERIAL`、`JSONB`
6. **分页响应必须使用 Spring Data Page<T> 格式**
7. **所有命名必须遵循 glossary.context.md**（如"词条"用 entry 而非 word）