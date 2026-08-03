import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { Toaster } from "sonner";
import { Layout } from "@/components/Layout";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { ToastSentinel } from "@/components/ToastSentinel";
import { TOAST_DURATION } from "@/lib/toast";
import { Dashboard } from "@/pages/Dashboard";
import { Servers } from "@/pages/Servers";
import { Skills } from "@/pages/Skills";
import { Memories } from "@/pages/Memories";
import { Connections } from "@/pages/Connections";
import { Settings } from "@/pages/Settings";
import { Logs } from "@/pages/Logs";
import { Usage } from "@/pages/Usage";
import { getSettings } from "@/lib/ipc";
import { useApp } from "@/stores/app";

export default function App() {
  const { t, i18n } = useTranslation();
  const setSettings = useApp((s) => s.setSettings);
  useEffect(() => {
    void (async () => {
      try {
        const s = await getSettings();
        setSettings(s);
        // 从后端 settings 同步语言偏好到 i18n（防止刷新页面回退到默认语言）
        // i18n 实例稳定，故意不加进依赖数组，避免 effect 反复触发
        if (s.language && s.language !== i18n.language) {
          void i18n.changeLanguage(s.language);
        }
      } catch {
        // 忽略：使用默认主题
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- i18n is stable
  }, [setSettings]);

  return (
    <ErrorBoundary label={t("error_boundary.app_label")}>
      <HashRouter>
        <Toaster position="top-right" richColors duration={TOAST_DURATION} />
        <ToastSentinel />
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Dashboard />} />
            <Route path="/servers" element={<Servers />} />
            <Route path="/skills" element={<Skills />} />
            <Route path="/memories" element={<Memories />} />
            <Route path="/connections" element={<Connections />} />
            <Route path="/logs" element={<Logs />} />
            <Route path="/usage" element={<Usage />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </HashRouter>
    </ErrorBoundary>
  );
}
