/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: {
          DEFAULT: "#0f1115",
          subtle: "#161922",
          elev: "#1c2030",
        },
        fg: {
          DEFAULT: "#e6e8ef",
          muted: "#9aa0b0",
        },
        accent: {
          DEFAULT: "#7c5cff",
          hover: "#6a4cf0",
        },
        ok: "#22c55e",
        warn: "#f59e0b",
        err: "#ef4444",
      },
      fontFamily: {
        sans: ["system-ui", "Microsoft YaHei", "Segoe UI", "sans-serif"],
        mono: ["JetBrains Mono", "Consolas", "monospace"],
      },
    },
  },
  plugins: [],
};
