import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { Toaster } from "sonner";
import { Layout } from "@/components/Layout";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { Dashboard } from "@/pages/Dashboard";
import { Servers } from "@/pages/Servers";
import { Skills } from "@/pages/Skills";
import { Connections } from "@/pages/Connections";
import { Settings } from "@/pages/Settings";
import { Logs } from "@/pages/Logs";
import { getSettings } from "@/lib/ipc";
import { useApp } from "@/stores/app";

export default function App() {
  const { t } = useTranslation();
  const setSettings = useApp((s) => s.setSettings);
  useEffect(() => {
    void (async () => {
      try {
        const s = await getSettings();
        setSettings(s);
      } catch {
        // 忽略：使用默认主题
      }
    })();
  }, [setSettings]);

  return (
    <ErrorBoundary label={t("error_boundary.app_label")}>
      <HashRouter>
        <Toaster position="top-right" richColors />
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Dashboard />} />
            <Route path="/servers" element={<Servers />} />
            <Route path="/skills" element={<Skills />} />
            <Route path="/connections" element={<Connections />} />
            <Route path="/logs" element={<Logs />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </HashRouter>
    </ErrorBoundary>
  );
}
