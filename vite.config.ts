import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { readFileSync } from "node:fs";

// 从 package.json 读版本号，注入到前端代码里
const pkg = JSON.parse(readFileSync(path.resolve(__dirname, "package.json"), "utf-8"));

// Tauri 期望固定端口、不在启动时打开浏览器
export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  clearScreen: false,
  server: {
    port: 1520,
    // strictPort=true 让端口被占时 vite 直接退出而不是静默漂移，
    // 避免 tauri 按 devUrl 去加载别的服务或空白页。
    strictPort: true,
    host: "127.0.0.1",
    hmr: { protocol: "ws", host: "127.0.0.1", port: 1521 },
    watch: {
      // 避免监听 src-tauri 内部
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome110", "safari15"],
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    outDir: "dist",
    emptyOutDir: true,
    chunkSizeWarningLimit: 600,
    rollupOptions: {
      output: {
        // 把大体积依赖拆成单独 chunk,避免主 chunk > 500KB 警告;
        // 也便于浏览器缓存复用(版本不变就不重新下载这些 chunk)。
        manualChunks: {
          "monaco-editor": ["monaco-editor", "@monaco-editor/react"],
          "react-i18next": ["i18next", "react-i18next"],
          "react-vendor": ["react", "react-dom", "react-router-dom"],
          "tauri-api": [
            "@tauri-apps/api",
            "@tauri-apps/plugin-dialog",
            "@tauri-apps/plugin-fs",
            "@tauri-apps/plugin-http",
            "@tauri-apps/plugin-shell",
            "@tauri-apps/plugin-store",
            "@tauri-apps/plugin-autostart",
          ],
          "ui-vendor": [
            "lucide-react",
            "clsx",
            "tailwind-merge",
            "sonner",
            "dompurify",
            "marked",
            "zustand",
          ],
        },
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
