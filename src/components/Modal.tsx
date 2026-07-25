import { useEffect } from "react";
import { X } from "lucide-react";
import { clsx } from "clsx";

interface Props {
  open: boolean;
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  footer?: React.ReactNode;
  size?: "sm" | "md" | "lg" | "xl";
}

/**
 * 全局模态栈计数器:body data-modal-depth 反映当前打开的 modal 数量。
 *
 * 每个 Modal 在 mount 时 +1,unmount 时 -1;按 Esc 时:
 * - 仅"栈顶"模态响应:depth 严格等于本 modal 的层数。
 * - 栈内下层 modal 看到 depth > 自己层数,不响应 —— 避免用户在多层弹窗中按一下 Esc
 *   就把最外层也连带关闭(典型场景:Servers 页打开 ProjectsModal,然后点 Delete
 *   弹"确认删除"二级 modal,这时按 Esc 应该只关二层而不是连 ProjectsModal 都关了)。
 *
 * 实现技巧:在事件 handler 里读取 data-modal-depth,跟当前 modal 的层数比对。
 * 由于 modal 是嵌套挂载的(depth 大的先被卸载),用 stack-counter 即可。
 */
let modalStackCounter = 0;
function useEscClose(open: boolean, onClose: () => void): void {
  useEffect(() => {
    if (!open) return;
    modalStackCounter += 1;
    document.body.dataset.modalDepth = String(modalStackCounter);
    const myDepth = modalStackCounter;
    const handler = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // 当前深度 vs 我自己的深度:只有顶上那个响应 Esc
      const cur = parseInt(document.body.dataset.modalDepth ?? "0", 10);
      if (cur !== myDepth) return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", handler, true);
    return () => {
      window.removeEventListener("keydown", handler, true);
      modalStackCounter -= 1;
      document.body.dataset.modalDepth = String(modalStackCounter);
    };
  }, [open, onClose]);
}

export function Modal({ open, title, onClose, children, footer, size = "md" }: Props) {
  useEscClose(open, onClose);
  if (!open) return null;
  const sizeCls = {
    sm: "max-w-sm",
    md: "max-w-lg",
    lg: "max-w-2xl",
    xl: "max-w-4xl",
  }[size];

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-overlay backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={clsx("w-full rounded-lg border border-border bg-bg-elev shadow-2xl", sizeCls)}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-base font-semibold">{title}</h2>
          <button className="btn-ghost p-1" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </div>
        <div className="max-h-[70vh] overflow-auto p-4">{children}</div>
        {footer && (
          <div className="flex justify-end gap-2 border-t border-border px-4 py-3">{footer}</div>
        )}
      </div>
    </div>
  );
}
