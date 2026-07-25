/**
 * 全局 UI store:目前只剩 settings 同步入口。
 * lang / theme 不再缓存为独立字段 —— 单一真相来自后端 settings,
 * 启动前(后端 settings 还没拉回来)的短暂 fallback 走 localStorage。
 */
import { create } from "zustand";
import type { AppSettings } from "@/lib/ipc";

export type ThemeName = "purple" | "midnight" | "forest" | "light";

interface AppState {
  settings?: AppSettings;
  setSettings: (s: AppSettings) => void;
}

const DEFAULT_THEME: ThemeName = "purple";

function applyTheme(t: ThemeName) {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = t;
  }
}

/** 启动期间(后端 settings 还没回来)从 localStorage 读一份上次的偏好。 */
export function readPersistedUi(): { theme: ThemeName } {
  if (typeof localStorage === "undefined") return { theme: DEFAULT_THEME };
  return {
    theme: (localStorage.getItem("theme") as ThemeName) ?? DEFAULT_THEME,
  };
}

export const useApp = create<AppState>((set) => ({
  settings: undefined,
  setSettings: (s) => {
    const t = (s.theme as ThemeName) ?? DEFAULT_THEME;
    applyTheme(t);
    if (typeof localStorage !== "undefined") localStorage.setItem("theme", t);
    set({ settings: s });
  },
}));

/**
 * 给 Settings 页用:用户改了偏好但还没 update_settings 之前,立刻反映到 DOM。
 * 这是过渡态 —— 后端落盘后再由 setSettings 二次同步兜底。
 */
export function previewTheme(t: ThemeName) {
  applyTheme(t);
  if (typeof localStorage !== "undefined") localStorage.setItem("theme", t);
}
