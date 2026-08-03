/**
 * 与 Rust 后端的 IPC 契约（类型与函数）。
 * 每个 invoke 名都对应 src-tauri/src/cmd_*.rs 中 #[tauri::command] 的同名函数。
 */
import { invoke } from "@tauri-apps/api/core";

export type Transport = "stdio" | "sse" | "http";

export interface McpServer {
  id: string;
  name: string;
  description?: string;
  transport: Transport;
  // stdio
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
  // sse / http
  url?: string;
  headers?: Record<string, string>;
  enabled: boolean;
  auto_start?: boolean;
  tags?: string[];
  project_id?: string | null;
  disabled_tools?: string[];
  created_at: string;
  updated_at: string;
  // 预安装状态（包管理）
  installed?: boolean;
  installed_version?: string | null;
  installed_path?: string | null;
  installed_at?: string | null;
  install_error?: string | null;
  // 后端派生：command 是否是 npx/uvx 等包管理器（判定收敛在后端 detect_pkg）
  needs_install?: boolean;
}

export interface McpTool {
  name: string;
  description?: string;
  inputSchema?: unknown;
}

export interface Project {
  id: string;
  name: string;
  description?: string;
  created_at: string;
  updated_at: string;
}

export type ServerStatus = "stopped" | "starting" | "running" | "error" | "disabled";

export interface ServerState {
  server: McpServer;
  status: ServerStatus;
  pid?: number;
  last_error?: string;
  started_at?: string;
  last_active_at?: string;
  tools_count?: number;
}

export interface Skill {
  id: string;
  name: string;
  description?: string;
  content: string;            // markdown 原文
  frontmatter: Record<string, unknown>;
  dir_path: string;           // skill 目录绝对路径（对应 Rust StoredSkill.dir_path）
  enabled: boolean;
  created_at: string;
  updated_at: string;
  files?: string[];           // skill 目录下相对文件列表
}

export interface LogEntry {
  id: string;
  timestamp: string;
  level: "debug" | "info" | "warn" | "error";
  source: string;             // "proxy" | "server:<id>" | "skill" | "system"
  message: string;
  detail?: string;
}

export interface ProxyStatus {
  running: boolean;
  endpoint?: string;          // e.g. "stdio://" 或 "http://127.0.0.1:39231/mcp"
  connected_clients: number;
  total_requests: number;
  started_at?: string;
}

export interface MarketItem {
  id: string;
  kind: "server" | "skill";
  name: string;
  description: string;
  author?: string;
  tags?: string[];
  installs: number;
  updated_at: string;
  source_url?: string;  // skill 来自 GitHub 时带这个
}

export interface AppSettings {
  language: "zh" | "en";
  theme: "purple" | "midnight" | "forest" | "light";
  auto_launch: boolean;
  proxy_mode: "stdio" | "http";
  proxy_http_port?: number;
  registry_url?: string;
  logs_max_entries: number;
  auto_start_proxy?: boolean;
  npm_registry?: string;
  auto_inject_npm_registry?: boolean;
  /** 永远只是掩码版(如 `mcpr_***1234`)。要看真实 token,调 `revealToken()`。 */
  mcp_token?: string;
  /** true 表示已生成 token(此时 mcp_token 字段是有值的掩码);false 表示还没生成。 */
  mcp_token_present?: boolean;
  /** github_token 也是掩码版。 */
  github_token?: string;
  /** 使用统计各数据源目录覆盖（key: opencode/claude/codex/gemini/grokbuild）。 */
  usage_overrides?: Record<string, string>;
}

// ---- MCP Server ----
export const listServers = () => invoke<McpServer[]>("list_servers");
export const getServerState = (id: string) => invoke<ServerState>("get_server_state", { id });
export const listServerStates = () => invoke<ServerState[]>("list_server_states");
export const upsertServer = (server: McpServer) => invoke<McpServer>("upsert_server", { server });
export const deleteServer = (id: string) => invoke<void>("delete_server", { id });
export const startServer = (id: string, setAutoStart?: boolean) =>
  invoke<void>("start_server", { id, setAutoStart: setAutoStart ?? null });
export const stopServer = (id: string, setAutoStart?: boolean) =>
  invoke<void>("stop_server", { id, setAutoStart: setAutoStart ?? null });
export const testServer = (id: string) => invoke<{ ok: boolean; latency_ms?: number; tools?: unknown[]; error?: string }>("test_server", { id });
export const fetchServerTools = (id: string) => invoke<{ name: string; description?: string; inputSchema?: unknown }[]>("fetch_server_tools", { id });
export const setToolEnabled = (id: string, toolName: string, enabled: boolean) => invoke<void>("set_tool_enabled", { id, toolName, enabled });

// ---- 安装 / 卸载（预装包到本地）----
export interface InstallProgressEvent {
  id: string;
  stage: "checking" | "installing" | "resolving" | "done" | "error";
  message: string;
}
export const installServer = (id: string) => invoke<void>("install_server", { id });
export const uninstallServer = (id: string) => invoke<void>("uninstall_server", { id });
export const refreshInstallStatus = (id: string) => invoke<McpServer>("refresh_install_status", { id });

// ---- Project ----
export const listProjects = () => invoke<Project[]>("list_projects");
export const upsertProject = (project: Project) => invoke<Project>("upsert_project", { project });
export const deleteProject = (id: string) => invoke<void>("delete_project", { id });

// ---- Skill ----
export const listSkills = () => invoke<Skill[]>("list_skills");
export const getSkill = (id: string) => invoke<Skill>("get_skill", { id });
export const upsertSkill = (skill: Skill) => invoke<Skill>("upsert_skill", { skill });
export const deleteSkill = (id: string) => invoke<void>("delete_skill", { id });
export const toggleSkill = (id: string, enabled: boolean) => invoke<void>("toggle_skill", { id, enabled });

/**
 * 把 SKILL.md 整段交给后端,用 serde_yaml 解析 frontmatter。
 * 返回 { front: Record<string, unknown>, name, description, body }。
 * 之前用 JS regex 解析时遇到 "Use this: for code review" 这种含 `:` 的描述会被截断。
 */
export interface ParsedFrontmatter {
  front: Record<string, unknown>;
  name: string | null;
  description: string | null;
  body: string;
}
export const parseSkillFrontmatter = (content: string) =>
  invoke<ParsedFrontmatter>("parse_skill_frontmatter", { content });

// ---- Skill 集成目标（mcp-router 风格：每个编辑器一个目标）----
export interface SkillTarget {
  id: string;
  name: string;
  path: string;
  enabled: boolean;
  builtin: boolean;
}
export const listSkillTargets = () => invoke<SkillTarget[]>("list_skill_targets");
export const toggleSkillTarget = (id: string, enabled: boolean) => invoke<void>("toggle_skill_target", { id, enabled });
export const addSkillTarget = (name: string, path: string) => invoke<SkillTarget>("add_skill_target", { name, path });
export const removeSkillTarget = (id: string) => invoke<void>("remove_skill_target", { id });
export const updateSkillTarget = (id: string, name?: string, path?: string) => invoke<void>("update_skill_target", { id, name, path });

export interface ImportSkillResult {
  imported: string[];
  errors: string[];
  skipped: string[];
}
export const importSkillFromPath = (path: string) => invoke<ImportSkillResult>("import_skill_from_path", { path });

// ---- 全局记忆文件 ----
export interface Memory {
  id: string;
  name: string;
  description?: string;
  target_path: string;         // 工具真实读取的绝对路径
  enabled: boolean;
  builtin: boolean;
  content: string;             // 主副本 Markdown
  created_at: string;
  updated_at: string;
  // 派生：同步状态（后端实时探测）
  target_exists?: boolean;
  in_sync?: boolean;
  target_mtime?: string | null;
}
export const listMemories = () => invoke<Memory[]>("list_memories");
export const getMemory = (id: string) => invoke<Memory>("get_memory", { id });
export const upsertMemory = (memory: Memory) => invoke<Memory>("upsert_memory", { memory });
export const deleteMemory = (id: string) => invoke<void>("delete_memory", { id });
export const toggleMemory = (id: string, enabled: boolean) => invoke<Memory>("toggle_memory", { id, enabled });
export const syncMemoryToTarget = (id: string) => invoke<Memory>("sync_memory_to_target", { id });
export const pullMemoryFromTarget = (id: string) => invoke<Memory>("pull_memory_from_target", { id });
export const openMemoryTargetDir = (id: string) => invoke<void>("open_memory_target_dir", { id });

// ---- Proxy ----
export const getProxyStatus = () => invoke<ProxyStatus & { last_request_at?: string }>("get_proxy_status");
export const getRouterConfig = () => invoke<{
  endpoint: string;
  host: string;
  port: number;
  token: string;
  running: boolean;
  mcpServers: Record<string, unknown>;
  instructions: string;
}>("get_router_config");
export const getToolsTotal = () => invoke<number>("get_tools_total");
export const getExposedTools = () => invoke<{ name: string; description?: string; __server: string }[]>("get_exposed_tools");

// 写一条日志（用户/前端想记事件时用）
export const logEvent = (
  level: "info" | "warn" | "error" | "debug",
  source: string,
  message: string,
  detail?: string,
) => invoke<void>("log_event", { level, source, message, detail });
export const startProxy = () => invoke<ProxyStatus>("start_proxy");
export const stopProxy = () => invoke<void>("stop_proxy");

// ---- Market ----
// Tauri 2 默认 camelCase: Rust 端 `force_refresh` / `source_url` 期望 JS 端发送 `forceRefresh` / `sourceUrl`。
// 之前用 `force_refresh` / `source_url` 在 v2 下被静默忽略 —— 参数取到默认值或 None,功能失效但无报错。
export const fetchMarket = (kind?: "server" | "skill", forceRefresh?: boolean) =>
  invoke<MarketItem[]>("fetch_market", { kind, forceRefresh: forceRefresh ?? false });
export const installMarketItem = (id: string, kind: string, sourceUrl?: string) =>
  invoke<void>("install_market_item", { id, kind, sourceUrl });

// ---- Logs ----
export const listLogs = (opts?: {
  limit?: number;
  /** 精确匹配 - 保留向后兼容。优先使用 level_min。 */
  level?: string;
  /** 最低级别:debug=0/info=1/warn=2/error=3。后端会自动包含更高级别。 */
  level_min?: string;
  source?: string;
}) =>
  invoke<LogEntry[]>("list_logs", { opts: opts ?? {} });
export const clearLogs = () => invoke<void>("clear_logs");

// ---- Settings ----
export const getSettings = () => invoke<AppSettings>("get_settings");
export const updateSettings = (settings: AppSettings) => invoke<void>("update_settings", { settings });
export const regenerateToken = () => invoke<{ token: string }>("regenerate_token");
export const revealToken = () => invoke<string>("reveal_token");
export const rewriteClientConfigs = (useNpx: boolean = true) => invoke<string[]>("rewrite_client_configs", { useNpx });
export const enableAutostart = () => invoke<void>("enable_autostart");
export const disableAutostart = () => invoke<void>("disable_autostart");
export const isAutostartEnabled = () => invoke<boolean>("is_autostart_enabled");

export const getAutoLaunch = () => invoke<boolean>("get_auto_launch");
export const setAutoLaunch = (enabled: boolean) => invoke<void>("set_auto_launch", { enabled });

// ---- App info ----
export const appInfo = () => invoke<{ version: string; data_dir: string; log_dir: string; app_dir: string }>("app_info");
export const openExternal = (url: string) => invoke<void>("open_external", { url });
export const openDataDir = () => invoke<void>("open_data_dir");

// 一键体检
export interface DoctorItem {
  id: string;
  title: string;
  status: "ok" | "warn" | "fail";
  detail: string;
  suggestion?: string;
}
export const doctorCheck = () => invoke<DoctorItem[]>("doctor_check");

// ---- Usage stats ----
export interface UsageSummary {
  requestCount: number;
  totalCost: string;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheCreationTokens: number;
  totalTokens: number;
  cacheHitRate: number;
}

export interface UsageSummaryByApp {
  appType: string;
  requestCount: number;
  totalCost: string;
  totalTokens: number;
}

export interface DailyStats {
  date: string;
  requestCount: number;
  totalCost: string;
  totalTokens: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheCreationTokens: number;
}

export interface ModelStats {
  model: string;
  requestCount: number;
  totalTokens: number;
  totalCost: string;
}

export interface DataSourceStatus {
  appType: string;
  label: string;
  path: string;
  exists: boolean;
}

export interface SessionSyncResult {
  imported: number;
  skipped: number;
  filesScanned: number;
  suspectedDuplicates: number;
  deferredFiles: number;
  errors: string[];
}

export interface ModelPricingInput {
  modelId: string;
  displayName?: string;
  inputCostPerMillion: string;
  outputCostPerMillion: string;
  cacheReadCostPerMillion: string;
  cacheCreationCostPerMillion: string;
}

export const getUsageSummary = (
  startDate?: number,
  endDate?: number,
  appType?: string,
) => invoke<UsageSummary>("get_usage_summary", { startDate, endDate, appType });
export const getUsageSummaryByApp = (startDate?: number, endDate?: number) =>
  invoke<UsageSummaryByApp[]>("get_usage_summary_by_app", { startDate, endDate });
export const getUsageTrends = (startDate?: number, endDate?: number, grouping?: string) =>
  invoke<DailyStats[]>("get_usage_trends", { startDate, endDate, grouping });
export const getModelStats = (startDate?: number, endDate?: number) =>
  invoke<ModelStats[]>("get_model_stats", { startDate, endDate });
export const getUsageDataSources = () => invoke<DataSourceStatus[]>("get_usage_data_sources");
export const syncSessionUsage = () => invoke<SessionSyncResult>("sync_session_usage");
export const getModelPricing = () => invoke<ModelPricingInput[]>("get_model_pricing");
export const updateModelPricingBatch = (items: ModelPricingInput[]) =>
  invoke<void>("update_model_pricing_batch", { items });

export interface RequestLogRow {
  requestId: string;
  appType: string;
  model: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalCostUsd: string;
  createdAt: number;
  sessionId?: string;
}

export interface PaginatedLogs {
  rows: RequestLogRow[];
  total: number;
  page: number;
  pageSize: number;
}

export interface RequestLogFilters {
  startDate?: number;
  endDate?: number;
  appType?: string;
  model?: string;
  page?: number;
  pageSize?: number;
}

export const getRequestLogs = (filters: RequestLogFilters = {}) =>
  invoke<PaginatedLogs>("get_request_logs", { filters });
