import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

interface TauriConfig {
  build?: {
    beforeBuildCommand?: string;
    beforeDevCommand?: string;
    devUrl?: string;
    frontendDist?: string;
  };
}

const config = JSON.parse(
  readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
) as TauriConfig;

describe("Tauri frontend lifecycle", () => {
  it("starts and builds the frontend through pnpm", () => {
    expect(config.build).toMatchObject({
      beforeBuildCommand: "pnpm build",
      beforeDevCommand: "pnpm dev",
      devUrl: "http://localhost:1420",
      frontendDist: "../dist",
    });
  });
});
