import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/track1.wav", name: "track1.wav", kind: "playable" },
  { id: "/music/track2.wav", name: "track2.wav", kind: "playable" },
  { id: "/music/track3.wav", name: "track3.wav", kind: "playable" },
] as const;

/**
 * The useAudioDevices hook calls listDevices and currentDevice on mount.
 * Without mock responses they throw TypeError, which breaks React's
 * rendering and prevents the FileList loading state from appearing.
 * These two mocks keep the audio subsystem quiet during the test.
 */
async function silenceAudioDevices(
  mockCommand: (cmd: string, resp: unknown) => Promise<void>,
) {
  await mockCommand("list_devices", { devices: [] });
  await mockCommand("current_device", { device: null });
}

test.describe("P0 audition workflow", () => {
  test("folder open, file selection, transport, mode change, and trash", async ({
    page,
    mockCommand,
    emitEvent,
    getCommandCalls,
  }) => {
    // ── 1. App loads ────────────────────────────────────────────────
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Open Folder" }),
    ).toBeVisible();

    // Audio devices are mocked here so their mount-time commands resolve
    // instead of throwing TypeErrors that could interfere with the main
    // workflow below.
    await silenceAudioDevices(mockCommand);

    // ── 2. Open Folder and mock enumeration ──────────────────────────
    await mockCommand("pick_folder_dialog", { path: "/music" });
    await mockCommand("start_enumeration", { session_id: "session-1" });

    await page.getByRole("button", { name: "Open Folder" }).click();

    // Emit enumeration result (already waited for React to process the
    // folder pick and set up the event listener).
    await emitEvent("browser:folder-chunk", {
      session_id: "session-1",
      entries: [...FILES],
      done: true,
    });

    // Verify files appear
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();
    await expect(page.getByRole("row", { name: /track2\.wav/ })).toBeVisible();
    await expect(page.getByRole("row", { name: /track3\.wav/ })).toBeVisible();

    // ── 3. Click file to start playback ──────────────────────────────
    await mockCommand("play", {});

    await page.getByRole("row", { name: /track1\.wav/ }).click();

    // Status label appears on the row; Pause button is visible
    await expect(
      page.getByRole("row", { name: /track1\.wav Playing/ }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();

    // ── 4. Pause playback ────────────────────────────────────────────
    await mockCommand("pause", {});

    await page.getByRole("button", { name: "Pause" }).click();
    await expect(page.getByRole("button", { name: "Play" })).toBeVisible();

    // ── 5. Switch playback mode to Loop ──────────────────────────────
    await mockCommand("set_playback_mode", { mode: "loop-current" });

    await page.getByLabel("Playback mode").selectOption("loop-current");

    // Confirm the select reflects the new mode
    await expect(page.getByLabel("Playback mode")).toHaveValue("loop-current");

    // ── 6. Move a file to Trash ──────────────────────────────────────
    await mockCommand("move_to_trash", {
      results: [{ path: "/music/track2.wav", ok: true }],
    });

    // Select track2
    await page.getByRole("row", { name: /track2\.wav/ }).click();

    // Click toolbar "Move to Trash" button
    await page.getByRole("button", { name: "Move to Trash" }).first().click();

    // Confirm dialog is open
    await expect(page.getByRole("alertdialog")).toBeVisible();

    // Click the confirm button inside the dialog
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "Move to Trash" })
      .click();

    // track2 is gone, track1 and track3 remain
    await expect(
      page.getByRole("row", { name: /track2\.wav/ }),
    ).not.toBeVisible();
    await expect(page.getByRole("row", { name: /track1\.wav/ })).toBeVisible();
    await expect(page.getByRole("row", { name: /track3\.wav/ })).toBeVisible();

    // ── 7. Verify command invocations ────────────────────────────────
    const calls = await getCommandCalls();
    expect(calls.some((c) => c.command === "play")).toBeTruthy();
    expect(calls.some((c) => c.command === "pause")).toBeTruthy();
    expect(calls.some((c) => c.command === "set_playback_mode")).toBeTruthy();
    expect(calls.some((c) => c.command === "move_to_trash")).toBeTruthy();
  });
});
