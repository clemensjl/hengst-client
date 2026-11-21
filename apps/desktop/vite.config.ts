import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { sonnerStaticStyle } from "./vite-plugins/sonnerStaticStyle.ts";

export default defineConfig({
  plugins: [sonnerStaticStyle(), react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "./src") } },
  clearScreen: false,
  // Pre-bundled dependencies skip plugin transforms; Sonner must pass through sonnerStaticStyle.
  optimizeDeps: { exclude: ["sonner"] },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: { environment: "jsdom" },
});
