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
        <div className="flex items-center justify-between">
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
