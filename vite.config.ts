import { defineConfig } from "vitest/config";
import type { Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { injectSplashVersion, readAppVersion } from "./src/versionSource";

const host = process.env.TAURI_DEV_HOST;

// The Rust package manifest is the single source of truth for the application
// version. Read it once at configuration time so the startup splash and the
// renderer bundle both display the version the Tauri binary is built with.
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
  envPrefix: ["VITE_", "TAURI_ENV_"],
  clearScreen: false,
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    exclude: ["e2e/**", "node_modules/**"],
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
