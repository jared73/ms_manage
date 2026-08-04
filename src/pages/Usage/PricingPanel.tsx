import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
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

  const priceFields = [
    ["inputCostPerMillion", t("usage.input_price")],
    ["outputCostPerMillion", t("usage.output_price")],
    ["cacheReadCostPerMillion", t("usage.cache_read_price")],
    ["cacheCreationCostPerMillion", t("usage.cache_creation_price")],
  ] as const;

  return (
    <div>
      <div className="mb-3 flex justify-end">
        <button
          onClick={openAdd}
          className="rounded-lg border border-border px-2.5 py-1 text-xs hover:bg-overlay-soft"
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
          {items.map((p) => (
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

      <Modal
        open={editing !== null}
        title={isNew ? t("usage.add_model") : t("usage.edit_pricing")}
        onClose={() => setEditing(null)}
        size="sm"
        footer={
          <>
            <button onClick={() => setEditing(null)} className="rounded-lg border border-border px-3 py-1.5 text-xs">
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
    </div>
  );
}
