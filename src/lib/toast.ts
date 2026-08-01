/**
 * toast 展示时长的单一来源。
 *
 * - <Toaster> 的全局默认 duration(未显式传 duration 的 toast 生效)
 * - ToastSentinel 的兜底关闭时限(必须与全局默认一致,否则哨兵会按旧值算剩余时间)
 */
export const TOAST_DURATION = 1000;
