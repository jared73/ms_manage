// ESLint 9 flat config for ms_manage.
// Plugins: typescript-eslint, react-hooks, react-refresh.
// 规则按"够用、不啰嗦"挑 —— 主要兜住常见 React/TS bug,不开严格美学警告。
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";

export default [
  // 全局忽略:dist / node_modules / Tauri Rust 输出 / 自动生成 / cli 编译产物
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/target/**",
      "src-tauri/gen/**",
      "**/*.d.ts",
      "packages/cli/dist/**", // cli 编译产物,不参与 lint
      "packages/cli/node_modules/**",
    ],
  },

  // 基础 JS 推荐规则
  js.configs.recommended,

  // TS 推荐规则(关闭我们项目里会大量误报的:no-unused-vars、no-explicit-any)
  ...tseslint.configs.recommended,
  {
    rules: {
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/no-explicit-any": "off", // 项目里还有少量 any,先放
      "@typescript-eslint/no-empty-object-type": "off", // 我们有 interface {}
      "@typescript-eslint/ban-ts-comment": "off", // 偶尔需要
      "no-empty": ["warn", { allowEmptyCatch: true }], // catch {} 是允许的
    },
  },

  // React 通用文件:React hook 规则 + react-refresh(防止误导出非组件)
  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    languageOptions: {
      globals: {
        ...globals.browser,
        // Tauri 全局 API 类型由 @tauri-apps/api 提供,但 runtime 全局还在 window 上
        ...globals.node,
      },
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // react-refresh:只有组件文件才允许导出 React 组件
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
      // TS-specific(去掉一些过时 TS 推荐规则)
      "no-undef": "off", // TS 已经管
    },
  },

  // Tauri Rust 工具脚本(只有 lib/ipc.ts 不在此处特殊处理,默认走 src 规则)

  // packages/cli 是 Node CLI(不是 React),给 node 全局 + 不要 react 插件
  {
    files: ["packages/cli/**/*.{ts,mjs,js}"],
    languageOptions: {
      globals: globals.node,
    },
    rules: {
      "react-hooks/rules-of-hooks": "off",
      "react-refresh/only-export-components": "off",
    },
  },
];