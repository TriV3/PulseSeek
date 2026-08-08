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

    await mockCommand("drag_out", {});

    // Drag track1 out by dispatching a bubbling dragstart on its row.
    await page.evaluate(() => {
      const row = document.querySelector('[data-row-id="/music/track1.wav"]');
      if (!row) throw new Error("row not found");
      const dt = new DataTransfer();
      const evt = new DragEvent("dragstart", {
        dataTransfer: dt,
        bubbles: true,
        cancelable: true,
      });
      row.dispatchEvent(evt);
    });

    const calls = await getCommandCalls();
    const dragCall = calls.find((c) => c.command === "drag_out");
    expect(dragCall).toBeTruthy();
    expect(dragCall?.payload).toEqual({ paths: ["/music/track1.wav"] });
  });
});
