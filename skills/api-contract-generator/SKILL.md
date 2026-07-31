---
name: api-contract-generator
description: |
  根据 specs 目录下的产品规格文件，生成标准化的 API 契约和数据库设计文档。
  当用户输入 "生成API设计文档+" 后跟模块名，或指定 spec 目录路径时使用此技能。
  自动生成 docs/backend/ 下的 2 个标准文件：API_SPEC.md、DB_SCHEMA.md
---

# API 契约生成技能（api-contract-generator）

## 触发条件

当用户输入以下格式时触发：
- `生成API设计文档+ {module-name}`（例如：`生成API设计文档+ app-management`）
- `生成API设计文档+` + spec 目录路径

## 前置依赖

执行前必须确认以下文件存在：
- `specs/{module}/01-overview.spec.md` — 功能规格主文件（含 REQ-ID 清单）
- `specs/{module}/02-user-stories.md` — 用户故事（理解业务场景）
- `specs/{module}/04-edge-cases.md` — 异常场景（错误码定义依据）
- `specs/{module}/05-acceptance.md` — 状态机（实体状态定义依据）

如果 spec 文件不存在，提示用户先使用 `Q1+` 生成规格文件。

## 执行流程

---

### 第一步：读取规格文件 & 确认模块

#### 1.1 读取规格文件

按以下顺序读取 `specs/{module}/` 下的文件：
1. `01-overview.spec.md` — 提取功能需求清单（REQ-ID）、核心流程、非功能需求
2. `02-user-stories.md` — 理解用户视角和业务场景
3. `04-edge-cases.md` — 提取异常场景（EC-ID），定义错误码
4. `05-acceptance.md` — 提取实体状态枚举和状态流转

#### 1.2 确认模块名称

如果用户未指定模块名，使用 `ask_user_question` 工具列出 `specs/` 下已有的模块目录供用户选择。

---

### 第二步：读取 Rules 与 Context

- `.qoder/rules/backend.rules.md` — RESTful API 规范、统一响应格式、参数校验
- `.qoder/rules/security.rules.md` — 认证鉴权、越权校验
- `.qoder/rules/naming.rules.md` — API 路径命名、数据库字段命名
- `.qoder/context/architecture.context.md` — 技术栈、数据库表清单
- `.qoder/context/glossary.context.md` — 术语对照
- `.qoder/context/coding.context.md` — DTO 命名约定、分页约定

同时读取项目中已有的 Controller 代码，确认哪些 API 接口已经实现。

---

### 第三步：生成 `API_SPEC.md`

在 `docs/backend/API_SPEC.md` 中追加或创建对应模块的 API 契约。

如果 `docs/backend/` 目录不存在，先创建目录。

如果 `API_SPEC.md` 已存在且包含其他模块的契约，在文件末尾追加新模块的内容。

严格遵循以下模板：

```markdown
# {模块中文名称} API 契约

> 模块标识: {module_name} | 版本: V1.0.0 | 创建日期: {当前日期}
> 关联 Spec: specs/{module}/01-overview.spec.md

---

## 1. 接口总览

| 序号 | 接口路径 | 方法 | 说明 | 关联 REQ-ID |
| --- | --- | --- | --- | --- |
| 1 | `/api/{resources}` | GET | {资源}列表（分页） | REQ-001 |
| 2 | `/api/{resources}/{id}` | GET | {资源}详情 | REQ-002 |
| 3 | `/api/{resources}` | POST | 创建{资源} | REQ-003 |
| 4 | `/api/{resources}/{id}` | PUT | 更新{资源} | REQ-004 |
| 5 | `/api/{resources}/{id}` | DELETE | 删除{资源} | REQ-005 |

---

## 2. 接口详细定义

### 2.1 GET /api/{resources} — {资源}列表

**请求参数**:

| 参数名 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | Integer | 否 | 0 | 页码（从 0 开始） |
| size | Integer | 否 | 20 | 每页条数（最大 100） |
| {filterParam1} | String | 否 | - | {搜索条件说明} |
| {filterParam2} | String | 否 | - | {搜索条件说明} |

**响应结构**:

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "content": [
      {
        "id": "{id}",
        "{field1}": "{value1}",
        "{field2}": "{value2}",
        "status": {statusValue},
        "createdAt": "2026-05-18T12:00:00"
      }
    ],
    "totalElements": 100,
    "totalPages": 5,
    "number": 0
  },
  "timestamp": "2026-05-18T12:00:00"
}
```

**错误响应**:

| 状态码 | 错误消息 | 关联 EC-ID | 说明 |
| --- | --- | --- | --- |
| 400 | 参数校验失败 | EC-006 | page/size 参数不合法 |

---

### 2.2 POST /api/{resources} — 创建{资源}

**请求体**:

```json
{
  "{field1}": "{value1}",
  "{field2}": "{value2}",
  "{relationList}": ["{id1}", "{id2}"]
}
```

**请求字段校验**:

| 字段名 | 类型 | 必填 | 校验规则 | 说明 |
| --- | --- | --- | --- | --- |
| {field1} | String | 是 | @NotBlank, @Size(max=100) | {说明} |
| {field2} | String | 是 | @NotNull | {说明} |

**成功响应**:

```json
{
  "code": 200,
  "message": "创建成功",
  "data": {
    "id": "{生成的ID}",
    "{field1}": "{value1}",
    "createdAt": "2026-05-18T12:00:00"
  }
}
```

**错误响应**:

| 状态码 | 错误消息 | 关联 EC-ID | 说明 |
| --- | --- | --- | --- |
| 400 | 参数校验失败 | EC-006 | 字段不满足校验规则 |
| 409 | 名称已存在 | EC-005 | 重复创建 |

---

## 3. DTO 定义

### Create{Resource}Request

| 字段 | 类型 | 校验 | 说明 |
| --- | --- | --- | --- |
| {field} | {type} | {validation} | {说明} |

### {Resource}VO

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| id | String/Long | {ID说明} |
| {field} | {type} | {说明} |
| createdAt | LocalDateTime | 创建时间 |
| updatedAt | LocalDateTime | 更新时间 |

---

## 4. 状态码枚举

| 枚举名 | 值 | 说明 |
| --- | --- | --- |
| {EnumValue1} | {code} | {说明} |
| {EnumValue2} | {code} | {说明} |

---

## 5. 全局错误码

| 错误码 | 消息 | 说明 | 适用场景 |
| --- | --- | --- | --- |
| 200 | success | 成功 | 全部接口 |
| 400 | 参数校验失败 | 请求参数不合法 | 全部接口 |
| 401 | 未授权/Token无效 | JWT 校验失败 | 需鉴权接口 |
| 403 | 权限不足 | 角色权限不足 | 需特定角色接口 |
| 404 | {资源}不存在 | 资源ID不存在 | 详情/更新/删除 |
| 409 | 名称已存在 | 重复创建 | 创建接口 |
| 500 | 服务器内部错误 | 未预期异常 | 全部接口 |
```

**关键要求**：
- 接口路径必须遵循 RESTful 规范（对照 backend.rules.md R-BE-005）
- 路径使用 kebab-case（对照 naming.rules.md R-NM-005）
- 错误码必须与 04-edge-cases.md 中的 EC-ID 对应
- DTO 字段校验必须具体（类型 + 注解 + 长度限制）
- 如果该模块的 Controller 已存在于项目中，标注接口状态为"已实现 ✅"

---

### 第四步：生成 `DB_SCHEMA.md`

在 `docs/backend/DB_SCHEMA.md` 中追加或创建对应模块的数据库设计。

如果 `DB_SCHEMA.md` 已存在，在文件末尾追加新模块内容。

严格遵循以下模板：

```markdown
# {模块中文名称} 数据库设计

> 模块标识: {module_name} | 版本: V1.0.0 | 创建日期: {当前日期}
> 关联 Spec: specs/{module}/01-overview.spec.md

---

## 1. 表结构定义

### {table_name}

| 字段名 | 类型 | 约束 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | {BIGINT/SERIAL/VARCHAR} | PRIMARY KEY | - | {主键说明} |
| {field} | {type} | {constraint} | {default} | {说明} |
| status | INTEGER | NOT NULL | {defaultStatus} | 状态：{-1=停用, 0=接入中, 1=正常} |
| created_at | TIMESTAMP | NOT NULL | CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | NOT NULL | CURRENT_TIMESTAMP | 更新时间 |

**索引**:

| 索引名 | 类型 | 字段 | 说明 |
| --- | --- | --- | --- |
| idx_{table}_{field} | {BTREE/UNIQUE} | {field} | {说明} |

---

## 2. 关系定义

```mermaid
erDiagram
    {TABLE_A} ||--o{ {TABLE_B} : "1:N 关系说明"
    {TABLE_A} ||--o{ {TABLE_C} : "关联表说明"
```

---

## 3. SQL DDL

```sql
-- {模块中文名称} 相关表
CREATE TABLE IF NOT EXISTS {table_name} (
    id {type} PRIMARY KEY,
    {field} {type} {constraint},
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_{table}_{field} ON {table_name} ({field});
```
```

**关键要求**：
- 表名使用 snake_case（对照 naming.rules.md R-NM-004）
- 字段名使用 snake_case
- 外键使用 `{referenced_table_singular}_id`
- 状态字段使用 Integer 编码（-1/0/1）
- 如果该模块的表已存在于数据库中（对照 architecture.context.md），标注为"已有 ✅"并在 DDL 中注明"无需新增"

---

### 第五步：生成完成 & 交付确认

```
✅ API 契约文档已全部生成！

📂 目录结构：
docs/backend/
├── API_SPEC.md     ← API 契约（接口定义 + DTO + 错误码）
└── DB_SCHEMA.md    ← 数据库设计（表结构 + 关系 + DDL）

📋 下一步建议：
1. 检查 API_SPEC.md 中的接口路径是否遵循 RESTful 规范
2. 确认 DB_SCHEMA.md 中的表名和字段名符合项目命名规范
3. 验证错误码与 04-edge-cases.md 的异常场景一一对应
4. 确认已有接口标注了"已实现 ✅"
5. 使用 Q4+ 或 Q5+ 开始生成前端/后端代码
```

---

## 重要约束

1. **RESTful 规范**：路径使用 kebab-case，操作映射 GET/POST/PUT/DELETE
2. **错误码对应**：每个错误响应必须标注关联的 EC-ID
3. **已有接口标注**：如果 Controller 中已存在该接口，标注"已实现 ✅"
4. **术语一致**：所有命名必须遵循 glossary.context.md 术语表
5. **分页约定**：page 从 0 开始，size 默认 20 最大 100
6. **响应格式**：统一使用 ApiResponseReturn<T> 包装