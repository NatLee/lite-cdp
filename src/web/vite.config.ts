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
    // Preserve the browser Host so the API same-origin guard accepts dev writes.
    proxy: {
      "/oauth": { target: apiTarget, changeOrigin: false },
      "/.well-known": { target: apiTarget, changeOrigin: false },
      "/mcp": { target: apiTarget, changeOrigin: false },
      "/api": { target: apiTarget, changeOrigin: false },
      "/collect": { target: apiTarget, changeOrigin: false },
      "/t.js": { target: apiTarget, changeOrigin: false },
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
