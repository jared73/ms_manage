import { HashRouter, Routes, Route, Navigate } from "react-router-dom";
import { Toaster } from "sonner";
import { Layout } from "@/components/Layout";
import { Dashboard } from "@/pages/Dashboard";
import { Servers } from "@/pages/Servers";
import { Skills } from "@/pages/Skills";
import { Connections } from "@/pages/Connections";
import { Market } from "@/pages/Market";
import { Settings } from "@/pages/Settings";
import { Logs } from "@/pages/Logs";

export default function App() {
  return (
    <HashRouter>
      <Toaster position="top-right" richColors />
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="/servers" element={<Servers />} />
          <Route path="/skills" element={<Skills />} />
          <Route path="/connections" element={<Connections />} />
          <Route path="/market" element={<Market />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
