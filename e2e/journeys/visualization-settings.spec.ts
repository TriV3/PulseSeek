import { test, expect } from "../fixtures/backend";

test.describe("visualization settings", () => {
  test("selects quality, disables analyzer work, and persists through the backend", async ({
    page,
    getCommandCalls,
  }) => {
    await page.goto("/");

    await page
      .getByLabel("Visualization", { exact: true })
      .selectOption("linear");
    await expect(
      page.getByRole("img", { name: "Linear frequency analyzer" }),
    ).toBeVisible();
    await page.getByLabel("Open application menu").click();
    await page.getByLabel("Visualization quality").selectOption("high");
    await page
      .getByRole("checkbox", {
        name: "Real-time visualizations",
      })
      .uncheck();

    await expect(
      page.getByRole("slider", { name: "Waveform seek" }),
    ).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);

    const saves = (await getCommandCalls()).filter(
      (call) => call.command === "save_visualization_settings",
    );
    expect(saves.at(-1)?.payload).toMatchObject({
      settings: { enabled: false, mode: "linear", quality: "high" },
      reducedMotion: false,
    });
  });

  test("honors reduced motion without erasing the selected analyzer", async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/");
    await page
      .getByLabel("Visualization", { exact: true })
      .selectOption("musical");
    await page.getByLabel("Open application menu").click();

    await expect(
      page.getByRole("slider", { name: "Waveform seek" }),
    ).toBeVisible();
    await expect(page.getByText(/Reduced motion is active/)).toBeVisible();
    await expect(page.getByLabel("Visualization", { exact: true })).toHaveValue(
      "musical",
    );
  });
});
