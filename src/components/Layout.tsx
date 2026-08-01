import { Outlet, NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LayoutGrid, TerminalSquare, Zap, BookOpen, Share2, ClipboardList, SlidersHorizontal } from "lucide-react";
import { clsx } from "clsx";

interface NavItem {
  to: string;
  key: string;
  icon: typeof LayoutGrid;
  end?: boolean;
}

const items: NavItem[] = [
  { to: "/",            key: "dashboard",    icon: LayoutGrid,     end: true },
  { to: "/servers",     key: "servers",      icon: TerminalSquare },
  { to: "/skills",      key: "skills",       icon: Zap },
  { to: "/memories",    key: "memories",     icon: BookOpen },
  { to: "/connections", key: "connections",  icon: Share2 },
  { to: "/logs",        key: "logs",         icon: ClipboardList },
  { to: "/settings",    key: "settings",     icon: SlidersHorizontal },
];

export function Layout() {
  const { t } = useTranslation();
  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-bg-subtle">
        <div className="flex h-14 items-center gap-2.5 border-b border-border px-4">
          <div className="grid h-8 w-8 place-items-center rounded-lg bg-accent text-base font-bold text-white shadow-md shadow-accent/25">山</div>
          <div>
            <div className="text-sm font-bold leading-tight tracking-tight">MS Manage</div>
            <div className="text-2xs text-fg-muted leading-tight">{t("app.tagline")}</div>
          </div>
        </div>
        <nav className="flex-1 space-y-0.5 p-2" aria-label="Main navigation">
          {items.map(({ to, key, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={Boolean(end)}
              className={({ isActive }) =>
                clsx(
                  "flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-all",
                  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-1 focus-visible:ring-offset-bg-subtle",
                  isActive
                    ? "bg-accent/15 text-accent font-medium shadow-sm shadow-accent/10"
                    : "text-fg-muted hover:bg-overlay-soft hover:text-fg",
                )
              }
            >
              {({ isActive }) => (
                <>
                  <Icon size={17} strokeWidth={isActive ? 2.2 : 1.8} aria-hidden="true" />
                  <span>{t(`nav.${key}`)}</span>
                </>
              )}
            </NavLink>
          ))}
        </nav>
        <div className="border-t border-border px-4 py-3 text-2xs text-fg-muted">
          v{__APP_VERSION__ ?? "1.5.0"}
        </div>
      </aside>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
