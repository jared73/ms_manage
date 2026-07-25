import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, Check } from "lucide-react";
import { Section, EmptyHint } from "@/components/Section";
import { toast } from "sonner";
import { fetchMarket, installMarketItem, listServers, listSkills } from "@/lib/ipc";
import type { MarketItem } from "@/lib/ipc";
import { describeError } from "@/lib/errors";

export function Market() {
  const { t } = useTranslation();
  const [items, setItems] = useState<MarketItem[]>([]);
  const [tab, setTab] = useState<"server" | "skill">("server");
  const [installedIds, setInstalledIds] = useState<Set<string>>(new Set());

  const inFlight = useRef(false);

  const refreshInstalled = async () => {
    const [svrs, sks] = await Promise.all([listServers(), listSkills()]);
    const ids = new Set<string>();
    for (const s of svrs) ids.add(s.name);
    for (const k of sks) ids.add(k.id);
    setInstalledIds(ids);
  };

  const load = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const r = await fetchMarket(tab);
      setItems(r);
      await refreshInstalled();
    } catch (e) {
      toast.error(t("market.load_failed", { msg: describeError(e) }) as string);
    } finally {
      inFlight.current = false;
    }
  };
  useEffect(() => { void load(); }, [tab]);

  const install = async (it: MarketItem) => {
    try {
      await installMarketItem(it.id, it.kind, it.source_url);
      await refreshInstalled();
      toast.success(t("market.install_success") as string);
    } catch (e) {
      toast.error(t("market.install_failed", { msg: describeError(e) }) as string);
    }
  };

  return (
    <div className="space-y-4 p-6">
      <Section
        title={t("market.title")}
        action={
          <div className="flex gap-2">
            <button className={tab === "server" ? "btn-primary" : "btn-ghost"} onClick={() => setTab("server")}>{t("market.tab_servers")}</button>
            <button className={tab === "skill"  ? "btn-primary" : "btn-ghost"} onClick={() => setTab("skill")}>{t("market.tab_skills")}</button>
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
