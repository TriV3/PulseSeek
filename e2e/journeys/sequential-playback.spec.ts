import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/a.wav", name: "a.wav", kind: "playable" },
  { id: "/music/b.wav", name: "b.wav", kind: "playable" },
] as const;

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
