import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/track1.wav", name: "track1.wav", kind: "playable" },
] as const;

/** Painted-pixel count for the waveform canvas, for style discrimination. */
async function paintedPixels(page: import("@playwright/test").Page) {
  return page
    .locator("canvas.waveform-canvas-surface")
    .evaluate((el) => {
      const canvas = el as HTMLCanvasElement;
      const ctx = canvas.getContext("2d");
      if (!ctx) return 0;
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let painted = 0;
      for (let i = 3; i < data.length; i += 4) {
        if (data[i] !== 0) painted += 1;
      }
      return painted;
    });
}

async function committedBaselinePlatform(page: import("@playwright/test").Page) {
  const navigatorPlatform = await page.evaluate(() => navigator.platform);
  if (/Mac/i.test(navigatorPlatform)) return "darwin";
  if (/Win/i.test(navigatorPlatform)) return "win32";
  return "linux";
}

test.describe("waveform styles", () => {
  test("selecting a style repaints without refetching waveform data", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeAttached();

    await page.getByText("Computer", { exact: true }).click();
    await expect(page.getByText("Music", { exact: true })).toBeVisible();
    await mockCommand("start_enumeration", { session_id: "session-1" });
    await page.getByText("Music", { exact: true }).click();

    await emitEvent("browser:folder-chunk", {
      session_id: "session-1",
      entries: [...FILES],
      folders_done: true,
      done: true,
    });
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();

    await mockCommand("play", {});
    await page.getByRole("row", { name: /track1\.wav/ }).click();

    const canvas = page.locator("canvas.waveform-canvas-surface");
    await expect(canvas).toBeAttached();
    await expect.poll(() => paintedPixels(page)).toBeGreaterThan(0);

    // The ResizeObserver refetch is debounced (200 ms); let it settle so the
    // baseline count reflects a stable render before style changes.
    await page.waitForTimeout(500);

    const callsAfterLoad = (await getCommandCalls()).filter(
      (call) => call.command === "get_waveform",
    ).length;

    // Each style repaints the canvas without re-requesting waveform data.
    for (const style of ["solid", "gradient", "outline"] as const) {
      await page.getByLabel("Waveform style").selectOption(style);
      await expect.poll(() => paintedPixels(page)).toBeGreaterThan(0);
    }
    await page.waitForTimeout(500);

    const callsAfterStyles = (await getCommandCalls()).filter(
      (call) => call.command === "get_waveform",
    ).length;
    expect(callsAfterStyles).toBe(callsAfterLoad);
  });

  test("screenshots match committed baselines when available", async ({
    page,
    mockCommand,
    emitEvent,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeAttached();

    await page.getByText("Computer", { exact: true }).click();
    await expect(page.getByText("Music", { exact: true })).toBeVisible();
    await mockCommand("start_enumeration", { session_id: "session-1" });
    await page.getByText("Music", { exact: true }).click();

    await emitEvent("browser:folder-chunk", {
      session_id: "session-1",
      entries: [...FILES],
      folders_done: true,
      done: true,
    });
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();

    await mockCommand("play", {});
    await page.getByRole("row", { name: /track1\.wav/ }).click();

    const canvas = page.locator("canvas.waveform-canvas-surface");
    await expect(canvas).toBeAttached();
    await expect.poll(() => paintedPixels(page)).toBeGreaterThan(0);

    const platform = await committedBaselinePlatform(page);
    for (const style of ["solid", "gradient", "outline"] as const) {
      await page.getByLabel("Waveform style").selectOption(style);
      await expect.poll(() => paintedPixels(page)).toBeGreaterThan(0);
      const baseline = fileURLToPath(
        new URL(
          `waveform-styles.spec.ts-snapshots/waveform-${style}-${platform}.png`,
          import.meta.url,
        ),
      );
      if (existsSync(baseline)) {
        await expect(canvas).toHaveScreenshot(`waveform-${style}.png`);
      } else {
        test.info().annotations.push({
          type: "note",
          description: `No committed screenshot baseline for ${platform}; visual comparison skipped. Generate one with pnpm test:e2e -- --update-snapshots on ${platform}.`,
        });
      }
    }
  });
});
