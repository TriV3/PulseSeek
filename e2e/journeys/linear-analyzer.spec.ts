import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "../fixtures/backend";

function spectrumMagnitudes(): number[] {
  const magnitudes = Array.from({ length: 2_049 }, () => 0.00004);
  magnitudes[32] = 0.28;
  magnitudes[256] = 0.5;
  magnitudes[768] = 0.85;
  magnitudes[1_280] = 0.65;
  magnitudes[1_920] = 0.35;
  return magnitudes;
}

async function canvasSignature(page: import("@playwright/test").Page) {
  return page.locator("canvas.linear-analyzer-canvas").evaluate((element) => {
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

test.describe("linear analyzer", () => {
  test("switches during playback, renders the latest spectrum, and keeps seek interaction", async ({
    page,
    emitEvent,
    mockCommand,
    getCommandCalls,
  }) => {
    await page.goto("/");
    // The startup splash is decorative and non-blocking (pointer-events: none);
    // interactions and screenshots must target the app, not the transient splash.
    await expect(page.locator("#startup-splash")).not.toBeAttached();
    await mockCommand("start_enumeration", { session_id: "linear-session" });
    await page.getByText("Music", { exact: true }).click();
    await emitEvent("browser:folder-chunk", {
      session_id: "linear-session",
      entries: [
        {
          id: "/music/linear-fixture.wav",
          name: "linear-fixture.wav",
          kind: "playable",
        },
      ],
      folders_done: true,
      done: true,
    });
    await mockCommand("play", {});
    await page.getByRole("row", { name: /linear-fixture\.wav/ }).click();

    const selector = page.getByLabel("Visualization", { exact: true });
    await selector.selectOption("logarithmic");
    await expect(
      page.getByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).toBeVisible();
    const commandsBeforeSwitch = await getCommandCalls();
    const playbackCommandsBeforeSwitch = commandsBeforeSwitch.filter((call) =>
      ["play", "pause", "stop"].includes(call.command),
    );

    await selector.selectOption("linear");
    const canvas = page.getByRole("img", { name: "Linear frequency analyzer" });
    await expect(canvas).toBeVisible();
    await expect(
      page.getByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("slider", { name: "Waveform seek" }),
    ).toHaveCount(0);
    const commandsAfterSwitch = await getCommandCalls();
    expect(
      commandsAfterSwitch.filter((call) =>
        ["play", "pause", "stop"].includes(call.command),
      ),
    ).toEqual(playbackCommandsBeforeSwitch);

    await emitEvent("playback:position", {
      position_ms: 500,
      duration_ms: 2_000,
    });
    const analyzerSeek = page.getByRole("slider", {
      name: "Linear analyzer seek",
    });
    await expect(analyzerSeek).toHaveAttribute("aria-valuenow", "500");
    const seekBounds = await analyzerSeek.boundingBox();
    expect(seekBounds).not.toBeNull();
    const currentMarker = page.locator(
      ".linear-analyzer [data-testid='waveform-current-marker']",
    );
    await expect
      .poll(() =>
        currentMarker.evaluate((element) =>
          parseFloat(element.style.getPropertyValue("--seek-x")),
        ),
      )
      .toBeCloseTo((seekBounds?.width ?? 0) * 0.25, 0);

    await analyzerSeek.hover({
      position: { x: Math.round((seekBounds?.width ?? 0) * 0.6), y: 20 },
    });
    await expect(
      page.locator(".linear-analyzer [data-testid='waveform-hover-marker']"),
    ).toBeVisible();
    await expect(
      page.locator(".linear-analyzer [data-testid='waveform-hover-time']"),
    ).toBeVisible();

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

    const originalWidth = await canvas.evaluate(
      (element) => (element as HTMLCanvasElement).width,
    );
    await page.setViewportSize({ width: 1_000, height: 720 });
    await expect
      .poll(() =>
        canvas.evaluate((element) => (element as HTMLCanvasElement).width),
      )
      .not.toBe(originalWidth);

    const platform = await committedBaselinePlatform(page);
    const baseline = fileURLToPath(
      new URL(
        `linear-analyzer.spec.ts-snapshots/linear-analyzer-${platform}.png`,
        import.meta.url,
      ),
    );
    if (existsSync(baseline) || platform === "darwin") {
      await expect(canvas).toHaveScreenshot("linear-analyzer.png");
    } else {
      test.info().annotations.push({
        type: "note",
        description: `No committed screenshot baseline for ${platform}; visual comparison skipped.`,
      });
    }
  });
});
