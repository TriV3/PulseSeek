import react from "@vitejs/plugin-react";
import path from "path";
import { defineConfig } from "vite";

// Vite config for Playwright E2E tests.
// Aliases Tauri modules to fake implementations under e2e/mocks/ so the
// app runs in a plain browser without a real Tauri backend.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@tauri-apps/api/core": path.resolve(
        __dirname,
        "e2e/mocks/tauri-core.ts",
      ),
      "@tauri-apps/api/event": path.resolve(
        __dirname,
        "e2e/mocks/tauri-event.ts",
      ),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});
