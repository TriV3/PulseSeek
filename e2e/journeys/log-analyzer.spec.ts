import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "../fixtures/backend";

function spectrumMagnitudes(): number[] {
  const magnitudes = Array.from({ length: 2_049 }, () => 0.00004);
  magnitudes[1] = 0.28;
  magnitudes[3] = 0.5;
  magnitudes[9] = 0.85;
  magnitudes[43] = 0.65;
  magnitudes[171] = 0.35;
  return magnitudes;
}

async function canvasSignature(page: import("@playwright/test").Page) {
  return page.locator("canvas.log-analyzer-canvas").evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    const context = canvas.getContext("2d");
    if (!context) return 0;
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let signature = 0;
    for (let index = 0; index < pixels.length; index += 1) {
      signature =
        (signature + pixels[index] * ((index % 17) + 1)) % 2_147_483_647;
    }
    return signature;
  });
}

async function committedBaselinePlatform(
  page: import("@playwright/test").Page,
) {
  const navigatorPlatform = await page.evaluate(() => navigator.platform);
  if (/Mac/i.test(navigatorPlatform)) return "darwin";
  if (/Win/i.test(navigatorPlatform)) return "win32";
  return "linux";
}

test.describe("logarithmic analyzer", () => {
  test("renders the latest spectrum and stays visible with a long selected name", async ({
    page,
    emitEvent,
    mockCommand,
    getCommandCalls,
  }) => {
    await page.goto("/");
    // The startup splash is decorative and non-blocking (pointer-events: none);
    // interactions and screenshots must target the app, not the transient splash.
    await expect(page.locator("#startup-splash")).not.toBeAttached();
    await mockCommand("start_enumeration", { session_id: "analyzer-session" });
    await page.getByText("Music", { exact: true }).click();
    const longName = `${"Long frequency analyzer fixture ".repeat(8)}.wav`;
    await emitEvent("browser:folder-chunk", {
      session_id: "analyzer-session",
      entries: [{ id: `/music/${longName}`, name: longName, kind: "playable" }],
      folders_done: true,
      done: true,
    });
    await mockCommand("play", {});
    await page
      .getByRole("row", { name: new RegExp(longName.slice(0, 20)) })
      .click();
    await expect(
      page.getByRole("slider", { name: "Waveform seek" }),
    ).toBeVisible();
    await expect(
      page.getByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).toHaveCount(0);
    await page
      .getByLabel("Visualization", { exact: true })
      .selectOption("logarithmic");
    const canvas = page.getByRole("img", {
      name: "Logarithmic frequency analyzer",
    });
    await expect(canvas).toBeVisible();
    await expect(
      page.getByRole("slider", { name: "Log analyzer seek" }),
    ).toBeVisible();
    await emitEvent("playback:position", {
      position_ms: 500,
      duration_ms: 2_000,
    });
    const analyzerSeek = page.getByRole("slider", {
      name: "Log analyzer seek",
    });
    await expect(analyzerSeek).toHaveAttribute("aria-valuenow", "500");
    const seekBounds = await analyzerSeek.boundingBox();
    expect(seekBounds).not.toBeNull();
    const marker = page.locator(
      ".log-analyzer [data-testid='waveform-current-marker']",
    );
    await expect
      .poll(() =>
        marker.evaluate((element) =>
          parseFloat(element.style.getPropertyValue("--seek-x")),
        ),
      )
      .toBeCloseTo((seekBounds?.width ?? 0) * 0.25, 0);

    const seekX = Math.round((seekBounds?.width ?? 0) * 0.75);
    const expectedSeekMs = Math.round(
      (seekX / (seekBounds?.width ?? 1)) * 2_000,
    );
    await analyzerSeek.click({ position: { x: seekX, y: 20 } });
    await expect
      .poll(async () => {
        const calls = await getCommandCalls();
        const payload = calls.findLast((call) => call.command === "seek")
          ?.payload as { position_ms?: number } | undefined;
        return payload?.position_ms;
      })
      .toBe(expectedSeekMs);
    const before = await canvasSignature(page);

    const spectrum = {
      format_version: 1,
      sequence: 3,
      position_frames: 2_048,
      sample_rate: 48_000,
      fft_size: 4_096,
      magnitudes: spectrumMagnitudes(),
    };

    await expect
      .poll(async () => {
        await emitEvent("visualization:spectrum", spectrum);
        return canvasSignature(page);
      })
      .not.toBe(before);
    const firstSpectrum = await canvasSignature(page);
    const movingSpectrum = {
      ...spectrum,
      sequence: 4,
      position_frames: 3_072,
      magnitudes: spectrum.magnitudes.map((magnitude, index) =>
        index % 7 === 0 ? magnitude * 0.15 : magnitude * 1.6,
      ),
    };
    await expect
      .poll(async () => {
        await emitEvent("visualization:spectrum", movingSpectrum);
        return canvasSignature(page);
      })
      .not.toBe(firstSpectrum);
    const originalWidth = await canvas.evaluate(
      (element) => (element as HTMLCanvasElement).width,
    );
    await page.setViewportSize({ width: 1_000, height: 720 });
    await expect
      .poll(() =>
        canvas.evaluate((element) => (element as HTMLCanvasElement).width),
      )
      .not.toBe(originalWidth);
    const bounds = await canvas.boundingBox();
    expect(bounds).not.toBeNull();
    expect((bounds?.x ?? 0) + (bounds?.width ?? 0)).toBeLessThanOrEqual(1_000);

    const platform = await committedBaselinePlatform(page);
    const baseline = fileURLToPath(
      new URL(
        `log-analyzer.spec.ts-snapshots/log-analyzer-${platform}.png`,
        import.meta.url,
      ),
    );
    if (existsSync(baseline)) {
      await expect(canvas).toHaveScreenshot("log-analyzer.png");
    } else {
      test.info().annotations.push({
        type: "note",
        description: `No committed screenshot baseline for ${platform}; visual comparison skipped.`,
      });
    }
  });
});
