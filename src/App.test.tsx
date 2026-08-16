import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { DEFAULT_PLAYER_PREFERENCES } from "./hooks/usePlayerPreferences";
import type { PlayerPreferences } from "./api/commandEnvelope";
import { DEFAULT_SHORTCUTS } from "./shortcuts/keyboardShortcuts";

const shortcutProfile = (bindings = DEFAULT_SHORTCUTS) => ({
  mappings: Object.entries(bindings)
    .filter((entry): entry is [string, NonNullable<(typeof entry)[1]>] =>
      Boolean(entry[1]),
    )
    .map(([action_id, binding]) => ({ action_id, ...binding })),
  unavailable_action_ids: [],
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: () => Promise.resolve(() => {}),
  }),
}));

const { mockSetSize, mockIsMaximized, mockInnerSize, mockScaleFactor } =
  vi.hoisted(() => ({
    mockSetSize: vi.fn(async () => {}),
    mockIsMaximized: vi.fn(async () => false),
    mockInnerSize: vi.fn(async () => ({ width: 1200, height: 800 })),
    mockScaleFactor: vi.fn(async () => 1),
  }));

const { resizeHandlers, mockOnResized } = vi.hoisted(() => {
  const resizeHandlers: Array<(event: unknown) => void> = [];
  return {
    resizeHandlers,
    mockOnResized: vi.fn(async (handler: (event: unknown) => void) => {
      resizeHandlers.push(handler);
      return () => {};
    }),
  };
});

const { mockSetMinSize } = vi.hoisted(() => ({
  mockSetMinSize: vi.fn(async () => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: mockIsMaximized,
    innerSize: mockInnerSize,
    scaleFactor: mockScaleFactor,
    setSize: mockSetSize,
    setMinSize: mockSetMinSize,
    onResized: mockOnResized,
  }),
  LogicalSize: class {
    width: number;
    height: number;
    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  resizeHandlers.length = 0;
  mockInnerSize.mockResolvedValue({ width: 1200, height: 800 });
  mockIsMaximized.mockResolvedValue(false);
  mockSetSize.mockResolvedValue(undefined);
  mockSetMinSize.mockResolvedValue(undefined);
  vi.mocked(invoke).mockReset();
});

describe("application shell", () => {
  it("suppresses the webview's native context menu", () => {
    const { container } = render(<App />);

    expect(fireEvent.contextMenu(container.querySelector("main")!)).toBe(false);
  });

  it("closes the application menu when clicking outside it", () => {
    render(<App />);
    const menuButton = screen.getByRole("button", { name: "Options" });

    fireEvent.click(menuButton);
    expect(menuButton).toHaveAttribute("aria-expanded", "true");

    fireEvent.pointerDown(document.body);
    expect(menuButton).toHaveAttribute("aria-expanded", "false");
  });

  it("closes Options with Escape and restores trigger focus", () => {
    render(<App />);
    const menuButton = screen.getByRole("button", { name: "Options" });

    fireEvent.click(menuButton);
    fireEvent.keyDown(document, { key: "Escape" });

    expect(menuButton).toHaveAttribute("aria-expanded", "false");
    expect(menuButton).toHaveFocus();
  });

  it("confirms before clearing waveform cache", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Options" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Clear waveform cache" }),
    );

    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Clear cache" }),
    ).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("clear_waveform_cache");
  });

  it("renders the folder tree with an accessible heading", () => {
    render(<App />);

    // The heading is visually hidden but present for screen readers.
    expect(
      screen.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeInTheDocument();

    expect(
      screen.queryByRole("button", { name: "Open Folder" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "File list" }),
    ).toBeInTheDocument();
  });

  it("presents the player workspace regions", () => {
    render(<App />);

    expect(
      screen.getByRole("region", { name: "Audio visualization" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("toolbar", { name: "Playback controls" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Browser" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: "Recent folders" })).toHaveAttribute(
      "aria-selected",
      "false",
    );
    expect(screen.queryByRole("tab", { name: "File list" })).toBeNull();
  });

  it("shares the sidebar between keyboard-accessible browser tabs", () => {
    render(<App />);

    const browserTab = screen.getByRole("tab", { name: "Browser" });
    const bookmarksTab = screen.getByRole("tab", { name: "Bookmarks" });
    const recentTab = screen.getByRole("tab", { name: "Recent folders" });
    expect(screen.getByRole("tabpanel", { name: "Browser" })).toBeVisible();

    fireEvent.click(recentTab);
    expect(recentTab).toHaveAttribute("aria-selected", "true");
    expect(
      screen.getByRole("tabpanel", { name: "Recent folders" }),
    ).toBeVisible();

    fireEvent.keyDown(recentTab, { key: "ArrowLeft" });
    expect(bookmarksTab).toHaveFocus();
    fireEvent.keyDown(bookmarksTab, { key: "ArrowLeft" });
    expect(browserTab).toHaveFocus();
    expect(browserTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Browser" })).toBeVisible();
  });

  it("exposes the waveform as a seek slider that is disabled without a file", () => {
    render(<App />);

    const slider = screen.getByRole("slider", { name: "Waveform seek" });
    expect(slider).toHaveAttribute("aria-disabled", "true");
    expect(slider).toHaveAttribute("tabindex", "-1");
  });

  it("lets keyboard users resize both workspace splits", () => {
    render(<App />);

    const waveformSeparator = screen.getByRole("separator", {
      name: "Resize visualization",
    });
    const browserSeparator = screen.getByRole("separator", {
      name: "Resize browser",
    });

    expect(waveformSeparator).toHaveAttribute("aria-valuenow", "38");
    fireEvent.keyDown(waveformSeparator, { key: "ArrowDown" });
    expect(waveformSeparator).toHaveAttribute("aria-valuenow", "40");

    expect(browserSeparator).toHaveAttribute("aria-valuenow", "24");
    fireEvent.keyDown(browserSeparator, { key: "ArrowRight" });
    expect(browserSeparator).toHaveAttribute("aria-valuenow", "26");
  });

  it("applies the persisted theme without restart", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: { ...DEFAULT_PLAYER_PREFERENCES, theme: "dark" },
        };
      }
      return undefined;
    });

    render(<App />);

    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe("dark"),
    );
  });

  it("applies the persisted midnight theme without restart", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: { ...DEFAULT_PLAYER_PREFERENCES, theme: "midnight" },
        };
      }
      return undefined;
    });

    render(<App />);

    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe("midnight"),
    );
  });

  it("applies the persisted high-contrast theme without restart", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: {
            ...DEFAULT_PLAYER_PREFERENCES,
            theme: "high-contrast",
          },
        };
      }
      return undefined;
    });

    render(<App />);

    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe("high-contrast"),
    );
  });

  it("selects and persists a waveform style", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
      }
      if (command === "save_player_preferences") {
        const preferences = (
          args as {
            preferences: PlayerPreferences;
          }
        ).preferences;
        return { version: 1, preferences };
      }
      return undefined;
    });

    render(<App />);

    await waitFor(() =>
      expect(screen.getByLabelText("Waveform style")).toHaveValue("outline"),
    );

    fireEvent.change(screen.getByLabelText("Waveform style"), {
      target: { value: "gradient" },
    });

    await waitFor(() =>
      expect(screen.getByLabelText("Waveform style")).toHaveValue("gradient"),
    );

    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(
          ([command, args]) =>
            command === "save_player_preferences" &&
            (args as { preferences: { waveform_style?: string } }).preferences
              ?.waveform_style === "gradient",
        ),
    ).toBe(true);
  });

  it("switches exclusively between waveform and both frequency analyzers", () => {
    render(<App />);

    const selector = screen.getByLabelText("Visualization");
    expect(selector).toHaveValue("waveform");
    expect(
      screen.getByRole("slider", { name: "Waveform seek" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).not.toBeInTheDocument();

    fireEvent.change(selector, { target: { value: "logarithmic" } });

    expect(
      screen.getByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("slider", { name: "Waveform seek" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Waveform style")).not.toBeInTheDocument();

    fireEvent.change(selector, { target: { value: "linear" } });

    expect(
      screen.getByRole("img", { name: "Linear frequency analyzer" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("slider", { name: "Linear analyzer seek" }),
    ).toBeInTheDocument();

    fireEvent.change(selector, { target: { value: "musical" } });

    expect(
      screen.getByRole("img", { name: "Musical spectrum" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Linear frequency analyzer" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("slider", { name: "Musical spectrum seek" }),
    ).toBeInTheDocument();
  });
});

describe("compact mode", () => {
  function mockBackend() {
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
      }
      if (command === "save_player_preferences") {
        const preferences = (args as { preferences: PlayerPreferences })
          .preferences;
        return { version: 1, preferences };
      }
      return undefined;
    });
  }

  it("keeps Options and playback mode available while shrinking the layout", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toHaveAttribute("aria-pressed", "false"),
    );
    const options = screen.getByRole("button", { name: "Options" });
    expect(options.closest(".now-playing-actions")).toContainElement(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 440, height: 600 }),
      ),
    );
    expect(
      screen.queryByRole("tab", { name: "Browser" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("separator", { name: "Resize browser" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Visualization")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Options" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Options" }));
    expect(
      screen.getByRole("combobox", { name: "Playback mode" }),
    ).toBeVisible();
    fireEvent.change(screen.getByRole("combobox", { name: "Playback mode" }), {
      target: { value: "sequential" },
    });
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(
          ([command, args]) =>
            command === "invoke_command" &&
            (args as { envelope?: { command?: string } }).envelope?.command ===
              "set_playback_mode",
        ),
    ).toBe(true);
    expect(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("region", { name: "File list" }),
    ).toBeInTheDocument();
  });

  it("remembers the window size and restores it when leaving compact mode", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );
    await waitFor(() => expect(mockSetSize).toHaveBeenCalledTimes(1));

    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 1200, height: 800 }),
      ),
    );
    expect(screen.getByRole("tab", { name: "Browser" })).toBeInTheDocument();
  });

  it("keeps the window size when maximized and persists the preference", async () => {
    mockIsMaximized.mockResolvedValue(true);
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toHaveAttribute("aria-pressed", "true"),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockSetSize).not.toHaveBeenCalled();
    expect(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    ).toHaveAttribute("aria-pressed", "true");
    await waitFor(() =>
      expect(
        vi
          .mocked(invoke)
          .mock.calls.some(
            ([command, args]) =>
              command === "save_player_preferences" &&
              (args as { preferences: { compact_mode?: boolean } }).preferences
                ?.compact_mode === true,
          ),
      ).toBe(true),
    );
    mockIsMaximized.mockResolvedValue(false);
  });

  it("switches between Files and Folders tabs in compact mode", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    const filesTab = await screen.findByRole("tab", { name: "Files" });
    const foldersTab = screen.getByRole("tab", { name: "Folders" });
    expect(filesTab).toHaveAttribute("aria-selected", "true");
    expect(foldersTab).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("region", { name: "File list" })).toBeVisible();

    fireEvent.click(foldersTab);
    expect(foldersTab).toHaveAttribute("aria-selected", "true");
    expect(filesTab).toHaveAttribute("aria-selected", "false");
    const filesPanel = document.getElementById("compact-panel-files")!;
    const foldersPanel = document.getElementById("compact-panel-folders")!;
    expect(filesPanel).toHaveAttribute("hidden");
    expect(foldersPanel).not.toHaveAttribute("hidden");
    expect(screen.getByRole("tree", { name: "Folder browser" })).toBeVisible();

    fireEvent.click(filesTab);
    expect(filesPanel).not.toHaveAttribute("hidden");
    expect(foldersPanel).toHaveAttribute("hidden");
    expect(screen.getByRole("region", { name: "File list" })).toBeVisible();
  });

  it("restores the persisted window size after restarting in compact mode", async () => {
    mockSetSize.mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: {
            ...DEFAULT_PLAYER_PREFERENCES,
            compact_mode: true,
            window_width: 960,
            window_height: 640,
          },
        };
      }
      if (command === "save_player_preferences") {
        return {
          version: 1,
          preferences: (args as { preferences: PlayerPreferences }).preferences,
        };
      }
      return undefined;
    });
    render(<App />);

    const toggle = await screen.findByRole("button", {
      name: "Toggle compact mode",
    });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-pressed", "true"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockSetSize).not.toHaveBeenCalled();

    fireEvent.click(toggle);

    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 960, height: 640 }),
      ),
    );
  });

  it("shows bookmarks and recent folders tabs in compact mode", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    const filesTab = await screen.findByRole("tab", { name: "Files" });
    expect(screen.getByRole("tab", { name: "Folders" })).toBeInTheDocument();
    const bookmarksTab = screen.getByRole("tab", { name: "Bookmarks" });
    const recentTab = screen.getByRole("tab", { name: "Recent folders" });
    expect(filesTab).toHaveAttribute("aria-selected", "true");

    fireEvent.click(bookmarksTab);
    expect(bookmarksTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("No bookmarks yet.")).toBeVisible();

    fireEvent.click(recentTab);
    expect(recentTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("No recent folders yet.")).toBeVisible();
  });

  it("reopens a recent folder into the folders tab in compact mode", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
      }
      if (command === "save_player_preferences") {
        return {
          version: 1,
          preferences: (args as { preferences: PlayerPreferences }).preferences,
        };
      }
      if (command === "invoke_command") {
        const envelope = (args as { envelope: { command: string } }).envelope;
        if (envelope.command === "list_recent_folders") {
          return {
            version: 1,
            ok: true,
            data: {
              folders: [{ path: "/music", name: "music", last_opened_ms: 1 }],
            },
          };
        }
        if (envelope.command === "list_devices") {
          return { version: 1, ok: true, data: { devices: [] } };
        }
        if (envelope.command === "current_device") {
          return { version: 1, ok: true, data: { device: null } };
        }
        if (envelope.command === "list_folder_bookmarks") {
          return { version: 1, ok: true, data: { bookmarks: [] } };
        }
        return { version: 1, ok: true, data: {} };
      }
      return undefined;
    });
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    fireEvent.click(await screen.findByRole("tab", { name: "Recent folders" }));
    fireEvent.click(await screen.findByRole("button", { name: "music" }));

    expect(screen.getByRole("tab", { name: "Folders" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("persists the compact window size after resizing", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );
    await waitFor(() => expect(mockSetSize).toHaveBeenCalledTimes(1));

    mockInnerSize.mockResolvedValue({ width: 500, height: 700 });
    for (const handler of resizeHandlers) handler({});
    await new Promise((resolve) => setTimeout(resolve, 500));

    await waitFor(() =>
      expect(
        vi
          .mocked(invoke)
          .mock.calls.some(
            ([command, args]) =>
              command === "save_player_preferences" &&
              (args as { preferences: { compact_window_width?: number } })
                .preferences?.compact_window_width === 500,
          ),
      ).toBe(true),
    );
  });

  it("restores the persisted compact window size after restarting in compact mode", async () => {
    mockSetSize.mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: {
            ...DEFAULT_PLAYER_PREFERENCES,
            compact_mode: true,
            compact_window_width: 460,
            compact_window_height: 640,
          },
        };
      }
      if (command === "save_player_preferences") {
        return {
          version: 1,
          preferences: (args as { preferences: PlayerPreferences }).preferences,
        };
      }
      return undefined;
    });
    render(<App />);

    const toggle = await screen.findByRole("button", {
      name: "Toggle compact mode",
    });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-pressed", "true"));
    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 460, height: 640 }),
      ),
    );
  });

  it("clamps a persisted compact size below the minimum on restart", async () => {
    mockSetSize.mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: {
            ...DEFAULT_PLAYER_PREFERENCES,
            compact_mode: true,
            compact_window_width: 300,
            compact_window_height: 400,
          },
        };
      }
      if (command === "save_player_preferences") {
        return {
          version: 1,
          preferences: (args as { preferences: PlayerPreferences }).preferences,
        };
      }
      return undefined;
    });
    render(<App />);

    const toggle = await screen.findByRole("button", {
      name: "Toggle compact mode",
    });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-pressed", "true"));
    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 440, height: 600 }),
      ),
    );
  });

  it("clamps persisted resizes to the compact minimum", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );
    await waitFor(() => expect(mockSetSize).toHaveBeenCalledTimes(1));

    mockInnerSize.mockResolvedValue({ width: 300, height: 400 });
    for (const handler of resizeHandlers) handler({});
    await new Promise((resolve) => setTimeout(resolve, 500));

    await waitFor(() =>
      expect(
        vi.mocked(invoke).mock.calls.some(
          ([command, args]) =>
            command === "save_player_preferences" &&
            (
              args as {
                preferences: {
                  compact_window_width?: number;
                  compact_window_height?: number;
                };
              }
            ).preferences?.compact_window_width === 440 &&
            (
              args as {
                preferences: {
                  compact_window_width?: number;
                  compact_window_height?: number;
                };
              }
            ).preferences?.compact_window_height === 600,
        ),
      ).toBe(true),
    );
  });

  it("enforces the compact minimum on the live window and lifts it on exit", async () => {
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    await waitFor(() =>
      expect(mockSetMinSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 440, height: 600 }),
      ),
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );
    await waitFor(() => expect(mockSetMinSize).toHaveBeenCalledWith(null));
  });

  it("enforces the compact minimum when restarting in compact mode", async () => {
    mockSetMinSize.mockClear();
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return {
          version: 1,
          preferences: {
            ...DEFAULT_PLAYER_PREFERENCES,
            compact_mode: true,
            compact_window_width: 460,
            compact_window_height: 640,
          },
        };
      }
      if (command === "save_player_preferences") {
        return {
          version: 1,
          preferences: (args as { preferences: PlayerPreferences }).preferences,
        };
      }
      return undefined;
    });
    render(<App />);

    const toggle = await screen.findByRole("button", {
      name: "Toggle compact mode",
    });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-pressed", "true"));
    await waitFor(() =>
      expect(mockSetMinSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 440, height: 600 }),
      ),
    );
  });

  it("still resizes to the compact minimum when setMinSize is rejected", async () => {
    mockSetSize.mockClear();
    mockSetMinSize.mockRejectedValue(new Error("not allowed"));
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 440, height: 600 }),
      ),
    );
  });

  it("still restores the window size when lifting the minimum is rejected", async () => {
    mockSetMinSize.mockRejectedValue(new Error("not allowed"));
    mockBackend();
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Toggle compact mode" }),
      ).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );
    await waitFor(() => expect(mockSetSize).toHaveBeenCalledTimes(1));

    fireEvent.click(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );
    await waitFor(() =>
      expect(mockSetSize).toHaveBeenCalledWith(
        expect.objectContaining({ width: 1200, height: 800 }),
      ),
    );
  });
});

describe("shortcut integration", () => {
  function mockAppBackend(
    bindings = DEFAULT_SHORTCUTS,
    failShortcutLoad = false,
  ) {
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
      }
      if (command === "save_player_preferences") {
        return {
          version: 1,
          preferences: (args as { preferences: PlayerPreferences }).preferences,
        };
      }
      if (command === "pick_folder_dialog") return { path: "/music" };
      if (command !== "invoke_command") return undefined;
      const envelope = (
        args as { envelope: { command: string; payload: unknown } }
      ).envelope;
      switch (envelope.command) {
        case "load_shortcuts":
          if (failShortcutLoad) throw new Error("private shortcut path");
          return { version: 1, ok: true, data: shortcutProfile(bindings) };
        case "save_shortcuts":
        case "reset_shortcuts":
          return { version: 1, ok: true, data: shortcutProfile(bindings) };
        case "list_browser_roots":
          return {
            version: 1,
            ok: true,
            data: {
              roots: [{ path: "/music", name: "Music", kind: "physical" }],
              libraries: [],
            },
          };
        case "list_recent_folders":
          return { version: 1, ok: true, data: { folders: [] } };
        case "list_folder_bookmarks":
          return { version: 1, ok: true, data: { bookmarks: [] } };
        case "record_recent_folder":
          return { version: 1, ok: true, data: {} };
        case "start_enumeration":
          return { version: 1, ok: true, data: { session_id: "shortcuts" } };
        case "set_playback_mode":
          return {
            version: 1,
            ok: true,
            data: { mode: (envelope.payload as { mode: string }).mode },
          };
        default:
          return {
            version: 1,
            ok: false,
            error: {
              category: "Unavailable",
              message: "Unmocked command.",
              diagnostic_code: "command.unknown",
            },
          };
      }
    });
  }

  it("opens editor and saves through backend-confirmed profile", async () => {
    mockAppBackend();
    render(<App />);

    expect(
      screen.queryByRole("button", { name: "Keyboard shortcuts" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Options" }));
    fireEvent.click(screen.getByRole("button", { name: "Keyboard shortcuts" }));
    expect(
      screen.getByRole("dialog", { name: "Keyboard shortcuts" }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Keyboard shortcuts" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(
          ([command, args]) =>
            command === "invoke_command" &&
            (args as { envelope: { command: string } }).envelope.command ===
              "save_shortcuts",
        ),
    ).toBe(true);
  });

  it("groups general settings inside an isolated application menu", async () => {
    mockAppBackend();
    render(<App />);

    const menuButton = screen.getByRole("button", { name: "Options" });
    expect(menuButton).toBeInTheDocument();
    expect(screen.queryByLabelText("Theme")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: "Real-time visualizations" }),
    ).not.toBeInTheDocument();
    expect(menuButton.closest(".now-playing-actions")).toContainElement(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    );

    fireEvent.click(menuButton);

    expect(screen.getByLabelText("Output device")).toBeVisible();
    expect(screen.getByLabelText("Theme")).toBeVisible();
    expect(
      screen.getByRole("checkbox", { name: "Real-time visualizations" }),
    ).toBeVisible();
    expect(screen.getByLabelText("Visualization quality")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Keyboard shortcuts" }),
    ).toBeVisible();
  });

  it("shows a safe shortcut loading error while defaults stay available", async () => {
    mockAppBackend(DEFAULT_SHORTCUTS, true);
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByText("Keyboard shortcuts unavailable."),
      ).toBeInTheDocument(),
    );
    fireEvent.keyDown(window, { key: "f", ctrlKey: true });
    expect(
      screen.getByRole("searchbox", { name: /search files/i }),
    ).toHaveFocus();
  });

  it("routes confirmed open, search, refresh, and playback-mode bindings", async () => {
    const bindings = {
      ...DEFAULT_SHORTCUTS,
      open_folder: { key: "p", primary: true, shift: false, alt: false },
      focus_search: { key: "g", primary: true, shift: false, alt: false },
    };
    mockAppBackend(bindings);
    render(<App />);
    await waitFor(() =>
      expect(screen.queryByText("Loading shortcuts…")).not.toBeInTheDocument(),
    );

    fireEvent.keyDown(window, { key: "g", ctrlKey: true });
    expect(
      screen.getByRole("searchbox", { name: /search files/i }),
    ).toHaveFocus();
    fireEvent.keyDown(window, { key: "p", ctrlKey: true });
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("pick_folder_dialog", {}),
    );
    await waitFor(() =>
      expect(
        vi
          .mocked(invoke)
          .mock.calls.filter(
            ([command, args]) =>
              command === "invoke_command" &&
              (args as { envelope: { command: string } }).envelope.command ===
                "start_enumeration",
          ),
      ).toHaveLength(1),
    );
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });
    fireEvent.keyDown(window, { key: "4", ctrlKey: true, altKey: true });

    await waitFor(() =>
      expect(
        vi.mocked(invoke).mock.calls.some(
          ([command, args]) =>
            command === "invoke_command" &&
            (args as { envelope: { command: string } }).envelope.command ===
              "set_playback_mode" &&
            (
              args as {
                envelope: { payload: { mode: string } };
              }
            ).envelope.payload.mode === "random",
        ),
      ).toBe(true),
    );
    expect(
      vi
        .mocked(invoke)
        .mock.calls.filter(
          ([command, args]) =>
            command === "invoke_command" &&
            (args as { envelope: { command: string } }).envelope.command ===
              "start_enumeration",
        ).length,
    ).toBeGreaterThanOrEqual(2);
  });
});

describe("recent folders wiring", () => {
  const envelopeResponse = (data: unknown) => ({
    version: 1,
    ok: true,
    data,
  });

  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(async (command: string, args) => {
      if (command === "load_player_preferences") {
        return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
      }
      if (command === "save_player_preferences") {
        const preferences = (args as { preferences: PlayerPreferences })
          .preferences;
        return { version: 1, preferences };
      }
      if (command === "invoke_command") {
        const envelope = (
          args as { envelope: { command: string; payload: unknown } }
        ).envelope;
        switch (envelope.command) {
          case "list_browser_roots":
            return envelopeResponse({
              roots: [{ path: "/music", name: "Music", kind: "physical" }],
              libraries: [],
            });
          case "list_recent_folders":
            return envelopeResponse({ folders: [] });
          case "list_folder_bookmarks":
            return envelopeResponse({ bookmarks: [] });
          case "record_recent_folder":
          case "clear_recent_folders":
          case "add_folder_bookmark":
          case "remove_folder_bookmark":
            return envelopeResponse({});
          case "start_enumeration":
            return envelopeResponse({ session_id: "session-1" });
          default:
            // Unknown commands fail like an unmocked backend so hooks fall
            // back to their empty/error states instead of crashing.
            return {
              version: 1,
              ok: false,
              error: {
                category: "Unavailable",
                message: "Unmocked command.",
                diagnostic_code: "command.unknown",
              },
            };
        }
      }
      return undefined;
    });
  });

  it("shows an empty recent-folders state on first launch", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("tab", { name: "Recent folders" }));

    await waitFor(() =>
      expect(screen.getByText("No recent folders yet.")).toBeInTheDocument(),
    );
  });

  it("records a selected folder and clears the history", async () => {
    render(<App />);

    await screen.findByText("Music", { exact: true });
    fireEvent.click(screen.getByText("Music", { exact: true }));
    fireEvent.click(screen.getByRole("tab", { name: "Recent folders" }));

    // The selected folder appears in the recent-folders sidebar with its
    // path basename ("music", unlike the capitalized tree root label).
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "music" })).toBeInTheDocument(),
    );

    expect(
      vi.mocked(invoke).mock.calls.some(
        ([command, args]) =>
          command === "invoke_command" &&
          (
            args as {
              envelope: { command: string; payload: { path?: string } };
            }
          ).envelope.command === "record_recent_folder" &&
          (args as { envelope: { payload: { path?: string } } }).envelope
            .payload.path === "/music",
      ),
    ).toBe(true);

    fireEvent.click(
      screen.getByRole("button", { name: "Clear recent folders" }),
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "music" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(
          ([command, args]) =>
            command === "invoke_command" &&
            (args as { envelope: { command: string } }).envelope.command ===
              "clear_recent_folders",
        ),
    ).toBe(true);
  });

  it("bookmarks the selected folder and removes it from the Bookmarks tab", async () => {
    render(<App />);

    await screen.findByText("Music", { exact: true });
    fireEvent.click(screen.getByText("Music", { exact: true }));
    fireEvent.click(screen.getByRole("button", { name: "Bookmark folder" }));
    fireEvent.click(screen.getByRole("tab", { name: "Bookmarks" }));

    const bookmark = await screen.findByRole("button", { name: "music" });
    expect(bookmark).toHaveAttribute("title", "/music");
    fireEvent.click(
      screen.getByRole("button", { name: "Remove music bookmark" }),
    );
    await waitFor(() => expect(bookmark).not.toBeInTheDocument());
  });

  it("persists the hidden-folder option and refreshes the selected folder", async () => {
    render(<App />);
    await screen.findByText("Music", { exact: true });
    fireEvent.click(screen.getByText("Music", { exact: true }));
    fireEvent.click(screen.getByRole("button", { name: "Options" }));

    const option = screen.getByRole("checkbox", {
      name: "Show hidden folders",
    });
    expect(option).not.toBeChecked();
    fireEvent.click(option);

    await waitFor(() => {
      expect(
        vi
          .mocked(invoke)
          .mock.calls.some(
            ([command, args]) =>
              command === "save_player_preferences" &&
              (args as { preferences: PlayerPreferences }).preferences
                .show_hidden_folders === true,
          ),
      ).toBe(true);
      expect(
        vi.mocked(invoke).mock.calls.some(
          ([command, args]) =>
            command === "invoke_command" &&
            (
              args as {
                envelope: {
                  command: string;
                  payload: { show_hidden?: boolean };
                };
              }
            ).envelope.command === "start_enumeration" &&
            (
              args as {
                envelope: { payload: { show_hidden?: boolean } };
              }
            ).envelope.payload.show_hidden === true,
        ),
      ).toBe(true);
    });
  });
});
