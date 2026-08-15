import { test, expect } from "../fixtures/backend";

function pathOf(call: { command: string; payload?: unknown }): string | null {
  if (typeof call.payload !== "object" || call.payload === null) return null;
  const value = (call.payload as { path?: unknown }).path;
  return typeof value === "string" ? value : null;
}

test.describe("File drag-in", () => {
  test("plays a dropped audio file and reveals its parent folder", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
  }) => {
    await page.goto("/");
    await expect(page.getByText("Music", { exact: true })).toBeVisible();

    await mockCommand("probe_path", { kind: "playable" });
    await mockCommand("start_enumeration", { session_id: "session-drop" });
    await mockCommand("play", {});

    await emitEvent("tauri://drag-drop", {
      type: "enter",
      paths: ["/music/track1.wav"],
      position: { x: 0, y: 0 },
    });
    await expect(page.getByText("Drop files to play or reveal")).toBeVisible();

    await emitEvent("tauri://drag-drop", {
      type: "drop",
      paths: ["/music/track1.wav"],
      position: { x: 0, y: 0 },
    });
    await expect(
      page.getByText("Drop files to play or reveal"),
    ).toHaveCount(0);

    await expect
      .poll(async () =>
        (await getCommandCalls()).some(
          (call) =>
            call.command === "play" && pathOf(call) === "/music/track1.wav",
        ),
      )
      .toBe(true);
    await expect
      .poll(async () =>
        (await getCommandCalls()).some(
          (call) =>
            call.command === "start_enumeration" && pathOf(call) === "/music",
        ),
      )
      .toBe(true);
  });

  test("reveals a dropped folder without playing", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
  }) => {
    await page.goto("/");
    await expect(page.getByText("Music", { exact: true })).toBeVisible();

    await mockCommand("probe_path", { kind: "directory" });
    await mockCommand("start_enumeration", { session_id: "session-dir" });

    await emitEvent("tauri://drag-drop", {
      type: "drop",
      paths: ["/downloads/project"],
      position: { x: 0, y: 0 },
    });

    await expect
      .poll(async () =>
        (await getCommandCalls()).some(
          (call) =>
            call.command === "start_enumeration" &&
            pathOf(call) === "/downloads/project",
        ),
      )
      .toBe(true);
    const calls = await getCommandCalls();
    expect(calls.some((call) => call.command === "play")).toBe(false);
  });

  test("ignores a non-audio drop", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
  }) => {
    await page.goto("/");
    await expect(page.getByText("Music", { exact: true })).toBeVisible();

    await mockCommand("probe_path", { kind: "unsupported" });

    await emitEvent("tauri://drag-drop", {
      type: "drop",
      paths: ["/notes.txt"],
      position: { x: 0, y: 0 },
    });

    await page.waitForTimeout(200);
    const calls = await getCommandCalls();
    expect(calls.some((call) => call.command === "play")).toBe(false);
    expect(calls.some((call) => call.command === "start_enumeration")).toBe(
      false,
    );
  });
});
