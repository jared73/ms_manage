import { create } from "zustand";
import type { AppSettings } from "@/lib/ipc";

interface AppState {
  ready: boolean;
  lang: "zh" | "en";
  setReady: (v: boolean) => void;
  setLang: (l: "zh" | "en") => void;
  settings?: AppSettings;
  setSettings: (s: AppSettings) => void;
}

export const useApp = create<AppState>((set) => ({
  ready: false,
  lang: (localStorage.getItem("lang") as "zh" | "en") ?? "zh",
  setReady: (v) => set({ ready: v }),
  setLang: (l) => {
    localStorage.setItem("lang", l);
    set({ lang: l });
  },
  setSettings: (s) => set({ settings: s }),
}));
