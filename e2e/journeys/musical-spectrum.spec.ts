import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test, expect } from "../fixtures/backend";

function musicalBands() {
  return Array.from({ length: 127 }, (_, index) => {
    const noteNumber = index + 12;
    const center = 440 * 2 ** ((noteNumber - 69) / 12);
    return {
      note_number: noteNumber,
      lower_frequency_hz: center * 2 ** (-1 / 24),
      center_frequency_hz: center,
      upper_frequency_hz: center * 2 ** (1 / 24),
      magnitude:
        noteNumber === 45
          ? 0.45
          : noteNumber === 69
            ? 0.9
            : noteNumber === 72
              ? 0.65
              : 0.00004,
    };
  });
}

async function canvasSignature(page: import("@playwright/test").Page) {
  return page.locator("canvas.musical-spectrum-canvas").evaluate((element) => {
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

test.describe("musical spectrum", () => {
  test("switches during playback, renders pitch bands, and keeps seek interaction", async ({
    page,
    emitEvent,
    mockCommand,
    getCommandCalls,
  }) => {
    await page.goto("/");
    await mockCommand("start_enumeration", { session_id: "musical-session" });
    await page.getByText("Music", { exact: true }).click();
    await emitEvent("browser:folder-chunk", {
      session_id: "musical-session",
      entries: [
        {
          id: "/music/a4-fixture.wav",
          name: "a4-fixture.wav",
          kind: "playable",
        },
      ],
      folders_done: true,
      done: true,
    });
    await mockCommand("play", {});
    await page.getByRole("row", { name: /a4-fixture\.wav/ }).click();

    const selector = page.getByLabel("Visualization", { exact: true });
    await selector.selectOption("linear");
    const playbackCommandsBeforeSwitch = (await getCommandCalls()).filter(
      (call) => ["play", "pause", "stop"].includes(call.command),
    );

    await selector.selectOption("musical");
    const canvas = page.getByRole("img", { name: "Musical spectrum" });
    await expect(canvas).toBeVisible();
    await expect(
      page.getByRole("img", { name: "Linear frequency analyzer" }),
    ).toHaveCount(0);
    expect(
      (await getCommandCalls()).filter((call) =>
        ["play", "pause", "stop"].includes(call.command),
      ),
    ).toEqual(playbackCommandsBeforeSwitch);

    await emitEvent("playback:position", {
      position_ms: 500,
      duration_ms: 2_000,
    });
    const analyzerSeek = page.getByRole("slider", {
      name: "Musical spectrum seek",
    });
    await expect(analyzerSeek).toHaveAttribute("aria-valuenow", "500");
    const seekBounds = await analyzerSeek.boundingBox();
    expect(seekBounds).not.toBeNull();
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
    await expect
      .poll(async () => {
        await emitEvent("visualization:musical-spectrum", {
          format_version: 1,
          sequence: 3,
          position_frames: 2_048,
          sample_rate: 48_000,
          tuning_reference_hz: 440,
          bands: musicalBands(),
        });
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
        `musical-spectrum.spec.ts-snapshots/musical-spectrum-${platform}.png`,
        import.meta.url,
      ),
    );
    if (existsSync(baseline) || platform === "darwin") {
      await expect(canvas).toHaveScreenshot("musical-spectrum.png");
    } else {
      test.info().annotations.push({
        type: "note",
        description: `No committed screenshot baseline for ${platform}; visual comparison skipped.`,
      });
    }
  });
});
