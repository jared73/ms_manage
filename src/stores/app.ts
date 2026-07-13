import { create } from "zustand";
import type { AppSettings } from "@/lib/ipc";

export type ThemeName = "purple" | "midnight" | "forest" | "light";

interface AppState {
  ready: boolean;
  lang: "zh" | "en";
  theme: ThemeName;
  setReady: (v: boolean) => void;
  setLang: (l: "zh" | "en") => void;
  setTheme: (t: ThemeName) => void;
  settings?: AppSettings;
  setSettings: (s: AppSettings) => void;
}

function applyTheme(t: ThemeName) {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = t;
  }
}

export const useApp = create<AppState>((set) => ({
  ready: false,
  lang: (localStorage.getItem("lang") as "zh" | "en") ?? "zh",
  theme: (localStorage.getItem("theme") as ThemeName) ?? "purple",
  setReady: (v) => set({ ready: v }),
  setLang: (l) => {
    localStorage.setItem("lang", l);
    set({ lang: l });
  },
  setTheme: (t) => {
    localStorage.setItem("theme", t);
    applyTheme(t);
    set({ theme: t });
  },
  setSettings: (s) => {
    // 启动时从后端 settings 同步主题到 DOM
    const t = (s.theme as ThemeName) ?? "purple";
    applyTheme(t);
    localStorage.setItem("theme", t);
    set({ settings: s, theme: t });
  },
}));
