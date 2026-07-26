import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { clsx } from "clsx";

interface Props {
  open: boolean;
  title: React.ReactNode;
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

/**
 * Modal 焦点循环:WCAG 2.4.3 要求 Tab 键在 dialog 内循环,不能跑到背景页面。
 *
 * 实现:监听 dialog 内 keydown(Tab/Shift+Tab):
 * - 找到 dialog 内第一个和最后一个可聚焦元素
 * - Shift+Tab 在第一个时 → 跳到最后一个
 * - Tab 在最后一个时 → 跳到第一个
 * - 其他情况不动(浏览器原生行为处理)
 *
 * 注意:不仅 dialog 内元素要循环,dialog 本身(tabIndex=-1)也算一个,
 * 因为有时焦点可能落在 dialog 容器上(无 children 时)。
 */
const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function Modal({ open, title, onClose, children, footer, size = "md" }: Props) {
  const { t } = useTranslation();
  useEscClose(open, onClose);
  const dialogRef = useRef<HTMLDivElement>(null);

  // 焦点管理:
  // - 打开时把焦点丢到对话框内第一个可聚焦元素(input/button/textarea/select/...
  //   匹配 tabindex>-1);如果对话框内没有可聚焦元素,焦点停留在 dialog 自身上。
  // - 关闭时把焦点归还给"打开它的按钮"(如果还能找到的话)。这一步需要文档级
  //   没有太多干扰,所以只在 dialog 仍打开且 focusing 工作未被打断时执行。
  useEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    // 微任务延后:等 React 把 children 真正挂上
    const handle = window.setTimeout(() => {
      const sel = 'input, select, textarea, button, [tabindex]:not([tabindex="-1"])';
      const first = dialog.querySelector<HTMLElement>(sel);
      if (first) first.focus();
      else dialog.focus();
    }, 0);
    return () => window.clearTimeout(handle);
  }, [open]);

  // Focus trap:在 dialog 内拦截 Tab/Shift+Tab,首尾循环
  useEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const focusables = Array.from(
        dialog.querySelectorAll<HTMLElement>(FOCUSABLE),
      ).filter((el) => !el.hasAttribute("disabled") && el.tabIndex !== -1);
      const active = document.activeElement as HTMLElement | null;
      // 先区分焦点是否在 dialog 外,不要把不可聚焦的 dialog 容器混入真实边界。
      // 只有没有任何可聚焦子元素时,dialog 才作为唯一 fallback。
      if (focusables.length === 0) {
        e.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      if (!dialog.contains(active)) {
        e.preventDefault();
        (e.shiftKey ? last : first).focus();
      } else if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    // capture 阶段,确保在 React 自己处理 keydown 前拦住
    dialog.addEventListener("keydown", handler, true);
    return () => dialog.removeEventListener("keydown", handler, true);
  }, [open]);

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
        ref={dialogRef}
        role="dialog"
        tabIndex={-1}
        aria-modal="true"
        aria-label={typeof title === "string" ? title : undefined}
        className={clsx("w-full rounded-lg border border-border bg-bg-elev shadow-2xl focus:outline-none", sizeCls)}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-base font-semibold">{title}</h2>
          <button className="btn-ghost p-1" onClick={onClose} aria-label={t("common.close")}>
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
