import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, Check, Search, Loader2, AlertTriangle, RefreshCw } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import { Modal } from "@/components/Modal";
import { toast } from "sonner";
import { fetchMarket, installMarketItem, listServers, listSkills } from "@/lib/ipc";
import type { MarketItem } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function Market() {
  const { t } = useTranslation();
  const [items, setItems] = useState<MarketItem[]>([]);
  const [tab, setTab] = useState<"server" | "skill">("server");
  const [installedIds, setInstalledIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const [detailItem, setDetailItem] = useState<MarketItem | null>(null);

  const inFlight = useRef(false);
  // 修复:unmount 时重置 inFlight,防止 re-mount 后死锁
  useEffect(() => {
    return () => { inFlight.current = false; };
  }, []);

  const refreshInstalled = async () => {
    const [svrs, sks] = await Promise.all([listServers(), listSkills()]);
    const ids = new Set<string>();
    // 修复:服务器用 name 匹配(与 market item id 分离),技能用 id 匹配
    for (const s of svrs) ids.add(`server:${s.name}`);
    for (const k of sks) ids.add(`skill:${k.id}`);
    setInstalledIds(ids);
  };

  const load = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setLoading(true);
    setLoadError(null);
    try {
      const r = await fetchMarket(tab);
      setItems(r);
      await refreshInstalled();
    } catch (e) {
      setLoadError(describeError(e));
    } finally {
      inFlight.current = false;
      setLoading(false);
    }
  };
  useEffect(() => { void load(); }, [tab]);

  const filtered = useMemo(() => {
    if (!search.trim()) return items;
    const q = search.toLowerCase();
    return items.filter(
      (it) =>
        it.name.toLowerCase().includes(q) ||
        (it.description ?? "").toLowerCase().includes(q),
    );
  }, [items, search]);

  const isInstalled = (it: MarketItem) =>
    installedIds.has(it.kind === "server" ? `server:${it.name}` : `skill:${it.id}`);

  const install = async (it: MarketItem) => {
    setBusyIds((p) => new Set(p).add(it.id));
    try {
      await installMarketItem(it.id, it.kind, it.source_url);
      await refreshInstalled();
      toast.success(t("market.install_success") as string);
    } catch (e) {
      toast.error(t("market.install_failed", { msg: describeError(e) }) as string);
    } finally {
      setBusyIds((p) => { const n = new Set(p); n.delete(it.id); return n; });
    }
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("market.title")}
        action={
          <div className="flex gap-2">
            <div className="flex max-w-[220px]">
              <input
                type="search"
                className="input flex-1 rounded-r-none"
                placeholder={t("common.search")}
                value={searchInput}
                onChange={(e) => setSearchInput(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") setSearch(searchInput); }}
              />
              <button className="btn-primary rounded-l-none" onClick={() => setSearch(searchInput)} title={t("common.search")}>
                <Search size={14} />
              </button>
            </div>
            <button className={tab === "server" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("server")}>{t("market.tab_servers")}</button>
            <button className={tab === "skill"  ? "btn-primary" : "btn-ghost"} onClick={() => setTab("skill")}>{t("market.tab_skills")}</button>
          </div>
        }
      >
        {/* 加载骨架 */}
        {loading && !loadError && (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="card space-y-3">
                <div className="h-5 w-2/3 animate-pulse rounded bg-bg-subtle" />
                <div className="h-4 w-1/3 animate-pulse rounded bg-bg-subtle" />
                <div className="h-10 w-full animate-pulse rounded bg-bg-subtle" />
                <div className="h-8 w-1/2 animate-pulse rounded bg-bg-subtle" />
              </div>
            ))}
          </div>
        )}

        {/* 错误状态 + 重试 */}
        {!loading && loadError && (
          <div role="alert" className="flex items-start gap-2 rounded-md border border-err/40 bg-err/5 p-3 text-xs">
            <AlertTriangle size={14} className="mt-0.5 shrink-0 text-err" />
            <div className="flex-1">
              <div className="font-medium text-err">{t("market.load_error_title")}</div>
              <div className="mt-0.5 text-fg-muted break-all">{loadError}</div>
            </div>
            <button className="btn-primary text-xs" onClick={() => void load()}>
              <RefreshCw size={14} />
              {t("market.retry")}
            </button>
          </div>
        )}

        {!loading && !loadError && filtered.length === 0 && (
          <EmptyHint>{search.trim() ? t("market.no_match") : t("market.empty")}</EmptyHint>
        )}

        {!loading && !loadError && filtered.length > 0 && (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {filtered.map((it) => {
              const installed = isInstalled(it);
              const busy = busyIds.has(it.id);
              return (
                <div
                  key={it.id}
                  className="card flex cursor-pointer flex-col gap-2 transition-shadow hover:shadow-md"
                  onClick={() => setDetailItem(it)}
                >
                  <div className="flex items-start justify-between">
                    <div>
                      <div className="font-semibold">{it.name}</div>
                      <div className="text-xs text-fg-muted">{t("market.by_author", { author: it.author ?? "—" })}</div>
                    </div>
                    <span className="badge-mute">{it.kind}</span>
                  </div>
                  <div className="line-clamp-3 text-sm text-fg-muted">{it.description}</div>
                  <div className="flex items-center justify-between text-xs" onClick={(e) => e.stopPropagation()}>
                    <span className="text-fg-muted">{t("market.installs_count", { n: it.installs })}</span>
                    <button
                      className={installed ? "btn-ok" : "btn-primary"}
                      disabled={installed || busy}
                      onClick={() => install(it)}
                    >
                      {busy ? <Loader2 size={14} className="animate-spin" /> :
                       installed ? <><Check size={14}/>{t("market.installed")}</> :
                       <><Download size={14}/>{t("market.install")}</>}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Section>

      {/* 详情弹窗 */}
      {detailItem && (
        <Modal
          open
          title={detailItem.name}
          onClose={() => setDetailItem(null)}
          footer={
            <button className="btn-primary" onClick={() => { install(detailItem); setDetailItem(null); }} disabled={isInstalled(detailItem)}>
              {isInstalled(detailItem) ? t("market.installed") : t("market.install")}
            </button>
          }
        >
          <div className="space-y-3 text-sm">
            <div className="flex items-center gap-2">
              <span className="badge-mute">{detailItem.kind}</span>
              <span className="text-xs text-fg-muted">{t("market.by_author", { author: detailItem.author ?? "—" })}</span>
              <span className="text-xs text-fg-muted">· {t("market.installs_count", { n: detailItem.installs })}</span>
            </div>
            <p className="text-fg-muted">{detailItem.description}</p>
            {detailItem.tags && detailItem.tags.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {detailItem.tags.map((tag) => (
                  <span key={tag} className="badge-mute text-3xs">{tag}</span>
                ))}
              </div>
            )}
          </div>
        </Modal>
      )}
    </div>
  );
}
