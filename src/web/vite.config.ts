/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// 開發時把 /api、收資料的 /collect 和追蹤程式 /t.js 轉給後端；
// 後端預設在 8585（CDP_BIND），要換 port 就設 CDP_API_URL。
const apiTarget = process.env.CDP_API_URL ?? "http://127.0.0.1:8585";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      "/api": apiTarget,
      "/collect": apiTarget,
      "/t.js": apiTarget,
    },
  },
  build: {
    outDir: "dist",
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
