/**
 * Servers 页面用到的共用小组件。
 * 把跨子组件复用的小东西集中到这里,避免每个文件重复定义。
 *
 * 注意:后端安装进度事件的类型 `InstallProgressEvent` 定义在 `@/lib/ipc`
 * (IPC 契约以该文件为准),这里不再重复定义。
 */
import type React from "react";

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
        <div className="mt-1 text-2xs text-err" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
