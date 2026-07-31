# Few-Shot 优质输入输出示例 (Examples)

---

## 示例一：应用管理模块（app-management）

### 输入

```
Q3+ app-management
```

### 输出 — API_SPEC.md（节选）

```markdown
## 2. 接口详细定义

### 2.1 GET /api/applications — 应用列表

**请求参数**:

| 参数名 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | Integer | 否 | 0 | 页码（从 0 开始） |
| size | Integer | 否 | 20 | 每页条数（最大 100） |
| name | String | 否 | - | 应用名称模糊搜索 |
| owner | String | 否 | - | 所属人搜索 |

**响应结构**:

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "content": [
      {
        "id": 1,
        "appId": "APP-801",
        "name": "测试应用",
        "owner": "张三",
        "invokeType": 1,
        "status": 1,
        "services": [
          { "id": "SRV-001", "name": "辱骂检测-词库引擎" }
        ],
        "secretKey": "******",
        "createdAt": "2026-05-18T12:00:00",
        "updatedAt": "2026-05-18T12:00:00"
      }
    ],
    "totalElements": 50,
    "totalPages": 3,
    "number": 0
  }
}
```

**注意**：列表接口中 secretKey 字段脱敏为 `******`，仅在创建时返回完整值。

---

### 2.3 POST /api/applications — 创建应用

**请求体**:

```json
{
  "name": "新应用",
  "owner": "李四",
  "invokeType": 1,
  "serviceIds": ["SRV-001", "SRV-002"]
}
```

**请求字段校验**:

| 字段名 | 类型 | 必填 | 校验规则 | 说明 |
| --- | --- | --- | --- | --- |
| name | String | 是 | @NotBlank, @Size(max=100) | 应用名称，全局唯一 |
| owner | String | 是 | @NotBlank, @Size(max=50) | 所属人 |
| invokeType | Integer | 是 | @NotNull, @Min(1), @Max(2) | 调用方式：1=同步 2=异步 |
| serviceIds | List<String> | 否 | @Size(max=10) | 订阅的服务ID列表 |

**成功响应（含完整 SecretKey，仅此一次）**:

```json
{
  "code": 200,
  "message": "创建成功",
  "data": {
    "id": 1,
    "appId": "APP-A3F8K1M2",
    "name": "新应用",
    "owner": "李四",
    "invokeType": 1,
    "status": 0,
    "secretKey": "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
    "services": [...],
    "createdAt": "2026-05-18T12:00:00"
  }
}
```

**错误响应**:

| 状态码 | 错误消息 | 关联 EC-ID | 说明 |
| --- | --- | --- | --- |
| 400 | 参数校验失败 | EC-006 | name 为空或超长 |
| 409 | 应用名称已存在 | EC-005 | 重复创建同名应用 |
```

### 输出 — DB_SCHEMA.md（节选）

```markdown
### applications ✅ 已有

| 字段名 | 类型 | 约束 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | BIGSERIAL | PRIMARY KEY | - | 自增主键 |
| app_id | VARCHAR(20) | UNIQUE NOT NULL | - | 应用标识，格式 APP-XXXXXXXX |
| name | VARCHAR(100) | UNIQUE NOT NULL | - | 应用名称，全局唯一 |
| owner | VARCHAR(50) | NOT NULL | - | 所属人 |
| invoke_type | INTEGER | NOT NULL | 1 | 调用方式：1=同步 2=异步 |
| status | INTEGER | NOT NULL | 0 | 状态：0=接入中 1=正常 -1=停用 |
| secret_key | VARCHAR(64) | NOT NULL | - | 访问密钥（BCrypt 加密存储） |
| created_at | TIMESTAMP | NOT NULL | CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | NOT NULL | CURRENT_TIMESTAMP | 更新时间 |

**索引**:

| 索引名 | 类型 | 字段 | 说明 |
| --- | --- | --- | --- |
| uk_applications_app_id | UNIQUE | app_id | 应用标识唯一索引 |
| uk_applications_name | UNIQUE | name | 应用名称唯一索引 |
| idx_applications_status | BTREE | status | 状态查询索引 |

---

### app_services ✅ 已有（关联表）

| 字段名 | 类型 | 约束 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| app_id | BIGINT | FOREIGN KEY → applications(id) | - | 应用ID |
| service_id | BIGINT | FOREIGN KEY → services(id) | - | 服务ID |

**主键**: (app_id, service_id) 复合主键
```

---

## 示例二：词库管理模块（lexicon-management）— 子资源路由

词库管理的特殊之处在于词条是词库的子资源，API 路径需要体现层级关系。

### API_SPEC.md 中的特殊接口

```markdown
### 2.6 GET /api/lexicons/{lexiconId}/entries — 词条列表

**路径参数**:

| 参数名 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| lexiconId | Long | 是 | 词库ID |

**请求参数**:

| 参数名 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | Integer | 否 | 0 | 页码 |
| size | Integer | 否 | 20 | 每页条数 |
| keyword | String | 否 | - | 词条关键字搜索 |

---

### 2.8 POST /api/lexicons/{lexiconId}/entries/batch — 批量添加词条

**请求体**:

```json
{
  "words": ["词条1", "词条2", "词条3"]
}
```

**请求字段校验**:

| 字段名 | 类型 | 必填 | 校验规则 | 说明 |
| --- | --- | --- | --- | --- |
| words | List<String> | 是 | @NotEmpty, @Size(max=1000) | 词条数组，最多 1000 条 |
| words[] | String | 是 | @NotBlank, @Size(max=50) | 单个词条最长 50 字符 |

**成功响应**:

```json
{
  "code": 200,
  "message": "批量添加成功",
  "data": {
    "addedCount": 3,
    "duplicatedCount": 0
  }
}
```
```

---

## 示例三：仪表盘模块（dashboard）— 纯查询，无 CRUD

仪表盘模块只有 GET 接口，无创建/更新/删除。

### API_SPEC.md 中的接口总览

```markdown
| 序号 | 接口路径 | 方法 | 说明 | 关联 REQ-ID |
| --- | --- | --- | --- | --- |
| 1 | /api/dashboard/stats | GET | 核心指标 | REQ-001 |
| 2 | /api/dashboard/trend | GET | 趋势数据 | REQ-002 |
| 3 | /api/dashboard/violation-source | GET | 违规来源 | REQ-003 |
| 4 | /api/dashboard/engine-efficiency | GET | 引擎效能 | REQ-004 |
| 5 | /api/dashboard/trending-words | GET | 热门变体词 | REQ-005 |
```

### DB_SCHEMA.md — 无新增表

```markdown
# 仪表盘 数据库设计

> 仪表盘模块使用聚合查询，无专属业务表。
> 数据来源：invocation_logs（调用日志表）、services（服务表）、trending_words（热门变体词表）。

## 无新增表

仪表盘的统计指标均通过 DashboardRepository 对已有表的聚合查询实现，不需要新增表。
```