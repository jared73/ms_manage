import type { McpServer } from "@/lib/ipc";

/** 给"新建"按钮用的空白 server。
 *
 * 拆到独立文件是为了满足 react-refresh 的"单文件只 export 组件"约束:
 * ServerFormModal.tsx 自身只 export ServerFormModal 组件,
 * blankServer 是数据工厂函数(不是组件),放这里最干净。
 */
export function blankServer(): McpServer {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    name: "",
    transport: "stdio",
    command: "",
    args: [],
    env: {},
    enabled: true,
    created_at: now,
    updated_at: now,
  };
}