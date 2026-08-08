import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, Upload, Download, Percent } from "lucide-react";
import { toast } from "sonner";
import {
  deleteModelPricing,
  getAppConfigs,
  getModelPricing,
  updateAppConfigBatch,
  updateModelPricingBatch,
  type ModelPricingInput,
} from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { Modal } from "@/components/Modal";
import { ConfirmModal } from "@/components/ConfirmModal";

const APP_TYPES = ["opencode", "claude", "codex", "gemini", "grokbuild"];

function emptyPricing(): ModelPricingInput {
  return {
    modelId: "",
    displayName: "",
    inputCostPerMillion: "",
    outputCostPerMillion: "",
    cacheReadCostPerMillion: "",
    cacheCreationCostPerMillion: "",
  };
}

function parseDecimal(s: string): number {
  const n = parseFloat(s);
  return Number.isFinite(n) ? n : 0;
}

export function PricingPanel() {
  const { t } = useTranslation();
  const [items, setItems] = useState<ModelPricingInput[]>([]);
  const [editing, setEditing] = useState<ModelPricingInput | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState<ModelPricingInput | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [multipliers, setMultipliers] = useState<Record<string, string>>({});
  const [multiplierSaving, setMultiplierSaving] = useState(false);

  const [query, setQuery] = useState("");

  // 批量调整 modal
  const [bulkOpen, setBulkOpen] = useState(false);
  const [bulkPercent, setBulkPercent] = useState("10");
  const [bulkBusy, setBulkBusy] = useState(false);

  // 导入 JSON modal
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [importPending, setImportPending] = useState<{
    parsed: ModelPricingInput[];
    existing: number;
    fresh: number;
  } | null>(null);
  const [importBusy, setImportBusy] = useState(false);

  const load = async () => {
    try {
      setItems(await getModelPricing());
    } catch (e) {
      toast.error(describeError(e));
    }
  };

  const loadMultipliers = async () => {
    try {
      const defaults: Record<string, string> = Object.fromEntries(
        APP_TYPES.map((app) => [app, "1.0"]),
      );
      for (const c of await getAppConfigs()) {
        defaults[c.appType] = c.costMultiplier;
      }
      setMultipliers(defaults);
    } catch (e) {
      toast.error(describeError(e));
    }
  };

  useEffect(() => {
    void load();
    void loadMultipliers();
  }, []);

  const filteredItems = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(
      (i) =>
        i.modelId.toLowerCase().includes(q) ||
        (i.displayName && i.displayName.toLowerCase().includes(q)),
    );
  }, [items, query]);

  const openAdd = () => {
    setEditing(emptyPricing());
    setIsNew(true);
  };

  const openEdit = (p: ModelPricingInput) => {
    setEditing({ ...p });
    setIsNew(false);
  };

  const save = async () => {
    if (!editing) return;
    if (isNew && !editing.modelId.trim()) {
      toast.error(t("usage.model_id_required"));
      return;
    }
    setSaving(true);
    try {
      await updateModelPricingBatch([editing]);
      toast.success(t("usage.pricing_saved"));
      setEditing(null);
      await load();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async () => {
    if (!deleting) return;
    setDeleteBusy(true);
    try {
      await deleteModelPricing(deleting.modelId);
      toast.success(t("usage.pricing_deleted"));
      setDeleting(null);
      await load();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setDeleteBusy(false);
    }
  };

  const saveMultipliers = async () => {
    setMultiplierSaving(true);
    try {
      await updateAppConfigBatch(
        APP_TYPES.map((app) => ({ appType: app, costMultiplier: multipliers[app] ?? "1.0" })),
      );
      toast.success(t("usage.multiplier_saved"));
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setMultiplierSaving(false);
    }
  };

  // 批量调整：按百分比同步上调/下调所有 PRICE_FIELDS
  const applyBulkAdjust = async () => {
    const pct = parseFloat(bulkPercent);
    if (!Number.isFinite(pct)) {
      toast.error(t("usage.pricing_bulk_adjust_label"));
      return;
    }
    const factor = 1 + pct / 100;
    const targets = filteredItems;
    if (targets.length === 0) {
      toast.error(t("usage.pricing_bulk_adjust_tip", { count: 0 }));
      return;
    }
    setBulkBusy(true);
    try {
      const adjusted = targets.map((p) => ({
        ...p,
        inputCostPerMillion: (parseDecimal(p.inputCostPerMillion) * factor).toFixed(4),
        outputCostPerMillion: (parseDecimal(p.outputCostPerMillion) * factor).toFixed(4),
        cacheReadCostPerMillion: (parseDecimal(p.cacheReadCostPerMillion) * factor).toFixed(4),
        cacheCreationCostPerMillion: (parseDecimal(p.cacheCreationCostPerMillion) * factor).toFixed(4),
      }));
      await updateModelPricingBatch(adjusted);
      toast.success(t("usage.pricing_saved"));
      setBulkOpen(false);
      await load();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setBulkBusy(false);
    }
  };

  // 导出 JSON
  const doExport = () => {
    const blob = new Blob([JSON.stringify(items, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `model-pricing-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  // 导入 JSON：解析 + 计算 overlap，再让用户选择覆盖/合并
  const onImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const parsed = JSON.parse(text);
      if (!Array.isArray(parsed)) {
        toast.error(t("usage.pricing_import_invalid"));
        return;
      }
      const normalized: ModelPricingInput[] = parsed.map((p: unknown) => {
        const o = p as Partial<ModelPricingInput>;
        return {
          modelId: String(o.modelId ?? "").trim(),
          displayName: String(o.displayName ?? ""),
          inputCostPerMillion: String(o.inputCostPerMillion ?? "0"),
          outputCostPerMillion: String(o.outputCostPerMillion ?? "0"),
          cacheReadCostPerMillion: String(o.cacheReadCostPerMillion ?? "0"),
          cacheCreationCostPerMillion: String(o.cacheCreationCostPerMillion ?? "0"),
        };
      }).filter((p) => p.modelId);
      const existingIds = new Set(items.map((i) => i.modelId));
      const existing = normalized.filter((p) => existingIds.has(p.modelId)).length;
      const fresh = normalized.length - existing;
      setImportPending({ parsed: normalized, existing, fresh });
    } catch (e) {
      toast.error(t("usage.pricing_import_invalid"));
      void e;
    } finally {
      // 清空 input value 以便下次同文件能再次触发
      if (fileInputRef.current) fileInputRef.current.value = "";
    }
  };

  const applyImport = async (mode: "overwrite" | "merge") => {
    if (!importPending) return;
    setImportBusy(true);
    try {
      const targets =
        mode === "merge"
          ? importPending.parsed.filter(
              (p) => !items.some((i) => i.modelId === p.modelId),
            )
          : importPending.parsed;
      if (targets.length === 0) {
        toast.success(t("usage.pricing_saved"));
      } else {
        await updateModelPricingBatch(targets);
        toast.success(t("usage.pricing_saved"));
      }
      setImportPending(null);
      await load();
    } catch (e) {
      toast.error(describeError(e));
    } finally {
      setImportBusy(false);
    }
  };

  const priceFields = [
    ["inputCostPerMillion", t("usage.input_price")],
    ["outputCostPerMillion", t("usage.output_price")],
    ["cacheReadCostPerMillion", t("usage.cache_read_price")],
    ["cacheCreationCostPerMillion", t("usage.cache_creation_price")],
  ] as const;

  return (
    <div>
      {/* Toolbar */}
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <div className="relative flex-1 min-w-[180px]">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-fg-muted" />
          <input
            type="text"
            className="w-full rounded-lg border border-border bg-bg py-1.5 pl-8 pr-3 text-xs focus:outline-none focus:ring-2 focus:ring-accent"
            placeholder={t("usage.pricing_search_placeholder") as string}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <button
          onClick={() => setBulkOpen(true)}
          disabled={filteredItems.length === 0}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-overlay-soft disabled:opacity-40"
          title={t("usage.pricing_bulk_adjust_tip", { count: filteredItems.length }) as string}
        >
          <Percent className="icon-xs" />
          {t("usage.pricing_bulk_adjust")}
        </button>
        <button
          onClick={doExport}
          disabled={items.length === 0}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-overlay-soft disabled:opacity-40"
        >
          <Download className="icon-xs" />
          {t("usage.pricing_export_json")}
        </button>
        <button
          onClick={() => fileInputRef.current?.click()}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-overlay-soft"
        >
          <Upload className="icon-xs" />
          {t("usage.pricing_import_json")}
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="application/json,.json"
          onChange={onImportFile}
          className="hidden"
        />
        <button
          onClick={openAdd}
          className="rounded-lg border border-border px-2.5 py-1.5 text-xs hover:bg-overlay-soft"
        >
          {t("usage.add_model")}
        </button>
      </div>

      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-2xs text-fg-muted">
            <th className="py-2 pr-3">{t("usage.model")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.input_price")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.output_price")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.cache_read_price")}</th>
            <th className="py-2 pr-3 text-right">{t("usage.cache_creation_price")}</th>
            <th className="py-2 text-right"></th>
          </tr>
        </thead>
        <tbody>
          {filteredItems.map((p) => (
            <tr key={p.modelId} className="border-b border-border">
              <td className="py-2 pr-3 font-mono text-xs">{p.displayName || p.modelId}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.inputCostPerMillion}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.outputCostPerMillion}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.cacheReadCostPerMillion}</td>
              <td className="py-2 pr-3 text-right text-xs">{p.cacheCreationCostPerMillion}</td>
              <td className="py-2 text-right">
                <div className="flex justify-end gap-2">
                  <button
                    onClick={() => openEdit(p)}
                    className="rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-overlay-soft"
                  >
                    {t("usage.edit")}
                  </button>
                  <button
                    onClick={() => setDeleting(p)}
                    className="rounded-lg border border-border px-2.5 py-1 text-xs text-err hover:bg-overlay-soft"
                  >
                    {t("usage.delete")}
                  </button>
                </div>
              </td>
            </tr>
          ))}
          {filteredItems.length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-xs text-fg-muted">
                {query ? t("usage.no_logs") : t("usage.no_logs")}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <div className="mt-6 border-t border-border pt-4">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-sm font-medium">{t("usage.cost_multiplier")}</h3>
          <button
            onClick={saveMultipliers}
            disabled={multiplierSaving}
            className="rounded-lg bg-accent px-2.5 py-1 text-xs font-medium text-white disabled:opacity-50"
          >
            {t("usage.save_multiplier")}
          </button>
        </div>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
          {APP_TYPES.map((app) => (
            <div key={app}>
              <label className="mb-1 block text-xs text-fg-muted">{app}</label>
              <input
                type="text"
                inputMode="decimal"
                value={multipliers[app] ?? "1.0"}
                onChange={(e) => setMultipliers((m) => ({ ...m, [app]: e.target.value }))}
                className="w-full rounded-lg border border-border bg-bg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-accent"
              />
            </div>
          ))}
        </div>
      </div>

      {/* 编辑/新增 Modal */}
      <Modal
        open={editing !== null}
        title={isNew ? t("usage.add_model") : t("usage.edit_pricing")}
        onClose={() => setEditing(null)}
        size="sm"
        footer={
          <>
            <button
              onClick={() => setEditing(null)}
              className="rounded-lg border border-border px-3 py-1.5 text-xs"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={save}
              disabled={saving}
              className="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
            >
              {t("common.save")}
            </button>
          </>
        }
      >
        <div className="space-y-3">
          {editing && (
            <>
              <div>
                <label className="mb-1 block text-xs text-fg-muted">{t("usage.model")}</label>
                <input
                  type="text"
                  value={editing.modelId}
                  onChange={(e) => setEditing({ ...editing, modelId: e.target.value })}
                  disabled={!isNew}
                  className="w-full rounded-lg border border-border bg-bg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-accent disabled:opacity-50"
                />
              </div>
              {priceFields.map(([field, label]) => (
                <div key={field}>
                  <label className="mb-1 block text-xs text-fg-muted">{label}</label>
                  <input
                    type="text"
                    inputMode="decimal"
                    value={editing[field]}
                    onChange={(e) => setEditing({ ...editing, [field]: e.target.value })}
                    className="w-full rounded-lg border border-border bg-bg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-accent"
                  />
                </div>
              ))}
            </>
          )}
        </div>
      </Modal>

      {/* 删除确认 */}
      <ConfirmModal
        open={deleting !== null}
        title={t("usage.delete")}
        busy={deleteBusy}
        onConfirm={confirmDelete}
        onClose={() => setDeleting(null)}
      >
        <p className="text-sm">{t("usage.confirm_delete_pricing")}</p>
        {deleting && <p className="mt-2 font-mono text-xs text-fg-muted">{deleting.modelId}</p>}
      </ConfirmModal>

      {/* 批量调整 Modal */}
      <Modal
        open={bulkOpen}
        title={t("usage.pricing_bulk_adjust")}
        onClose={() => setBulkOpen(false)}
        size="sm"
        footer={
          <>
            <button
              onClick={() => setBulkOpen(false)}
              className="rounded-lg border border-border px-3 py-1.5 text-xs"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={applyBulkAdjust}
              disabled={bulkBusy}
              className="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
            >
              {t("common.save")}
            </button>
          </>
        }
      >
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-xs text-fg-muted">
              {t("usage.pricing_bulk_adjust_label")}
            </label>
            <input
              type="text"
              inputMode="decimal"
              value={bulkPercent}
              onChange={(e) => setBulkPercent(e.target.value)}
              className="w-full rounded-lg border border-border bg-bg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-accent"
              placeholder="±10"
            />
          </div>
          <p className="text-xs text-fg-muted">
            {t("usage.pricing_bulk_adjust_tip", { count: filteredItems.length })}
          </p>
        </div>
      </Modal>

      {/* 导入 JSON 确认 Modal */}
      <Modal
        open={importPending !== null}
        title={t("usage.pricing_import_json")}
        onClose={() => setImportPending(null)}
        size="sm"
        footer={
          <>
            <button
              onClick={() => setImportPending(null)}
              disabled={importBusy}
              className="rounded-lg border border-border px-3 py-1.5 text-xs"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={() => applyImport("merge")}
              disabled={importBusy || !importPending || importPending.fresh === 0}
              className="rounded-lg border border-border px-3 py-1.5 text-xs hover:bg-overlay-soft disabled:opacity-40"
            >
              {t("usage.pricing_import_merge")}
            </button>
            <button
              onClick={() => applyImport("overwrite")}
              disabled={importBusy}
              className="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-white disabled:opacity-50"
            >
              {t("usage.pricing_import_overwrite")}
            </button>
          </>
        }
      >
        <div className="space-y-2 text-sm">
          {importPending && (
            <p>
              {t("usage.pricing_import_confirm", {
                total: importPending.parsed.length,
                exists: importPending.existing,
                new: importPending.fresh,
              })}
            </p>
          )}
        </div>
      </Modal>
    </div>
  );
}