import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

interface TauriConfig {
  version?: string;
  build?: {
    beforeBuildCommand?: string;
    beforeDevCommand?: string;
    devUrl?: string;
    frontendDist?: string;
  };
  bundle?: {
    icon?: string[];
    fileAssociations?: Array<{
      ext?: string[];
      name?: string;
      role?: string;
      mimeType?: string;
    }>;
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

  it("does not pin a bundle version so Tauri inherits the Cargo package version", () => {
    expect(config.version).toBeUndefined();
  });

  it("declares the generated desktop application icons", () => {
    expect(config.bundle?.icon).toEqual([
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico",
    ]);
  });

  it("declares file associations for every decodable audio format", () => {
    const associations = config.bundle?.fileAssociations ?? [];
    const extensions = associations
      .flatMap((association) => association.ext ?? [])
      .sort();
    expect(extensions).toEqual(
      ["aif", "aiff", "flac", "m4a", "mp3", "oga", "ogg", "wav", "wave"].sort(),
    );
    expect(
      associations.every((association) => association.role === "Viewer"),
    ).toBe(true);
    expect(
      associations.every((association) =>
        String(association.mimeType ?? "").startsWith("audio/"),
      ),
    ).toBe(true);
  });
});
