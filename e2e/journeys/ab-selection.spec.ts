import { test, expect } from "../fixtures/backend";

const FILE = {
  id: "/music/track1.wav",
  name: "track1.wav",
  kind: "playable",
} as const;

test.describe("A-B selection markers", () => {
  test("renders markers and the highlight band at the placed positions", async ({
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
      entries: [FILE],
      folders_done: true,
      done: true,
    });
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();

    await mockCommand("play", {});
    await page.getByRole("row", { name: /track1\.wav/ }).click();
    await mockCommand("seek", { position_ms: 500 });
    await emitEvent("playback:position", {
      position_ms: 500,
      duration_ms: 2000,
    });

    // ── After Set A: one ghost marker at 25% ───────────────────────────
    await page.getByRole("button", { name: /Set A point/i }).click();
    await expect(page.getByTestId("waveform-ab-start")).toBeVisible();

    const single = await page.evaluate(() => {
      const canvas = document.querySelector(
        "canvas.waveform-canvas-surface",
      ) as HTMLElement;
      const canvasRect = canvas.getBoundingClientRect();
      const marker = document.querySelector(
        "[data-testid='waveform-ab-start']",
      ) as HTMLElement;
      const markerRect = marker.getBoundingClientRect();
      const band = document.querySelector("[data-testid='waveform-ab-band']");
      return {
        canvasWidth: canvasRect.width,
        markerLeft: markerRect.left - canvasRect.left,
        markerWidth: markerRect.width,
        markerHeight: markerRect.height,
        bandPresent: Boolean(band),
      };
    });
    expect(single.bandPresent).toBe(false);
    // 25% of the canvas width (marker x centered at 25%; allow 1px for the
    // 1px-wide line so the center lands on the boundary).
    const centerX = single.markerLeft + single.markerWidth / 2;
    expect(centerX).toBeGreaterThan(single.canvasWidth * 0.25 - 1);
    expect(centerX).toBeLessThan(single.canvasWidth * 0.25 + 1);
    expect(single.markerHeight).toBeGreaterThan(0);

    // ── After Set B: solid markers + band spanning 25%→75% ─────────────
    await mockCommand("seek", { position_ms: 1500 });
    await mockCommand("set_loop_region", { start_ms: 500 });
    await mockCommand("clear_loop_region", {});
    await emitEvent("playback:position", {
      position_ms: 1500,
      duration_ms: 2000,
    });
    await page.getByRole("button", { name: /Set B point/i }).click();

    await expect(page.getByTestId("waveform-ab-end")).toBeVisible();
    await expect(page.getByTestId("waveform-ab-band")).toBeVisible();

    const region = await page.evaluate(() => {
      const canvas = document.querySelector(
        "canvas.waveform-canvas-surface",
      ) as HTMLElement;
      const canvasRect = canvas.getBoundingClientRect();
      const start = document.querySelector(
        "[data-testid='waveform-ab-start']",
      ) as HTMLElement;
      const end = document.querySelector(
        "[data-testid='waveform-ab-end']",
      ) as HTMLElement;
      const band = document.querySelector(
        "[data-testid='waveform-ab-band']",
      ) as HTMLElement;
      const startRect = start.getBoundingClientRect();
      const endRect = end.getBoundingClientRect();
      const bandRect = band.getBoundingClientRect();
      const startStyle = getComputedStyle(start);
      const bandStyle = getComputedStyle(band);
      return {
        canvasWidth: canvasRect.width,
        startX: startRect.left + startRect.width / 2 - canvasRect.left,
        endX: endRect.left + endRect.width / 2 - canvasRect.left,
        bandLeft: bandRect.left - canvasRect.left,
        bandWidth: bandRect.width,
        bandHeight: bandRect.height,
        startIsPending: startStyle.borderInlineStartStyle === "dashed",
        bandOpacity: bandStyle.opacity,
        bandBackground: bandStyle.backgroundColor,
      };
    });
    expect(region.startX).toBeGreaterThan(region.canvasWidth * 0.25 - 2);
    expect(region.startX).toBeLessThan(region.canvasWidth * 0.25 + 2);
    expect(region.endX).toBeGreaterThan(region.canvasWidth * 0.75 - 2);
    expect(region.endX).toBeLessThan(region.canvasWidth * 0.75 + 2);
    expect(region.bandLeft).toBeGreaterThan(region.canvasWidth * 0.25 - 2);
    expect(region.bandLeft).toBeLessThan(region.canvasWidth * 0.25 + 2);
    expect(region.bandWidth).toBeGreaterThan(region.canvasWidth * 0.5 - 2);
    expect(region.bandWidth).toBeLessThan(region.canvasWidth * 0.5 + 2);
    expect(region.bandHeight).toBeGreaterThan(0);
    expect(region.startIsPending).toBe(false);
    expect(region.bandOpacity).toBe("0.4");
    expect(region.bandBackground).not.toBe("rgba(0, 0, 0, 0)");
  });

  test("places points at the visual playhead without waiting for events", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
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
      entries: [FILE],
      folders_done: true,
      done: true,
    });
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();

    // Select the file. One position event supplies the duration so placement
    // is enabled; the playhead stays at 0 and no further events arrive.
    await mockCommand("play", {});
    await mockCommand("seek", { position_ms: 1500 });
    await mockCommand("set_loop_region", { start_ms: 0 });
    await mockCommand("clear_loop_region", {});
    await page.getByRole("row", { name: /track1\.wav/ }).click();
    await emitEvent("playback:position", { position_ms: 0, duration_ms: 2000 });

    // The waveform reveal animation (280ms) clips the canvas; wait for it to
    // finish so pointer hit-testing reaches the canvas.
    await page.waitForTimeout(400);

    // Place A at the playhead (0ms, left edge).
    await page.getByRole("button", { name: /Set A point/i }).click();
    await expect(page.getByTestId("waveform-ab-start")).toBeVisible();

    // Seek by clicking the waveform at 75% (seek returns immediately, but no
    // position event is emitted) and place B right after.
    const box = await page
      .locator("canvas.waveform-canvas-surface")
      .boundingBox();
    if (!box) throw new Error("waveform canvas missing");
    const hit = await page.evaluate(() => {
      const canvas = document.querySelector(
        "canvas.waveform-canvas-surface",
      ) as HTMLElement;
      const rect = canvas.getBoundingClientRect();
      const el = document.elementFromPoint(
        rect.left + rect.width * 0.75,
        rect.top + rect.height / 2,
      );
      return {
        rectWidth: rect.width,
        rectHeight: rect.height,
        rectTop: rect.top,
        hitTag: el?.tagName ?? null,
        hitCls: el?.className ?? null,
        valueMax: canvas.getAttribute("aria-valuemax"),
      };
    });
    expect(hit.rectHeight).toBeGreaterThan(0);
    expect(hit).toEqual(expect.objectContaining({ hitTag: "CANVAS" }));
    expect(hit.valueMax).toBe("2000");

    await page.mouse.click(box.x + box.width * 0.75, box.y + box.height / 2);

    const callsAfterSeek = await getCommandCalls();
    const seekCalls = callsAfterSeek.filter((call) => call.command === "seek");
    expect(seekCalls.length).toBeGreaterThan(0);

    await page.getByRole("button", { name: /Set B point/i }).click();

    await expect(page.getByTestId("waveform-ab-band")).toBeVisible();

    const geometry = await page.evaluate(() => {
      const canvas = document.querySelector(
        "canvas.waveform-canvas-surface",
      ) as HTMLElement;
      const canvasRect = canvas.getBoundingClientRect();
      const start = document.querySelector(
        "[data-testid='waveform-ab-start']",
      ) as HTMLElement;
      const end = document.querySelector(
        "[data-testid='waveform-ab-end']",
      ) as HTMLElement;
      const band = document.querySelector(
        "[data-testid='waveform-ab-band']",
      ) as HTMLElement;
      const startRect = start.getBoundingClientRect();
      const endRect = end.getBoundingClientRect();
      return {
        canvasWidth: canvasRect.width,
        startX: startRect.left + startRect.width / 2 - canvasRect.left,
        endX: endRect.left + endRect.width / 2 - canvasRect.left,
        bandWidth: band.getBoundingClientRect().width,
      };
    });
    expect(geometry.startX).toBeGreaterThan(-2);
    expect(geometry.startX).toBeLessThan(2);
    expect(geometry.endX).toBeGreaterThan(geometry.canvasWidth * 0.75 - 2);
    expect(geometry.endX).toBeLessThan(geometry.canvasWidth * 0.75 + 2);
    expect(geometry.bandWidth).toBeGreaterThan(geometry.canvasWidth * 0.75 - 2);
  });

  test("drags the B marker to reposition the region", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
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
      entries: [FILE],
      folders_done: true,
      done: true,
    });
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();

    await mockCommand("play", {});
    await mockCommand("seek", { position_ms: 500 });
    await mockCommand("set_loop_region", { start_ms: 500 });
    await mockCommand("clear_loop_region", {});
    await page.getByRole("row", { name: /track1\.wav/ }).click();
    await emitEvent("playback:position", { position_ms: 500, duration_ms: 2000 });

    // Create A=500 (25%) and B=1500 (75%).
    await page.getByRole("button", { name: /Set A point/i }).click();
    await emitEvent("playback:position", {
      position_ms: 1500,
      duration_ms: 2000,
    });
    await page.getByRole("button", { name: /Set B point/i }).click();
    await expect(page.getByTestId("waveform-ab-band")).toBeVisible();

    const before = await page.evaluate(() => {
      const canvas = document.querySelector(
        "canvas.waveform-canvas-surface",
      ) as HTMLElement;
      return canvas.getBoundingClientRect();
    });

    // Drag the B marker from 75% to ~90%.
    const endX = before.left + before.width * 0.9;
    await page.mouse.move(
      before.left + before.width * 0.75,
      before.top + before.height / 2,
    );
    await page.mouse.down();
    await page.mouse.move(endX, before.top + before.height / 2, { steps: 5 });
    await page.mouse.up();

    // The transport re-commits the full pair through setLoopRegion.
    await expect
      .poll(() =>
        getCommandCalls().then((calls) =>
          calls.filter((call) => call.command === "set_loop_region"),
        ),
      )
      .toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            payload: { start_ms: 500, end_ms: 1800 },
          }),
        ]),
      );

    const after = await page.evaluate(() => {
      const canvas = document.querySelector(
        "canvas.waveform-canvas-surface",
      ) as HTMLElement;
      const canvasRect = canvas.getBoundingClientRect();
      const start = document.querySelector(
        "[data-testid='waveform-ab-start']",
      ) as HTMLElement;
      const end = document.querySelector(
        "[data-testid='waveform-ab-end']",
      ) as HTMLElement;
      const band = document.querySelector(
        "[data-testid='waveform-ab-band']",
      ) as HTMLElement;
      const startRect = start.getBoundingClientRect();
      const endRect = end.getBoundingClientRect();
      return {
        startCenter: startRect.left + startRect.width / 2 - canvasRect.left,
        endCenter: endRect.left + endRect.width / 2 - canvasRect.left,
        bandWidth: band.getBoundingClientRect().width,
      };
    });
    expect(after.startCenter).toBeGreaterThan(before.width * 0.25 - 2);
    expect(after.startCenter).toBeLessThan(before.width * 0.25 + 2);
    expect(after.endCenter).toBeGreaterThan(before.width * 0.9 - 4);
    expect(after.endCenter).toBeLessThan(before.width * 0.9 + 4);
    expect(after.bandWidth).toBeGreaterThan(before.width * 0.6);
  });
});
