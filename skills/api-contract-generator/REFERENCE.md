# 技术参考与架构约束 (Reference)

---

## 项目已有 API 接口清单

以下接口已在项目中实现，生成 API_SPEC.md 时需标注"已实现 ✅"：

| 模块 | 接口路径 | 方法 | Controller |
|------|----------|------|-----------|
| 认证 | /api/auth/current | GET | AuthController |
| 认证 | /api/auth/logout | POST | AuthController |
| 应用 | /api/applications | GET/POST | ApplicationController |
| 应用 | /api/applications/{id} | GET/PUT/DELETE | ApplicationController |
| 服务 | /api/services | GET/POST | ServiceController |
| 服务 | /api/services/{id} | GET/PUT/DELETE | ServiceController |
| 词库 | /api/lexicons | GET/POST | LexiconController |
| 词库 | /api/lexicons/{id} | GET/PUT/DELETE | LexiconController |
| 词库 | /api/lexicons/{lexiconId}/entries | GET/POST | LexiconController |
| 词库 | /api/lexicons/{lexiconId}/entries/{entryId} | DELETE | LexiconController |
| 词库 | /api/lexicons/{lexiconId}/entries/batch | POST/DELETE | LexiconController |
| 日志 | /api/invocation-logs | GET | InvocationLogController |
| 仪表盘 | /api/dashboard/stats | GET | DashboardController |
| 仪表盘 | /api/dashboard/trend | GET | DashboardController |
| 仪表盘 | /api/dashboard/violation-source | GET | DashboardController |
| 仪表盘 | /api/dashboard/engine-efficiency | GET | DashboardController |
| 仪表盘 | /api/dashboard/trending-words | GET | DashboardController |
| 检测 | /api/detect | POST | DetectController (detection服务) |

---

## 项目已有数据库表清单

以下表已存在于数据库中，生成 DB_SCHEMA.md 时需标注"已有 ✅"：

| 表名 | 说明 | 关键字段 |
|------|------|---------|
| users | 用户表 | id, username, password, role, created_at, updated_at |
| applications | 应用表 | id, app_id, name, owner, invoke_type, status, secret_key, created_at, updated_at |
| services | 服务表 | id, name, detect_type, implement_type, status, config(JSON), created_at, updated_at |
| lexicons | 词库表 | id, name, type, creator, word_count, created_at, updated_at |
| lexicon_entries | 词条表 | id, lexicon_id(FK), word, created_at |
| app_services | 应用-服务关联表 | app_id(FK), service_id(FK) |
| invocation_logs | 调用日志表 | id, app_id, app_name, invoked_services, input, hit_service, result, latency_ms, token_used, invocation_time |
| trending_words | 热门变体词表 | id, word, tag, trend, description, first_discovered_at, is_blocked, created_at, updated_at |

---

## API 设计约束（摘自 rules）

### RESTful 路径规范

```
GET    /api/{resources}           — 列表查询（分页）
GET    /api/{resources}/{id}      — 详情查询
POST   /api/{resources}           — 创建
PUT    /api/{resources}/{id}      — 更新
DELETE /api/{resources}/{id}      — 删除
GET    /api/{resources}/{id}/{sub-resources}     — 子资源列表
POST   /api/{resources}/{id}/{sub-resources}     — 创建子资源
DELETE /api/{resources}/{id}/{sub-resources}/{subId} — 删除子资源
```

### 统一响应格式

```json
// 成功
{ "code": 200, "message": "success", "data": T, "timestamp": "..." }

// 失败
{ "code": xxx, "message": "错误描述", "data": null, "timestamp": "..." }
```

### 分页约定

- 请求：`page`（从 0 开始）+ `size`（默认 20，最大 100）
- 响应：Spring Data `Page<T>` 格式
  ```json
  { "content": [...], "totalElements": 100, "totalPages": 5, "number": 0 }
  ```

### 字段校验注解对照

| 校验规则 | 注解 | 常用场景 |
|---------|------|---------|
| 非空字符串 | @NotBlank | name, title |
| 非空值 | @NotNull | status, type |
| 最大长度 | @Size(max=N) | name≤100, description≤500 |
| 正数 | @Min(1) | invokeType |
| 范围 | @Min @Max | invokeType(1-2) |
| 集合大小 | @Size(max=N) | 列表最多 N 项 |
| 正则匹配 | @Pattern | ID 格式校验 |

---

## 数据库设计约束

### PostgreSQL 特有语法

- 自增主键：`BIGSERIAL` 或 `SERIAL`
- JSON 字段：`JSONB`（服务配置 config 字段）
- 时间字段：`TIMESTAMP` + `DEFAULT CURRENT_TIMESTAMP`
- 枚举值用 `INTEGER` 存储（不使用 PostgreSQL ENUM 类型）

### 命名规范

| 类别 | 规范 | 示例 |
|------|------|------|
| 表名 | snake_case，复数 | applications, invocation_logs |
| 字段名 | snake_case | secret_key, invoke_type |
| 主键 | id | id |
| 外键 | {表名单数}_id | lexicon_id, app_id |
| 时间 | {name}_at | created_at, updated_at |
| 布尔 | is_{形容词} | is_blocked |
| 索引 | idx_{表}_{字段} | idx_applications_status |
| 唯一索引 | uk_{表}_{字段} | uk_applications_app_id |