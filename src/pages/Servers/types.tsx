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

/**
 * 表单字段统一壳。
 * - label 显示在控件上方
 * - children 渲染实际控件(input/textarea/select)
 * - error 在控件下方红字 + role="alert";
 *   同时给所有 input/textarea 加红色边框状态。
 */
export function Field({
  label,
  error,
  children,
}: {
  label: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div className="mb-1 text-xs text-fg-muted">{label}</div>
      <div className={error ? "field-invalid" : ""}>{children}</div>
      {error && (
        <div className="mt-1 text-[11px] text-err" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
