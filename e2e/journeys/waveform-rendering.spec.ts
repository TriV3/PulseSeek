import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/track1.wav", name: "track1.wav", kind: "playable" },
] as const;

test.describe("waveform rendering", () => {
  test("draws the envelope after file selection", async ({
    page,
    mockCommand,
    emitEvent,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeAttached();

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

    // The canvas must paint the envelope on its own — without needing a
    // resize. This guards the dev StrictMode remount that used to cancel the
    // pending animation frame and freeze every later draw.
    const canvas = page.locator("canvas.waveform-canvas-surface");
    await expect(canvas).toBeAttached();
    await expect
      .poll(() =>
        canvas.evaluate((el) => {
          const c = el as HTMLCanvasElement;
          const ctx = c.getContext("2d");
          if (!ctx) return 0;
          const data = ctx.getImageData(0, 0, c.width, c.height).data;
          let painted = 0;
          for (let i = 3; i < data.length; i += 4) {
            if (data[i] !== 0) painted += 1;
          }
          return painted;
        }),
      )
      .toBeGreaterThan(0);
  });
});
