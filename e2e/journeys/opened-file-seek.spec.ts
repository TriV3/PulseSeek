import { test, expect } from "../fixtures/backend";

/** Painted-pixel count for the waveform canvas; the first paint happens only
 * after the duration ref is set, so it is the reliable "seekable" signal. */
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

// Reproduction: OS-opened file then click the waveform to seek. The persisted
// preferences claim the opened file was the last-played one, which used to
// make the restore flow reset the active session to "idle" and silently break
// seek until the user re-selected the track in the list.
test("opened file waveform click seeks playback", async ({
  page,
  mockCommand,
  emitEvent,
  getCommandCalls,
}) => {
  await page.addInitScript(() => {
    const backend = (window as never as {
      __TAURI_BACKEND__?: {
        _state: {
          commandHandlers: Record<string, () => unknown>;
        };
      };
    }).__TAURI_BACKEND__;
    if (backend) {
      backend._state.commandHandlers.load_player_preferences = () => ({
        version: 1,
        preferences: {
          schema_version: 1,
          revision: 0,
          playback_mode: "one-shot",
          output_device_id: null,
          volume: 1,
          muted: false,
          waveform_size: 38,
          browser_size: 24,
          selected_folder_path: null,
          expanded_folder_paths: [],
          last_played_file_path: "/music/track1.wav",
          last_played_position_ms: 30_000,
          last_played_duration_ms: 60_000,
          theme: "system",
          waveform_style: "outline",
          seek_step_mode: "auto",
          show_hidden_folders: false,
        },
      });
    }
  });

  await page.goto("/");
  await expect(
    page.getByRole("heading", { level: 1, name: "PulseSeek" }),
  ).toBeAttached();

  // Warm open: the OS asks the running app to open one audio file.
  await mockCommand("probe_path", { kind: "playable" });
  await mockCommand("play", {});
  await mockCommand("get_waveform", {
    format_version: 1,
    channels: 1,
    samples_per_peak: 64,
    min: Array.from({ length: 96 }, (_, i) => Math.sin(i / 6) * 0.5 - 0.2),
    max: Array.from({ length: 96 }, (_, i) => Math.sin(i / 6) * 0.5 + 0.2),
  });
  await mockCommand("start_enumeration", { session_id: "open-session" });
  await emitEvent("browser:opened-files", { paths: ["/music/track1.wav"] });

  // The parent folder enumeration delivers the file into the list.
  await emitEvent("browser:folder-chunk", {
    session_id: "open-session",
    entries: [
      {
        id: "/music/track1.wav",
        name: "track1.wav",
        kind: "playable",
        metadata: {
          duration_ms: 60_000,
          size_bytes: 2048,
          modified_at_ms: 1_700_000_000_000,
          channels: 2,
          sample_rate: 44_100,
          bit_depth: 16,
          codec: "wav",
        },
      },
    ],
    folders_done: true,
    done: true,
  });

  // The opened file must start playing.
  await expect
    .poll(async () =>
      (await getCommandCalls()).some(
        (call) =>
          call.command === "play" &&
          (call.payload as { path?: string }).path === "/music/track1.wav",
      ),
    )
    .toBe(true);

  const canvas = page.locator("canvas.waveform-canvas-surface");
  await expect(canvas).toBeAttached();
  // The waveform only becomes seekable once the duration is known and the
  // canvas painted (drawing reads the duration ref); wait for that signal so
  // the click always maps to a real position, even under CI load.
  await expect.poll(() => paintedPixels(page)).toBeGreaterThan(0);
  await page.waitForTimeout(300);

  // Click at 75% of the waveform → expect a seek near 45s. The relative
  // position is measured fresh at click time to survive any late layout shift.
  const box = (await canvas.boundingBox())!;
  await canvas.click({
    position: { x: box.width * 0.75, y: box.height / 2 },
  });

  await expect
    .poll(async () =>
      (await getCommandCalls()).some(
        (call) =>
          call.command === "seek" &&
          (call.payload as { position_ms?: number }).position_ms === 45_000,
      ),
    )
    .toBe(true);
});
