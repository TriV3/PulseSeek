import { test, expect } from "../fixtures/backend";

test("folder and file right-clicks open PulseSeek actions", async ({
  page,
  mockCommand,
  emitEvent,
  getCommandCalls,
}) => {
  await page.goto("/");

  const musicFolder = page.getByText("Music", { exact: true });
  await musicFolder.click({ button: "right" });
  const folderMenu = page.getByRole("menu", {
    name: "Folder actions for Music",
  });
  await expect(folderMenu).toBeVisible();
  await folderMenu.getByRole("menuitem", { name: "Bookmark folder" }).click();

  await mockCommand("start_enumeration", { session_id: "context-menu-scan" });
  await musicFolder.click();
  await emitEvent("browser:folder-chunk", {
    session_id: "context-menu-scan",
    entries: [
      { id: "/music/context.wav", name: "context.wav", kind: "playable" },
    ],
    folders_done: true,
    done: true,
  });

  const file = page.getByRole("row", { name: /context\.wav/ });
  await file.click({ button: "right" });
  const fileMenu = page.getByRole("menu", {
    name: "File actions for context.wav",
  });
  await expect(fileMenu.getByRole("menuitem", { name: "Move…" })).toBeVisible();
  await expect(fileMenu.getByRole("menuitem", { name: "Copy…" })).toBeVisible();
  await fileMenu.getByRole("menuitem", { name: "Mark Favorite" }).click();
  await expect(file.getByText("Favorite")).toBeVisible();

  const calls = await getCommandCalls();
  expect(
    calls.some((call) => call.command === "add_folder_bookmark"),
  ).toBeTruthy();
});
