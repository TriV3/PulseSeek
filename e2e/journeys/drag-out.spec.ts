import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/track1.wav", name: "track1.wav", kind: "playable" },
  { id: "/music/track2.wav", name: "track2.wav", kind: "playable" },
] as const;

test.describe("File drag-out", () => {
  test("on macOS invokes the native drag_out command for the dragged row", async ({
    page,
    context,
    mockCommand,
    emitEvent,
    getCommandCalls,
  }) => {
    // Force macOS platform detection so the native drag-out path is used.
    await context.addInitScript(() => {
      Object.defineProperty(navigator, "platform", {
        value: "MacIntel",
        configurable: true,
      });
    });

    await page.goto("/");
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

    await mockCommand("drag_out", {});

    // The macOS path deliberately avoids HTML dragstart because WKWebView can
    // overwrite the native file pasteboard. Exercise the real gesture instead:
    // press the row, then move far enough to cross the native drag threshold.
    const row = page.locator('[data-row-id="/music/track1.wav"]');
    const box = await row.boundingBox();
    expect(box).not.toBeNull();
    if (!box) throw new Error("row has no bounding box");
    const startX = box.x + box.width / 2;
    const startY = box.y + box.height / 2;
    await page.mouse.move(startX, startY);
    await page.mouse.down();
    await page.mouse.move(startX + 10, startY);
    await page.mouse.up();

    const calls = await getCommandCalls();
    const dragCall = calls.find((c) => c.command === "drag_out");
    expect(dragCall).toBeTruthy();
    expect(dragCall?.payload).toEqual({ paths: ["/music/track1.wav"] });
  });
});
