import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, Download, Check } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import { fetchMarket, installMarketItem, listServers, listSkills } from "@/lib/ipc";
import type { MarketItem } from "@/lib/ipc";

export function Market() {
  const { t } = useTranslation();
  const [items, setItems] = useState<MarketItem[]>([]);
  const [tab, setTab] = useState<"server" | "skill">("server");
  // 已安装 ID 集合：根据 servers/skills 实际存储动态计算（不是 local state）
  const [installedIds, setInstalledIds] = useState<Set<string>>(new Set());

  const refreshInstalled = async () => {
    const [svrs, sks] = await Promise.all([listServers(), listSkills()]);
    const ids = new Set<string>();
    for (const s of svrs) ids.add(s.name);            // server name == market item.id
    for (const k of sks)  ids.add(k.id);               // skill id   == market item.id
    setInstalledIds(ids);
  };

  const load = async () => {
    const r = await fetchMarket(tab);
    setItems(r);
    await refreshInstalled();
  };
  useEffect(() => { void load(); }, [tab]);

  const install = async (it: MarketItem) => {
    await installMarketItem(it.id);
    await refreshInstalled();
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("market.title")}
        action={
          <div className="flex gap-2">
            <button className={tab === "server" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("server")}>{t("market.tab_servers")}</button>
            <button className={tab === "skill"  ? "btn-primary" : "btn-ghost"} onClick={() => setTab("skill")}>{t("market.tab_skills")}</button>
            <button className="btn-ghost" onClick={load}><RefreshCw size={14}/>{t("market.refresh_btn")}</button>
          </div>
        }
      >
        {items.length === 0 ? (
          <EmptyHint>{t("market.empty")}</EmptyHint>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
            {items.map((it) => {
              const installed = installedIds.has(it.id);
              return (
                <div key={it.id} className="card flex flex-col gap-2">
                  <div className="flex items-start justify-between">
                    <div>
                      <div className="font-semibold">{it.name}</div>
                      <div className="text-xs text-fg-muted">{t("market.by_author", { author: it.author ?? "—" })}</div>
                    </div>
                    <span className="badge-mute">{it.kind}</span>
                  </div>
                  <div className="line-clamp-3 text-sm text-fg-muted">{it.description}</div>
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-fg-muted">{t("market.installs_count", { n: it.installs })}</span>
                    <button
                      className={installed ? "btn-ok" : "btn-primary"}
                      disabled={installed}
                      onClick={() => install(it)}
                    >
                      {installed ? <><Check size={14}/>{t("market.installed")}</> : <><Download size={14}/>{t("market.install")}</>}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Section>
    </div>
  );
}
