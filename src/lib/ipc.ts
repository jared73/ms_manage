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
  bearer_token?: string;
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
  file_path: string;
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
  mcp_token?: string;
}

// ---- MCP Server ----
export const listServers = () => invoke<McpServer[]>("list_servers");
export const getServerState = (id: string) => invoke<ServerState>("get_server_state", { id });
export const listServerStates = () => invoke<ServerState[]>("list_server_states");
export const upsertServer = (server: McpServer) => invoke<McpServer>("upsert_server", { server });
export const deleteServer = (id: string) => invoke<void>("delete_server", { id });
export const startServer = (id: string) => invoke<void>("start_server", { id });
export const stopServer = (id: string) => invoke<void>("stop_server", { id });
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
export const fetchMarket = (kind?: "server" | "skill", force_refresh?: boolean) =>
  invoke<MarketItem[]>("fetch_market", { kind, force_refresh: force_refresh ?? false });
export const installMarketItem = (id: string, kind: string, source_url?: string) => invoke<void>("install_market_item", { id, kind, source_url });

// ---- Logs ----
export const listLogs = (opts?: { limit?: number; level?: string; source?: string }) =>
  invoke<LogEntry[]>("list_logs", { opts: opts ?? {} });
export const clearLogs = () => invoke<void>("clear_logs");

// ---- Settings ----
export const getSettings = () => invoke<AppSettings>("get_settings");
export const updateSettings = (settings: AppSettings) => invoke<void>("update_settings", { settings });
export const regenerateToken = () => invoke<string>("regenerate_token");
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
