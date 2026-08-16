import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

/**
 * Build-time helpers that keep the application version in the Rust package
 * manifest as the single source of truth.
 *
 * The Tauri binary derives its bundle version from `src-tauri/Cargo.toml`, so
 * the renderer and the startup splash must display the exact same value. These
 * helpers are only imported by Vite configuration and tests; the renderer
 * bundle never ships them.
 */

export const SPLASH_VERSION_PLACEHOLDER = "{{APP_VERSION}}";

const PACKAGE_SECTION = /\[package\]\s*\n([\s\S]*?)(?=\n\[|\s*$)/;
const VERSION_FIELD = /^\s*version\s*=\s*"([^"]+)"/m;

/** Absolute path of the Tauri package manifest. */
function tauriCargoPath(): string {
  return path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../src-tauri/Cargo.toml",
  );
}

/** Reads the semantic version from the `[package]` section of a Cargo manifest. */
export function readAppVersion(
  cargoTomlPath: string = tauriCargoPath(),
): string {
  const source = readFileSync(cargoTomlPath, "utf8");
  const packageSection = source.match(PACKAGE_SECTION);

  if (!packageSection) {
    throw new Error(`No [package] section found in ${cargoTomlPath}`);
  }

  const versionMatch = packageSection[1].match(VERSION_FIELD);

  if (!versionMatch) {
    throw new Error(`No version field found in [package] of ${cargoTomlPath}`);
  }

  const version = versionMatch[1];

  if (!/^\d+\.\d+\.\d+/.test(version)) {
    throw new Error(
      `Package version ${JSON.stringify(version)} in ${cargoTomlPath} is not semantic`,
    );
  }

  return version;
}

/**
 * Replaces the splash version placeholder inside the startup HTML document.
 * Throws when the placeholder is missing so a forgotten placeholder cannot
 * silently ship a versionless splash.
 */
export function injectSplashVersion(html: string, version: string): string {
  if (!html.includes(SPLASH_VERSION_PLACEHOLDER)) {
    throw new Error(
      `Startup splash is missing the ${SPLASH_VERSION_PLACEHOLDER} placeholder`,
    );
  }

  return html.split(SPLASH_VERSION_PLACEHOLDER).join(version);
}
