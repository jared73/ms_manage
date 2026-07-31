# Few-Shot 优质输入输出示例

---

## 示例一：应用管理 — API 测试

### TC-API-001: 应用列表 — 成功

**关联 AC-ID**: AC-001
**接口**: GET /api/applications?page=0&size=20

**Given**:
- 数据库中存在 2 条应用记录：
  - APP-801, name="测试应用", owner="张三", status=1
  - APP-802, name="生产应用", owner="李四", status=1
- 用户已登录（ADMIN 角色）

**When**: GET /api/applications?page=0&size=20

**Then**:
- code = 200
- data.content.length = 2
- data.totalElements = 2
- data.content[0].appId = "APP-801"
- data.content[0].secretKey = "******"（脱敏）

---

### TC-API-005: 创建应用 — 重复名称

**关联 EC-ID**: EC-005
**接口**: POST /api/applications

**Given**:
- 数据库中已存在 name="测试应用" 的记录

**When**: POST /api/applications body: { "name": "测试应用", "owner": "王五", "invokeType": 1 }

**Then**:
- code = 409
- message = "应用名称已存在"

---

### TC-API-020: 未授权访问

**Given**: 不携带 Authorization Header
**When**: GET /api/applications
**Then**: code = 401

---

### TC-API-030: 应用名称为空

**When**: POST /api/applications body: { "name": "", "owner": "张三", "invokeType": 1 }
**Then**: code = 400, message 包含"应用名称不能为空"

### TC-API-031: 应用名称超长

**When**: POST /api/applications body: { "name": "A".repeat(101), "owner": "张三", "invokeType": 1 }
**Then**: code = 400, message 包含"不能超过100字符"

---

## 示例二：词库管理 — E2E 测试

### TC-E2E-001: 词库管理完整流程

| 序号 | 操作 | 预期结果 |
| --- | --- | --- |
| 1 | 进入词库管理页面 | 显示词库列表 |
| 2 | 点击"创建词库" | 弹出创建表单 |
| 3 | 输入名称"辱骂黑名单"，类型选"黑名单"，提交 | 创建成功，列表出现新词库 |
| 4 | 点击词库名称进入词条管理 | 显示空词条列表 |
| 5 | 在批量添加框输入"词条1\n词条2\n词条3"，点击添加 | 成功添加 3 条，列表刷新 |
| 6 | 选中"词条2"，点击删除 | 二次确认后删除成功 |
| 7 | 点击批量添加，输入"词条1\n词条4" | 添加 1 条（词条1 去重），提示 addedCount=1 |
| 8 | 返回词库列表，确认词条数 = 3 | 列表中该词库 word_count = 3 |

---

## 示例三：仪表盘 — Mock 数据

```json
{
  "dashboard": {
    "statsResponse": {
      "code": 200,
      "data": {
        "todayCalls": 12345,
        "callGrowthRate": 12.5,
        "violationRate": 3.2,
        "todayViolations": 395,
        "tokenUsed": 1200000,
        "tokenCost": 12.0,
        "avgLatency": 45,
        "healthStatus": "healthy"
      }
    },
    "trendResponse": {
      "code": 200,
      "data": [
        { "date": "2026-05-12", "calls": 10000, "violations": 300 },
        { "date": "2026-05-13", "calls": 11000, "violations": 320 }
      ]
    }
  }
}
```