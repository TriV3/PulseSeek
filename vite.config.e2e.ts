import react from "@vitejs/plugin-react";
import path from "path";
import { defineConfig, type Plugin } from "vite";
import { injectSplashVersion, readAppVersion } from "./src/versionSource";

// Vite config for Playwright E2E tests.
// Aliases Tauri modules to fake implementations under e2e/mocks/ so the
// app runs in a plain browser without a real Tauri backend.
//
// The version define and splash injection mirror vite.config.ts: the e2e app
// must boot with the same build-time constants as the production renderer.
const appVersion = readAppVersion();

/** Injects the application version into the startup splash markup. */
function splashVersionPlugin(): Plugin {
  return {
    name: "pulseseek:splash-version",
    transformIndexHtml(html) {
      return injectSplashVersion(html, appVersion);
    },
  };
}

export default defineConfig({
  plugins: [react(), splashVersionPlugin()],
  define: {
    __PULSESEEK_VERSION__: JSON.stringify(appVersion),
  },
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
      "@tauri-apps/api/webview": path.resolve(
        __dirname,
        "e2e/mocks/tauri-webview.ts",
      ),
      "@tauri-apps/api/window": path.resolve(
        __dirname,
        "e2e/mocks/tauri-window.ts",
      ),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});
