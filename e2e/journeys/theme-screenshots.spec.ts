import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "../fixtures/backend";

/**
 * Playwright names golden-file baselines with the runner platform suffix.
 * A baseline is committed for the platform it was generated on; screenshots
 * are compared only when the current platform has a committed baseline so the
 * Quality workflow (ubuntu-latest) never fails on a missing golden file.
 */
async function committedBaselinePlatform(
  page: import("@playwright/test").Page,
) {
  const navigatorPlatform = await page.evaluate(() => navigator.platform);
  if (/Mac/i.test(navigatorPlatform)) return "darwin";
  if (/Win/i.test(navigatorPlatform)) return "win32";
  return "linux";
}

test.describe("theme screenshots", () => {
  test("Midnight Blue applies and renders without falling back", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeAttached();
    // The startup splash is decorative and non-blocking (pointer-events: none);
    // screenshots must capture the app, not the transient splash.
    await expect(page.locator("#startup-splash")).not.toBeAttached();

    // Persisted preference defaults to system; pick Midnight Blue explicitly.
    await page.getByRole("button", { name: "Options" }).click();
    await page.getByLabel("Theme").selectOption("midnight");

    // The active theme must be applied to the document without a restart.
    await expect(page.locator("html")).toHaveAttribute(
      "data-theme",
      "midnight",
    );

    // Deterministic token check: the Midnight Blue values must resolve on the
    // document so no component falls back to another theme. This assertion
    // runs on every platform and does not depend on golden files.
    const resolved = await page.evaluate(() => {
      const style = getComputedStyle(document.documentElement);
      const token = (name: string) =>
        style.getPropertyValue(`--${name}`).trim();
      return {
        canvas: token("bg-canvas"),
        text: token("text"),
        accent: token("accent"),
        surface: token("bg-surface"),
      };
    });
    expect(resolved).toEqual({
      canvas: "#0b1020",
      text: "#a9b7d6",
      accent: "#6aa6f5",
      surface: "#151b30",
    });

    // Visual comparison: the Midnight Blue token set must render identically
    // to the committed baseline. Only runs when the running platform has one.
    const platform = await committedBaselinePlatform(page);
    const baseline = fileURLToPath(
      new URL(
        `theme-screenshots.spec.ts-snapshots/midnight-blue-${platform}.png`,
        import.meta.url,
      ),
    );
    if (existsSync(baseline)) {
      await expect(page).toHaveScreenshot("midnight-blue.png");
    } else {
      test.info().annotations.push({
        type: "note",
        description: `No committed screenshot baseline for ${platform}; visual comparison skipped. Generate one with pnpm test:e2e -- --update-snapshots on ${platform}.`,
      });
    }
  });
});
