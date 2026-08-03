# MS Manage 使用统计模块设计

日期：2026-08-03

## 背景

用户希望把 cc-switch（`farion1231/cc-switch`）的"使用统计"功能移植到 ms_manage。cc-switch 通过解析各 AI 编程工具自有的会话日志/SQLite 数据库，聚合出 token 用量与费用统计，无需用户在这些工具里配置任何东西。

移植目标：ms_manage 新增一个"使用统计"页面，展示本机 5 种 AI 工具（OpenCode / Claude Code / Codex / Gemini / Grok Build）的历史用量。

## 目标与范围

### 功能范围（全部）

- **5 个数据源**：OpenCode（SQLite）、Claude Code（JSONL）、Codex（JSONL）、Gemini（JSONL）、Grok Build（JSONL）
- **展示粒度**：总计、按应用汇总、按模型汇总、按天/周趋势、请求明细表、数据源状态
- **模型定价表**：内置常用模型单价 + 设置页可编辑
- **目录配置**：自动探测各工具标准目录 + 设置页可手动覆盖
- **同步**：启动时自动同步 + 每 60 秒增量检查 + 页面手动"立即同步"按钮

### 非目标

- 不做 cc-switch 的供应商切换/代理功能（ms_manage 已有自己的 proxy）
- 不统计 ms_manage 自身代理流量的请求日志（将来可扩展，本次不做）
- 不做费用分摊/多账户/团队统计

## 数据源

| 工具 | 路径 | 格式 |
|------|------|------|
| OpenCode | `~/.local/share/opencode/opencode.db` | SQLite（只读） |
| Claude Code | `~/.claude/projects/**/*.jsonl` | JSONL |
| Codex | `~/.codex/sessions/**/*.jsonl` | JSONL |
| Gemini | `~/.gemini/**/*.jsonl` | JSONL |
| Grok Build | `~/.grok/**/*.jsonl` | JSONL |

每个目录可通过 `settings.usage.overrides` 覆盖（`{ "opencode": "/path", ... }`）。

### OpenCode（SQLite）

只读打开 `opencode.db`，走 WAL 模式，需同时检查 `-wal` 文件 mtime。

```
session 表 → message 表 → data JSON（role=assistant 且有 tokens）
```

解析字段：`tokens.input / .output / .reasoning / .cache.read / .cache.write`、`cost`、`modelID`、`time.created / .completed`。仅统计已完成的 assistant 消息（`time.completed` 存在）。全零 token 消息跳过。opencode 的 `cost` 已有则直接使用；否则用定价表估算（output 含 reasoning，Anthropic 风格计费）。

### Claude Code / Codex / Gemini / Grok Build（JSONL）

遍历目录下 `*.jsonl`，按行解析，提取 assistant 消息的 token 使用数据。无自带费用时用定价表估算。每文件记录 `session_log_sync` 水位线（文件路径 + 最后修改时间 + 行偏移），实现增量同步。

## 架构

### 后端（Rust）

新增依赖：`rusqlite = { version = "0.32", features = ["bundled"] }`（编译含 SQLite，无外部依赖）。

```
src-tauri/src/usage/
  mod.rs                 // 模块声明 + sync_all() 入口 + 后台定时任务 spawn
  db.rs                  // SQLite 连接（data_dir/usage.db）、建表、索引、事务
  paths.rs               // 各工具目录探测 + 覆盖配置读取
  pricing.rs             // 内置模型定价表 + CostCalculator
  stats.rs               // 聚合查询：summary / by_app / by_model / trends / logs / data_sources
  session_opencode.rs    // OpenCode SQLite 解析 + 增量同步
  session_claude.rs      // Claude JSONL 解析
  session_codex.rs       // Codex JSONL 解析
  session_gemini.rs      // Gemini JSONL 解析
  session_grokbuild.rs   // Grok Build JSONL 解析
  session_common.rs      // 各 JSONL 解析共用：水位线、去重、插入
```

新增 `src-tauri/src/cmd_usage.rs`（IPC 命令，注册到 `invoke_handler`）。

### 数据库表（usage.db）

```sql
request_logs (
  request_id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,        -- 会话源标识，如 "_opencode_session"
  app_type TEXT NOT NULL,           -- opencode / claude / codex / gemini / grokbuild
  model TEXT NOT NULL,
  request_model TEXT,
  pricing_model TEXT,
  input_tokens INTEGER DEFAULT 0,
  output_tokens INTEGER DEFAULT 0,
  cache_read_tokens INTEGER DEFAULT 0,
  cache_creation_tokens INTEGER DEFAULT 0,
  input_cost_usd TEXT DEFAULT '0',      -- DECIMAL 用 TEXT 存，避免浮点误差
  output_cost_usd TEXT DEFAULT '0',
  cache_read_cost_usd TEXT DEFAULT '0',
  cache_creation_cost_usd TEXT DEFAULT '0',
  total_cost_usd TEXT DEFAULT '0',
  latency_ms INTEGER DEFAULT 0,
  status_code INTEGER DEFAULT 200,
  error_message TEXT,
  session_id TEXT,
  is_streaming INTEGER DEFAULT 1,
  cost_multiplier TEXT DEFAULT '1.0',
  created_at INTEGER NOT NULL,          -- unix 秒
  data_source TEXT DEFAULT 'session'    -- 来源：session / proxy
)

usage_daily_rollups (
  date TEXT NOT NULL,               -- YYYY-MM-DD
  app_type TEXT NOT NULL,
  model TEXT NOT NULL,
  request_count INTEGER DEFAULT 0,
  input_tokens INTEGER DEFAULT 0,
  output_tokens INTEGER DEFAULT 0,
  cache_read_tokens INTEGER DEFAULT 0,
  cache_creation_tokens INTEGER DEFAULT 0,
  total_cost_usd TEXT DEFAULT '0',
  PRIMARY KEY (date, app_type, model)
)

model_pricing (
  model_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  input_cost_per_million TEXT NOT NULL,
  output_cost_per_million TEXT NOT NULL,
  cache_read_cost_per_million TEXT DEFAULT '0',
  cache_creation_cost_per_million TEXT DEFAULT '0'
)

session_log_sync (
  file_path TEXT PRIMARY KEY,
  last_modified INTEGER NOT NULL,
  last_line_offset INTEGER DEFAULT 0,
  last_synced_at INTEGER NOT NULL
)
```

索引：`request_logs(created_at)`、`request_logs(app_type)`、`request_logs(model)`。

去重策略：`request_id` 主键 `INSERT OR IGNORE`。OpenCode 的 request_id 用 `opencode_session:{session_id}:{message_id}`，其余用 `{app}_session:{session_id}:{message_id}`。

### 同步流程

```
sync_all():
  for each source in [opencode, claude, codex, gemini, grokbuild]:
    if dir/file 不存在 → skip
    if 文件 mtime <= 上次同步 → skip
    打开读取 → 解析 → 插入 request_logs
    更新 session_log_sync 水位线
  更新 usage_daily_rollups（增量重算受影响日期）
```

- 后台任务：启动时 `tauri::async_runtime::spawn`，立即跑一次，之后每 60 秒检查。全局同步互斥（`tokio::sync::Mutex`），避免定时任务与手动同步并发。
- 手动同步：`sync_session_usage` 命令，前端"立即同步"按钮调用，返回 `SessionSyncResult`（imported/skipped/errors）。
- 定时任务只处理增量（文件未变则跳过）；失败不推进水位线，下次重试。

### IPC 命令（cmd_usage.rs）

| 命令 | 返回 | 说明 |
|------|------|------|
| `get_usage_summary` | `UsageSummary` | 总 token/费用/请求数/会话数，支持日期范围 |
| `get_usage_summary_by_app` | `UsageSummaryByApp[]` | 按应用汇总 |
| `get_usage_trends` | `DailyStats[]` | 按天/周聚合 |
| `get_model_stats` | `ModelStats[]` | 按模型汇总 |
| `get_request_logs` | `PaginatedLogs` | 明细表分页 |
| `sync_session_usage` | `SessionSyncResult` | 立即手动同步 |
| `get_usage_data_sources` | `DataSourceStatus[]` | 各数据源路径/是否存在/上次同步 |
| `get_model_pricing` / `update_model_pricing` / `update_model_pricing_batch` | - | 定价表管理 |

### 前端（React）

新增页面 `src/pages/Usage/index.tsx`，路由 `/usage`，侧边导航加"使用统计"入口（Layout 组件）。

```
src/pages/Usage/
  index.tsx            // 页面容器：日期范围 + 手动同步按钮
  UsageHero.tsx        // 汇总卡片：总 token / 总费用 / 请求数 / 会话数
  UsageTrendChart.tsx  // recharts 折线图（按天）
  ModelStatsTable.tsx  // 按模型表格
  AppSummaryCards.tsx  // 按应用（5 工具）卡片
  RequestLogTable.tsx  // 明细表 + 分页
  DataSourcesPanel.tsx // 数据源状态 + 路径覆盖配置入口
  PricingPanel.tsx     // 定价表管理（Modal 编辑）
```

- 图表库：**recharts**（`^3.5.1`，与 cc-switch 一致，需新增 npm 依赖）
- i18n：`src/i18n/zh.ts` / `en.ts` 增加 usage.* 键，`src/i18n/schema.ts` 同步
- IPC 封装：`src/lib/ipc.ts` 增加类型与 invoke 函数
- 日期范围：最近 7 天 / 30 天 / 90 天 / 全部，可切换
- 设置页（Settings.tsx）增加"数据源目录覆盖"配置区块

## 错误处理

- 数据源文件不存在/损坏：跳过该源，不阻塞其他源；结果记录到 `SessionSyncResult.errors`
- 单个消息解析失败：跳过该条，不中断整个会话
- 手动同步全程捕获错误，返回给前端提示
- 数据库操作失败：返回 `AppError::Database`，前端 toast 展示

## 测试

- **解析器单测**（`session_opencode.rs` / `session_common.rs` 内 `#[cfg(test)]`）：
  - OpenCode message JSON 解析（完整字段 / 缺 cache / 全零跳过 / 非 assistant / 未完成跳过）
  - JSONL 行解析（Claude/Codex/Gemini/Grok 变体）
  - 水位线推进逻辑
- **去重测试**：同一 request_id 重复插入不产生重复行
- **数据库测试**：建表、rollup 重算正确性
- 前端不写自动化测试（与现有项目一致，`src/lib/errors.test.ts` 例外）

## 实施顺序

1. 加依赖（rusqlite、recharts）→ 2. 建库表（db.rs）→ 3. 定价表 + CostCalculator（pricing.rs）
4. OpenCode 同步（会话数据最完整）→ 5. 其余 4 个 JSONL 同步 → 6. 聚合查询（stats.rs）
7. IPC 命令（cmd_usage.rs）→ 8. 前端页面 → 9. 设置页覆盖配置 → 10. 后台定时任务
