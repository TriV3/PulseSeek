import { test, expect } from "../fixtures/backend";

const FILES = [
  { id: "/music/track1.wav", name: "track1.wav", kind: "playable" },
  { id: "/music/track2.wav", name: "track2.wav", kind: "playable" },
] as const;

test.describe("High Contrast accessibility", () => {
  test("keeps keyboard focus visible with the High Contrast token set", async ({
    page,
    mockCommand,
    emitEvent,
  }) => {
    // ── 1. App loads and a folder is enumerated ──────────────────────
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

    // ── 2. Switch to High Contrast ──────────────────────────────────
    await page.getByLabel("Open application menu").click();
    await page.getByLabel("Theme").selectOption("high-contrast");
    await expect(page.locator("html")).toHaveAttribute(
      "data-theme",
      "high-contrast",
    );
    await page.getByLabel("Open application menu").click();

    // ── 3. Deterministic token check ────────────────────────────────
    const resolved = await page.evaluate(() => {
      const style = getComputedStyle(document.documentElement);
      const token = (name: string) =>
        style.getPropertyValue(`--${name}`).trim();
      return {
        canvas: token("bg-canvas"),
        text: token("text"),
        accent: token("accent"),
        focusRing: token("focus-ring"),
      };
    });
    expect(resolved).toEqual({
      canvas: "#000000",
      text: "#ffffff",
      accent: "#ffff00",
      focusRing: "#ffff00",
    });

    // ── 4. Keyboard focus visibility on a file row ──────────────────
    await mockCommand("play", {});
    await page.getByRole("row", { name: /track1\.wav/ }).click();
    await page.getByRole("row", { name: /track1\.wav/ }).focus();

    const outline = await page.evaluate(() => {
      const style = getComputedStyle(document.activeElement as HTMLElement);
      return {
        color: style.outlineColor,
        width: style.outlineWidth,
        style: style.outlineStyle,
      };
    });

    // The High Contrast focus ring is a bright 2px solid outline so the
    // keyboard position stays visible on the black canvas.
    expect(outline).toEqual({
      color: "rgb(255, 255, 0)",
      width: "2px",
      style: "solid",
    });
  });
});
