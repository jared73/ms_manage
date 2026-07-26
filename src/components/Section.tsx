import { clsx } from "clsx";

export function Section({ title, action, children, className }: {
  title?: React.ReactNode;
  action?: React.ReactNode;
  children?: React.ReactNode;
  className?: string;
}) {
  return (
    <section className={clsx("space-y-3", className)}>
      {(title || action) && (
        // 用 flex-wrap + gap:窄窗口下 action 控件会折到下一行,而不是挤压成竖排。
        // 之前用 justify-between + 单行 flex,960×640 下 Logs 页 7 个控件会顶成 1px 宽竖排。
        <div className="flex flex-wrap items-center justify-between gap-3">
          {title && <h2 className="text-xl font-semibold">{title}</h2>}
          {action}
        </div>
      )}
      {children}
    </section>
  );
}

export function EmptyHint({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-dashed border-border bg-bg-subtle p-10 text-center text-sm text-fg-muted">
      {children}
    </div>
  );
}
