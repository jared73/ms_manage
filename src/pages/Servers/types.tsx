/**
 * Servers 页面用到的共用类型与小组件。
 * 把跨子组件复用的小东西集中到这里,避免每个文件重复定义。
 */
import type React from "react";

/** 后端安装进度事件(转译自 src-tauri/src/proxy/install.rs)。 */
export interface InstallProgressEvent {
  id: string;
  stage: "checking" | "installing" | "resolving" | "done" | "error";
  message: string;
}

/** 表单字段统一壳。后续 i18n 收敛时改用 t() 渲染 label 即可。 */
export function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1 text-xs text-fg-muted">{label}</div>
      {children}
    </div>
  );
}
