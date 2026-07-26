/**
 * Toggle 开关(Settings autostart / Skills target / ServerDetailDrawer tool 启用)。
 *
 * 设计:
 * - 关态 bg-fg-muted/30 + border-border:light 主题下 fg-muted=slate-600/30%
 *   叠加白底 ≈ 中浅灰,加上 1px 边框 → 跨主题清晰可见。
 * - 开态 bg-accent + border-accent:边框跟底色同色,看起来无边。
 * - 圆形滑块用 absolute 定位 + left 切换。
 *
 * 替代了三处分散的 <button role="switch"> 实现,
 * 避免以后主题/对比度调整时三处各改一次。
 */
import { clsx } from "clsx";

export interface ToggleProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  /** 屏幕阅读器标签;也作为 title 鼠标悬浮提示 */
  label?: string;
  disabled?: boolean;
  /** 异步进行中(true 时显示 aria-busy,样式 + 阻止重复点击) */
  busy?: boolean;
  /** "sm" (h-5 w-9) / "md" (h-6 w-11) */
  size?: "sm" | "md";
  /** 自定义 className(比如 margin / mt-1 这种位置微调) */
  className?: string;
}

export function Toggle({
  checked,
  onChange,
  label,
  disabled = false,
  busy = false,
  size = "sm",
  className,
}: ToggleProps) {
  const dims = size === "md"
    ? { track: "h-6 w-11", knob: "h-5 w-5", onLeft: "left-5", offLeft: "left-0.5" }
    : { track: "h-5 w-9", knob: "h-4 w-4", onLeft: "left-4", offLeft: "left-0.5" };

  const isDisabled = disabled || busy;

  return (
    <button
      type="button"
      onClick={() => { if (!isDisabled) onChange(!checked); }}
      role="switch"
      aria-checked={checked}
      aria-busy={busy || undefined}
      aria-label={label}
      disabled={isDisabled}
      title={label}
      className={clsx(
        "relative shrink-0 rounded-full border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-60 disabled:cursor-not-allowed",
        dims.track,
        checked ? "bg-accent border-accent" : "bg-fg-muted/30 border-border",
        className,
      )}
    >
      <span
        aria-hidden="true"
        className={clsx(
          "absolute top-0.5 rounded-full bg-bg-subtle shadow-sm transition-all",
          dims.knob,
          checked ? dims.onLeft : dims.offLeft,
        )}
      />
    </button>
  );
}