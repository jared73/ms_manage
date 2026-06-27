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
          {title && <h1 className="text-xl font-semibold">{title}</h1>}
          {action}
        </div>
      )}
      {children}
    </section>
  );
}

export function EmptyHint({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-dashed border-white/10 bg-bg-subtle p-10 text-center text-sm text-fg-muted">
      {children}
    </div>
  );
}
