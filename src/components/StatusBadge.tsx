import { useTranslation } from "react-i18next";
import { clsx } from "clsx";
import type { ServerStatus } from "@/lib/ipc";

export function StatusBadge({ status }: { status: ServerStatus }) {
  const { t } = useTranslation();
  const map: Record<ServerStatus, { cls: string; label: string }> = {
    running:  { cls: "badge-ok",   label: t("status.running") },
    starting: { cls: "badge-warn", label: t("status.starting") },
    stopped:  { cls: "badge-mute", label: t("status.stopped") },
    error:    { cls: "badge-err",  label: t("status.error") },
    disabled: { cls: "badge-mute", label: t("status.disabled") },
  };
  const m = map[status];
  return (
    <span className={clsx(m.cls)}>
      <span className={clsx("inline-block h-1.5 w-1.5 rounded-full",
        status === "running"  && "bg-ok",
        status === "starting" && "bg-warn animate-pulse",
        status === "stopped"  && "bg-fg-muted",
        status === "error"    && "bg-err",
        status === "disabled" && "bg-fg-muted/50")} />
      {m.label}
    </span>
  );
}
