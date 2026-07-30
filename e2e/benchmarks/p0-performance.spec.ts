import { test, expect } from "../fixtures/backend";
import fs from "fs";
import path from "path";

// ── Output ─────────────────────────────────────────────────────────────────
const BENCHMARK_FILE = "test-results/benchmarks.ndjson";

const BUDGETS = {
  cold_start_ms: 1000,
  selection_to_play_ms: 100,
} as const;

interface BenchmarkResult {
  metric: string;
  value: number;
  unit: string;
  budget: number | null;
  pass: boolean | null;
}

function recordResult(result: BenchmarkResult): void {
  const dir = path.dirname(BENCHMARK_FILE);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.appendFileSync(BENCHMARK_FILE, JSON.stringify(result) + "\n");
  // Write to stdout so CI runners can capture benchmark output
  process.stdout.write(
    `[BENCH] ${result.metric}: ${result.value}${result.unit}` +
      `${result.budget !== null ? ` (budget: ${result.budget}${result.unit})` : ""}` +
      `${result.pass === true ? " PASS" : result.pass === false ? " EXCEEDS" : ""}\n`,
  );
}

// ── Helpers ─────────────────────────────────────────────────────────────────

function generateEntries(
  count: number,
  prefix = "track",
): Array<{ id: string; name: string; kind: string }> {
  return Array.from({ length: count }, (_, i) => ({
    id: `/music/${prefix}-${i}.wav`,
    name: `${prefix}-${i}.wav`,
    kind: "playable",
  }));
}

/**
 * Inject audio-device mock handlers into the page context so they survive
 * navigation.  The fixture's addInitScript creates __TAURI_BACKEND__, and
 * this script (added after) sets the handlers that useAudioDevices needs
 * on mount.
 */
async function injectAudioMocks(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.context().addInitScript(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- mock backend injected by fixture
    const b = (window as any).__TAURI_BACKEND__;
    if (b) {
      b.mockCommand("list_devices", { devices: [] });
      b.mockCommand("current_device", { device: null });
    }
  });
}

/**
 * Emit all entries for a session in bulk via a single page.evaluate call.
 * Avoids IPC round-trip overhead of calling emitEvent per batch.
 */
async function emitBulk(
  page: import("@playwright/test").Page,
  sessionId: string,
  entries: Array<{ id: string; name: string; kind: string }>,
  batchSize = 500,
): Promise<void> {
  await page.evaluate(
    ({ sessionId, entries, batchSize }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- mock backend injected by fixture
      const b = (window as any).__TAURI_BACKEND__;
      for (let i = 0; i < entries.length; i += batchSize) {
        const batch = entries.slice(i, i + batchSize);
        b.emit("browser:folder-chunk", {
          session_id: sessionId,
          entries: batch,
          done: i + batchSize >= entries.length,
        });
      }
    },
    { sessionId, entries, batchSize },
  );
}

/** Navigate, set folder-pick mocks, and click Open Folder. */
async function openFolder(
  page: import("@playwright/test").Page,
  mockCommand: (cmd: string, resp: unknown) => Promise<void>,
  sessionId: string,
): Promise<void> {
  await mockCommand("pick_folder_dialog", { path: "/music" });
  await mockCommand("start_enumeration", { session_id: sessionId });
  await page.getByRole("button", { name: "Open Folder" }).click();
}

// ── Benchmark suite ─────────────────────────────────────────────────────────

test.describe("P0 performance benchmarks", () => {
  // ── 1. Cold start ─────────────────────────────────────────────────────────
  test("cold start", async ({ page }) => {
    // Inject audio mocks before first navigation so useAudioDevices
    // succeeds immediately instead of throwing caught errors.
    await injectAudioMocks(page);

    const start = performance.now();
    await page.goto("/");

    await expect(
      page.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      page.getByRole("button", { name: "Open Folder" }),
    ).toBeVisible({ timeout: 5_000 });

    const elapsed = performance.now() - start;
    recordResult({
      metric: "cold_start_ms",
      value: Math.round(elapsed),
      unit: "ms",
      budget: BUDGETS.cold_start_ms,
      pass: elapsed <= BUDGETS.cold_start_ms ? true : false,
    });
  });

  // ── 2. Enumeration + render throughput ────────────────────────────────────
  test.describe("enumeration render throughput", () => {
    const FILE_COUNTS = [100, 1_000] as const;

    for (const fileCount of FILE_COUNTS) {
      test(`render ${fileCount.toLocaleString("en-US")} files`, async ({
        page,
        mockCommand,
      }) => {
        await injectAudioMocks(page);
        await page.goto("/");
        await expect(
          page.getByRole("button", { name: "Open Folder" }),
        ).toBeVisible();

        await openFolder(page, mockCommand, "enum");

        const entries = generateEntries(fileCount);

        const start = performance.now();
        await emitBulk(page, "enum", entries);

        const grid = page.getByRole("grid", { name: "Playable files" });
        await expect(grid).toBeVisible({ timeout: 15_000 });
        await expect(
          page.getByRole("row", { name: /track-0\.wav/ }),
        ).toBeVisible({ timeout: 15_000 });

        const elapsed = performance.now() - start;
        recordResult({
          metric: `enumeration_render_${fileCount}_files_ms`,
          value: Math.round(elapsed),
          unit: "ms",
          budget: null,
          pass: null,
        });
      });
    }
  });

  // ── 3. Selection-to-play latency ─────────────────────────────────────────
  test("selection to play", async ({
    page,
    mockCommand,
    getCommandCalls,
  }) => {
    await injectAudioMocks(page);
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Open Folder" })).toBeVisible();

    await mockCommand("play", {});
    await openFolder(page, mockCommand, "sel");

    const entries = generateEntries(100);
    await emitBulk(page, "sel", entries);

    await expect(
      page.getByRole("row", { name: /track-0\.wav/ }),
    ).toBeVisible({ timeout: 10_000 });

    // Clear previous command calls
    await page.evaluate("window.__TAURI_BACKEND__._state.calls = []");

    // Measure from browser-side click to play command dispatch
    const elapsed = await page.evaluate(async () => {
      const before = performance.now();
      const row = document.querySelector('[data-row-id="/music/track-0.wav"]');
      if (!row) throw new Error("Row not found");
      (row as HTMLElement).click();

      await new Promise<void>((resolve) => {
        const check = () => {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any -- mock backend
          const calls = (window as any).__TAURI_BACKEND__?.getCalls?.();
          if (calls?.some((c: { command: string }) => c.command === "play")) {
            resolve();
          } else {
            requestAnimationFrame(check);
          }
        };
        check();
      });

      return Math.round(performance.now() - before);
    });

    recordResult({
      metric: "selection_to_play_ms",
      value: elapsed,
      unit: "ms",
      budget: BUDGETS.selection_to_play_ms,
      pass: elapsed <= BUDGETS.selection_to_play_ms ? true : false,
    });

    const calls = await getCommandCalls();
    expect(calls.some((c) => c.command === "play")).toBeTruthy();
  });

  // ── 4. Large-list scroll responsiveness ──────────────────────────────────
  test("large list scroll responsiveness", async ({
    page,
    mockCommand,
  }) => {
    await injectAudioMocks(page);
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Open Folder" })).toBeVisible();

    await openFolder(page, mockCommand, "scroll");

    const entries = generateEntries(100_000);
    await emitBulk(page, "scroll", entries);

    const grid = page.getByRole("grid", { name: "Playable files" });
    await expect(grid).toBeVisible({ timeout: 15_000 });

    await expect(
      page.getByRole("row", { name: /track-0\.wav/ }),
    ).toBeVisible({ timeout: 15_000 });

    // ── Scroll jump times ───────────────────────────────────────────────
    const scrollTargets = [
      { label: "mid", index: 50_000 },
      { label: "near_end", index: 99_999 },
    ] as const;

    for (const target of scrollTargets) {
      const scrollTime = await page.evaluate(
        async ({ index }: { index: number }) => {
          const viewport = document.querySelector(".file-list-viewport");
          if (!viewport) throw new Error("Viewport not found");

          const before = performance.now();
          viewport.scrollTop = index * 32;

          await new Promise<void>((resolve) => {
            let frames = 0;
            const check = () => {
              frames++;
              if (
                document.querySelector(
                  `[data-row-id="/music/track-${index}.wav"]`,
                )
              ) {
                resolve();
              } else if (frames > 300) {
                resolve();
              } else {
                requestAnimationFrame(check);
              }
            };
            requestAnimationFrame(check);
          });

          return Math.round(performance.now() - before);
        },
        { index: target.index },
      );

      recordResult({
        metric: `scroll_to_${target.label}_${target.index}_ms`,
        value: scrollTime,
        unit: "ms",
        budget: null,
        pass: null,
      });
    }

    // ── Scroll FPS ──────────────────────────────────────────────────────
    const scrollFps = await page.evaluate(async () => {
      const viewport = document.querySelector(
        ".file-list-viewport",
      ) as HTMLElement;
      if (!viewport) throw new Error("Viewport not found");

      const frameTimes: number[] = [];
      const step = 500;
      let pos = 0;

      return new Promise<number>((resolve) => {
        const frame = (ts: number) => {
          frameTimes.push(ts);
          pos += step;
          viewport.scrollTop = pos;

          if (pos < 100_000 * 32 * 0.5) {
            requestAnimationFrame(frame);
          } else {
            if (frameTimes.length < 2) {
              resolve(0);
              return;
            }
            const intervals: number[] = [];
            for (let i = 1; i < frameTimes.length; i++) {
              intervals.push(frameTimes[i] - frameTimes[i - 1]);
            }
            const avg =
              intervals.reduce((a: number, b: number) => a + b, 0) /
              intervals.length;
            resolve(Math.round(1000 / avg));
          }
        };
        requestAnimationFrame(frame);
      });
    });

    recordResult({
      metric: "scroll_fps",
      value: scrollFps,
      unit: "fps",
      budget: null,
      pass: null,
    });
  });
});
