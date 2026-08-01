/**
 * Toast 哨兵:兜底强制关闭被 sonner 的悬停暂停机制卡住的 toast。
 *
 * 背景:sonner(1.7.x/2.x)的自动关闭计时器在鼠标悬停过 toast 容器(expanded)
 * 或点击过 toast(interacting)时永久暂停,且 onMouseLeave 存在边界导致
 * expanded 无法复位 —— 结果就是 toast 永远不消失、挡住界面。
 *
 * 设计:
 * - 每 500ms 轮询 toast.getToasts(),记录每条活跃 toast 的首次出现时间
 * - 超过 duration + 600ms 宽限仍未消失时,调用 toast.dismiss() 强制关闭
 * - loading 类型与显式 duration: Infinity 的 toast 跳过(sonner 本身就不该自动关)
 * - 零业务改动:不碰任何 toast.success/error 调用点
 */
import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { TOAST_DURATION } from "@/lib/toast";

const GRACE_MS = 600;
const POLL_MS = 500;

export function ToastSentinel() {
  const seenAt = useRef(new Map<string | number, number>());

  useEffect(() => {
    const iv = setInterval(() => {
      const now = Date.now();
      for (const t of toast.getToasts()) {
        if ("dismiss" in t) continue;
        if (t.type === "loading" || t.duration === Infinity) continue;
        const duration = typeof t.duration === "number" ? t.duration : TOAST_DURATION;
        const born = seenAt.current.get(t.id) ?? now;
        seenAt.current.set(t.id, born);
        if (now - born >= duration + GRACE_MS) {
          seenAt.current.delete(t.id);
          toast.dismiss(t.id);
        }
      }
    }, POLL_MS);
    return () => clearInterval(iv);
  }, []);

  return null;
}
