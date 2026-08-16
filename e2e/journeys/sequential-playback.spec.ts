import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/a.wav", name: "a.wav", kind: "playable" },
  { id: "/music/b.wav", name: "b.wav", kind: "playable" },
] as const;

test("compact player keeps Options and playback mode available", async ({
  page,
  getCommandCalls,
}) => {
  await page.goto("/");
  const options = page.getByRole("button", { name: "Options" });
  const compactToggle = page.getByRole("button", {
    name: "Toggle compact mode",
  });

  await expect(options).toBeVisible();
  await compactToggle.click();
  await expect(compactToggle).toHaveAttribute("aria-pressed", "true");
  await expect(options).toBeVisible();

  await options.click();
  await page.getByLabel("Playback mode").selectOption({ label: "Sequential" });
  await expect
    .poll(async () =>
      (await getCommandCalls()).some(
        (call) =>
          call.command === "set_playback_mode" &&
          (call.payload as { mode?: string }).mode === "sequential",
      ),
    )
    .toBe(true);
});

test("sequential completion starts next visible playable file", async ({
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
    entries: [...FILES],
    folders_done: true,
    done: true,
  });
  await expect(page.getByRole("row", { name: /a\.wav/ })).toBeVisible();
  await expect(page.getByRole("row", { name: /b\.wav/ })).toBeVisible();

  await mockCommand("play", {});
  await page.getByRole("row", { name: /a\.wav/ }).click();
  await expect(page.getByRole("row", { name: /a\.wav/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );

  await mockCommand("set_playback_mode", { mode: "sequential" });
  await page.getByRole("button", { name: "Options" }).click();
  await page.getByLabel("Playback mode").selectOption({ label: "Sequential" });
  await emitEvent("playback:completed", {});

  await expect
    .poll(async () =>
      (await getCommandCalls())
        .filter((call) => call.command === "play")
        .map((call) => call.payload),
    )
    .toEqual([{ path: "/music/a.wav" }, { path: "/music/b.wav" }]);
  await expect(page.getByRole("row", { name: /b\.wav/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("gapless boundary switches waveform without starting a second player", async ({
  page,
  mockCommand,
  emitEvent,
  getCommandCalls,
}) => {
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
  await expect(page.getByRole("row", { name: /a\.wav/ })).toBeVisible();

  await mockCommand("play", {});
  await mockCommand("prepare_next", {});
  await page.getByRole("row", { name: /a\.wav/ }).click();
  await mockCommand("set_playback_mode", { mode: "sequential" });
  await page.getByRole("button", { name: "Options" }).click();
  await page.getByLabel("Playback mode").selectOption({ label: "Sequential" });

  await expect
    .poll(async () =>
      (await getCommandCalls()).some(
        (call) =>
          call.command === "prepare_next" &&
          (call.payload as { path?: string }).path === "/music/b.wav",
      ),
    )
    .toBe(true);
  await emitEvent("playback:track-changed", {
    path: "/music/b.wav",
    duration_ms: 4_000,
  });

  await expect(page.getByRole("row", { name: /b\.wav/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const waveform = page.getByRole("slider", { name: "Waveform seek" });
  await expect(waveform).toHaveAttribute("aria-valuenow", "0");
  await expect(waveform).toHaveAttribute("aria-valuemax", "4000");
  await expect
    .poll(async () =>
      (await getCommandCalls())
        .filter((call) => call.command === "play")
        .map((call) => call.payload),
    )
    .toEqual([{ path: "/music/a.wav" }]);
  await expect
    .poll(async () =>
      (await getCommandCalls()).some(
        (call) =>
          call.command === "get_waveform" &&
          (call.payload as { path?: string }).path === "/music/b.wav",
      ),
    )
    .toBe(true);
});

test("random completion starts another visible playable file", async ({
  page,
  mockCommand,
  emitEvent,
  getCommandCalls,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { level: 1, name: "PulseSeek" }),
  ).toBeAttached();

  await mockCommand("start_enumeration", { session_id: "session-1" });
  await page.getByText("Music", { exact: true }).click();
  await emitEvent("browser:folder-chunk", {
    session_id: "session-1",
    entries: [...FILES],
    folders_done: true,
    done: true,
  });
  await expect(page.getByRole("row", { name: /a\.wav/ })).toBeVisible();
  await expect(page.getByRole("row", { name: /b\.wav/ })).toBeVisible();

  await mockCommand("play", {});
  await page.getByRole("row", { name: /a\.wav/ }).click();
  await mockCommand("set_playback_mode", { mode: "random" });
  await page.getByRole("button", { name: "Options" }).click();
  await page.getByLabel("Playback mode").selectOption({ label: "Random" });
  await emitEvent("playback:completed", {});

  await expect
    .poll(async () =>
      (await getCommandCalls())
        .filter((call) => call.command === "play")
        .map((call) => call.payload),
    )
    .toEqual([{ path: "/music/a.wav" }, { path: "/music/b.wav" }]);
});
