import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  injectSplashVersion,
  readAppVersion,
  SPLASH_VERSION_PLACEHOLDER,
} from "./versionSource";

describe("application version source", () => {
  it("reads a semantic version from the Tauri package manifest", () => {
    expect(readAppVersion()).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("injects the version into the startup splash markup", () => {
    const html = `<p class="startup-splash__version">v${SPLASH_VERSION_PLACEHOLDER}</p>`;

    expect(injectSplashVersion(html, "1.2.3")).toContain("v1.2.3");
    expect(injectSplashVersion(html, "1.2.3")).not.toContain(
      SPLASH_VERSION_PLACEHOLDER,
    );
  });

  it("rejects splash markup that lacks the version placeholder", () => {
    expect(() => injectSplashVersion("<div></div>", "1.2.3")).toThrow();
  });

  it("keeps the splash placeholder in the shipped HTML document", () => {
    const html = readFileSync(
      path.resolve(
        path.dirname(fileURLToPath(import.meta.url)),
        "../index.html",
      ),
      "utf8",
    );

    expect(html).toContain(SPLASH_VERSION_PLACEHOLDER);
  });
});
