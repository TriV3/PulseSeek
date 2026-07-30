import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e/journeys",
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: "http://localhost:1420",
    headless: true,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "pnpm vite --config vite.config.e2e.ts",
    port: 1420,
    reuseExistingServer: !process.env.CI,
    timeout: 15_000,
  },
});
