import { Outlet, NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { LayoutDashboard, Server, Sparkles, MonitorSmartphone, Store, ScrollText, Settings as Cog } from "lucide-react";
import { clsx } from "clsx";

interface NavItem {
  to: string;
  key: string;
  icon: typeof LayoutDashboard;
  end?: boolean;
}

const items: NavItem[] = [
  { to: "/",         key: "dashboard", icon: LayoutDashboard, end: true },
  { to: "/servers",  key: "servers",   icon: Server },
  { to: "/skills",   key: "skills",    icon: Sparkles },
  { to: "/claude",   key: "claude",    icon: MonitorSmartphone },
  { to: "/market",   key: "market",    icon: Store },
  { to: "/logs",     key: "logs",      icon: ScrollText },
  { to: "/settings", key: "settings",  icon: Cog },
];

export function Layout() {
  const { t } = useTranslation();
  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <aside className="flex w-56 shrink-0 flex-col border-r border-white/5 bg-bg-subtle">
        <div className="flex h-14 items-center gap-2 border-b border-white/5 px-4">
          <div className="grid h-8 w-8 place-items-center rounded-md bg-accent text-sm font-bold">AI</div>
          <div>
            <div className="text-sm font-semibold leading-tight">{t("app.title")}</div>
            <div className="text-[11px] text-fg-muted leading-tight">{t("app.tagline")}</div>
          </div>
        </div>
        <nav className="flex-1 space-y-0.5 p-2">
          {items.map(({ to, key, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={Boolean(end)}
              className={({ isActive }) =>
                clsx(
                  "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
                  isActive
                    ? "bg-accent/15 text-accent"
                    : "text-fg-muted hover:bg-white/5 hover:text-fg",
                )
              }
            >
              <Icon size={16} />
              <span>{t(`nav.${key}`)}</span>
            </NavLink>
          ))}
        </nav>
        <div className="border-t border-white/5 p-3 text-[11px] text-fg-muted">
          v0.1.0 · Tauri 2 + React
        </div>
      </aside>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
