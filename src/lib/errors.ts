/**
 * 把任意 catch 到的值变成用户可读的字符串。
 *
 * 后端的 `Result<_, String>`、Tauri 的 IPCError、原生 Error、纯字符串都应该走它,
 * 避免用户在 toast 里看到 `启动 xx 失败: Error: spawn npx ENOENT`(dev 模式 double-wrap)。
 *
 * 用法:
 *   catch (e) { toast.error(`...: ${describeError(e)}`); }
 */
export function describeError(e: unknown): string {
  if (e == null) return "未知错误";
  if (typeof e === "string") return e;
  if (e instanceof Error) {
    // Tauri IPCError / 标准 Error 都有 .message
    if (typeof (e as { message?: unknown }).message === "string") {
      const msg = (e as { message: string }).message;
      if (msg) return msg;
    }
    return e.message || "未知错误";
  }
  if (typeof e === "object" && e !== null) {
    const obj = e as Record<string, unknown>;
    if (typeof obj.message === "string") return obj.message;
    if (typeof obj.error === "string") return obj.error;
    if (typeof obj.toString === "function") {
      try {
        const s = (obj as { toString: () => string }).toString();
        if (s && s !== "[object Object]") return s;
      } catch {
        return "未知错误";
      }
    }
    try {
      return JSON.stringify(e);
    } catch {
      return "未知错误";
    }
  }
  return String(e);
}
