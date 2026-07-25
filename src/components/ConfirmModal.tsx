/**
 * 通用二次确认弹窗。
 *
 * 用于"删除 / 重置 / 清空"等危险操作的二次确认。
 * - 默认 size="sm"
 * - 默认 danger 主按钮(红底);通过 `variant="default"` 改用普通 btn-primary。
 * - 内置 busy/disabled 状态:`onConfirm` 在执行时通过 setBusy 控制 spinning。
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Modal } from "./Modal";

interface ConfirmModalProps {
  open: boolean;
  /** 主标题内容(主标题是必传,具体文案可包 JSX) */
  title: React.ReactNode;
  children: React.ReactNode;
  /** 主解释文本(已用 children 替代;保留仅为向后兼容) */
  message?: React.ReactNode;
  /** 取消按钮文案,默认 "common.cancel" */
  cancelLabel?: React.ReactNode;
  /** 确认按钮文案,默认 "common.confirm" */
  confirmLabel?: React.ReactNode;
  /** variant="danger" 用红色主按钮;"default" 用蓝色 */
  variant?: "danger" | "default";
  /** 外部传 busy(用于调用方已跟踪的加载状态) */
  busy?: boolean;
  /** 点击确认后调用,可以是 Promise;内部使用组件 busy 状态管理按钮 disabled */
  onConfirm: () => void | Promise<void>;
  onClose: () => void;
}

export function ConfirmModal({
  open,
  title,
  children,
  // message 已废弃 —— 直接用 children 即可;保留参数避免破坏旧调用
  message: _deprecatedMessage,
  cancelLabel,
  confirmLabel,
  variant = "danger",
  busy,
  onConfirm,
  onClose,
}: ConfirmModalProps) {
  const { t } = useTranslation();
  // 组件自管 busy:调用方即使忘了传也能 disable 按钮 + 转圈
  const [internalBusy, setInternalBusy] = useState(false);
  const isBusy = busy ?? internalBusy;
  const handleConfirm = async () => {
    if (isBusy) return;
    const ret = onConfirm();
    if (ret instanceof Promise) {
      setInternalBusy(true);
      try { await ret; }
      catch { /* swallow — 调用方已自行 toast */ }
      finally { setInternalBusy(false); }
    }
  };
  return (
    <Modal
      open={open}
      size="sm"
      title={title}
      onClose={() => { if (!isBusy) onClose(); }}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={isBusy}>
            {cancelLabel ?? t("common.cancel")}
          </button>
          <button
            className={
              variant === "danger"
                ? "btn-primary bg-err hover:bg-err/80 text-bg disabled:opacity-60"
                : "btn-primary disabled:opacity-60"
            }
            onClick={handleConfirm}
            disabled={isBusy}
          >
            {isBusy ? <Loader2 size={14} className="inline animate-spin mr-1" /> : null}
            {confirmLabel ?? t("common.confirm")}
          </button>
        </>
      }
    >
      {children}
    </Modal>
  );
}
