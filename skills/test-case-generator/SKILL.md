---
name: test-case-generator
description: |
  根据 Spec + API 契约，生成标准化的测试用例文档。
  当用户输入 "生成测试用例+" 后跟模块名时触发。
  自动生成 testing/{module}/ 下的测试场景和 Mock 数据
---

# 测试用例生成技能（qtest-case-generator）

## 触发条件

- `生成测试用例+ {module-name}`（例如：`生成测试用例+ app-management`）

## 前置依赖

- `specs/{module}/03-state-machine.md` — GWT 验收标准（测试场景的主要来源）
- `specs/{module}/04-edge-cases.md` — 异常场景（异常测试的主要来源）
- `docs/backend/API_SPEC.md` — 接口定义（请求参数、响应结构、错误码）

如果 spec 文件不存在，提示用户先使用 `生成spec+` 生成。

## 执行流程

---

### 第一步：读取依赖文件

1. 读取 `specs/{module}/03-state-machine.md` — 提取全部 AC（Given-When-Then）
2. 读取 `specs/{module}/04-edge-cases.md` — 提取全部 EC（异常场景）
3. 读取 `docs/backend/API_SPEC.md` — 接口定义、错误码、DTO 校验规则
4. 读取 `.qoder/rules/security.rules.md` — 安全测试约束
5. 读取 `.qoder/context/glossary.context.md` — 术语一致性

---

### 第二步：创建目录结构

在项目根目录下创建：

```
testing/
└── {module}/
    ├── api-test.md        — API 接口测试场景
    ├── e2e-test.md        — E2E 端到端测试场景
    └── fixtures/          — Mock 数据
        └── mock-data.json
```

---

### 第三步：生成测试文档

#### 3.1 生成 `api-test.md`

基于 GWT 验收标准 + 异常场景，生成 API 接口测试用例：

```markdown
# API 接口测试场景: {模块英文名称}

> 模块标识: {module_name} | 创建日期: {当前日期}
> 关联 Spec: specs/{module}/

---

## 1. 正向测试（Happy Path）

### TC-API-001: {接口名} — 成功场景

**关联 AC-ID**: AC-001
**关联 REQ-ID**: REQ-001
**接口**: GET /api/{resources}

**Given (前置条件)**:
- 数据库中存在至少 1 条{资源}记录
- 用户已登录，Token 有效

**When (操作)**:
- 发送 GET /api/{resources}?page=0&size=20

**Then (预期结果)**:
- HTTP 状态码 200
- 响应体 code = 200
- data.content 不为空
- data.totalElements > 0
- data.number = 0

**测试数据**:
```json
{
  "precondition": "INSERT INTO {table} (...) VALUES (...)"
}
```

---

## 2. 异常测试（Error Cases）

### TC-API-010: {接口名} — {异常场景}

**关联 EC-ID**: EC-005
**接口**: POST /api/{resources}

**Given (前置条件)**:
- 数据库中已存在 name="xxx" 的记录

**When (操作)**:
- 发送 POST /api/{resources} body: { "name": "xxx" }

**Then (预期结果)**:
- HTTP 状态码 200（业务错误）
- 响应体 code = 409
- 响应体 message = "名称已存在"

---

## 3. 安全校验测试

### TC-API-020: 未授权访问

**关联 EC-ID**: 全局
**接口**: GET /api/{resources}

**Given**: 不携带 Token
**When**: 发送请求
**Then**: code = 401, message 包含"未授权"

### TC-API-021: 权限不足

**Given**: 使用 OPERATOR 角色登录
**When**: 调用 DELETE 接口
**Then**: code = 403, message = "权限不足"

---

## 4. 参数校验测试

### TC-API-030: {字段} 为空

**接口**: POST /api/{resources}
**When**: body 中 {field} 为空/null
**Then**: code = 400, message 包含"{字段名}不能为空"

### TC-API-031: {字段} 超长

**When**: body 中 {field} 为 101 字符
**Then**: code = 400, message 包含"不能超过100字符"

---

## 5. 测试覆盖率矩阵

| AC-ID / EC-ID | 测试用例 ID | 是否覆盖 |
| --- | --- | --- |
| AC-001 | TC-API-001 | ✅ |
| AC-002 | TC-API-002 | ✅ |
| EC-001 | TC-API-010 | ✅ |
| EC-005 | TC-API-011 | ✅ |
```

**关键要求**：
- 每个正向测试关联 AC-ID，每个异常测试关联 EC-ID
- Given 中的测试数据必须具体（如具体的字段值、数据库记录）
- Then 中的断言必须可验证（如 code=200、data.content.length > 0）
- 覆盖率矩阵确保全部 AC 和 EC 都有对应测试

#### 3.2 生成 `e2e-test.md`

基于用户故事和业务流程，生成端到端测试场景：

```markdown
# E2E 端到端测试场景: {模块英文名称}

---

## 1. 核心业务流程

### TC-E2E-001: {业务流程名}

**关联 US-ID**: US-01
**前置条件**: 用户已登录（ADMIN 角色）

**步骤**:

| 序号 | 操作 | 预期结果 |
| --- | --- | --- |
| 1 | 进入{模块}页面 | 页面正常加载，显示列表 |
| 2 | 点击"创建"按钮 | 弹出创建表单 |
| 3 | 填写表单并提交 | 创建成功，列表刷新，新记录出现 |
| 4 | 点击"编辑"按钮 | 表单回填已有数据 |
| 5 | 修改并提交 | 更新成功，列表刷新 |
| 6 | 点击"删除"按钮 | 弹出确认框 |
| 7 | 确认删除 | 删除成功，记录消失 |

---

## 2. UI 状态流转

### TC-E2E-010: Loading → Success

**步骤**: 进入列表页 → 等待加载完成
**预期**: 加载动画 → 数据正常展示

### TC-E2E-011: Loading → Empty

**步骤**: 进入无数据的列表页
**预期**: 加载动画 → 空状态提示 + "创建"引导按钮

### TC-E2E-012: Loading → Error

**步骤**: 断开网络 → 进入列表页
**预期**: 加载动画 → 错误提示 + "重试"按钮

### TC-E2E-013: Error → Success（重试）

**步骤**: 错误状态 → 点击"重试" → 网络恢复
**预期**: 错误提示 → 加载 → 数据正常展示
```

#### 3.3 生成 `fixtures/mock-data.json`

基于 API_SPEC.md 中的 DTO 定义和 AC 中的 Given 数据，生成 Mock 数据：

```json
{
  "{module}": {
    "createRequest": {
      "name": "测试{资源}",
      "{field2}": "{value2}"
    },
    "response": {
      "id": 1,
      "name": "测试{资源}",
      "status": 1,
      "createdAt": "2026-05-18T12:00:00"
    },
    "listResponse": {
      "code": 200,
      "message": "success",
      "data": {
        "content": [],
        "totalElements": 0,
        "totalPages": 0,
        "number": 0
      }
    },
    "errorResponses": {
      "notFound": { "code": 404, "message": "{资源}不存在" },
      "conflict": { "code": 409, "message": "名称已存在" },
      "validation": { "code": 400, "message": "参数校验失败" }
    }
  }
}
```

---

### 第四步：生成完成 & 交付确认

```
✅ 测试用例文档已全部生成！

📂 目录结构：
testing/{module}/
├── api-test.md          ← API 接口测试场景
├── e2e-test.md          ← E2E 端到端测试场景
└── fixtures/
    └── mock-data.json   ← Mock 数据

📋 覆盖率摘要：
- 正向测试: {N} 条（覆盖全部 AC）
- 异常测试: {N} 条（覆盖全部 EC）
- 安全校验: {N} 条
- 参数校验: {N} 条
- E2E 场景: {N} 条

📋 下一步建议：
1. 确认覆盖率矩阵中全部 AC/EC 都已覆盖
2. 基于 api-test.md 编写自动化测试脚本
3. 基于 e2e-test.md 进行手动验收测试
```

---

## 重要约束

1. **AC/EC 全覆盖**：每个 AC 至少 1 条正向测试，每个 EC 至少 1 条异常测试
2. **Given 数据具体**：不能写"数据存在"，必须写具体的字段值
3. **Then 断言可验证**：必须包含具体的 code、message、data 字段值
4. **Mock 数据与 Given 一致**：fixtures 中的数据与 AC 中 Given 的数据值一致
5. **安全测试必须覆盖**：至少包含未授权(401)和权限不足(403)测试
6. **参数校验必须覆盖**：每个 @NotBlank/@NotNull/@Size 字段至少 1 条测试