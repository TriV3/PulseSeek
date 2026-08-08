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
  unavailable_action_ids: ["set_ab_start", "set_ab_end", "toggle_ab_repeat"],
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockReset();
});

describe("application shell", () => {
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

  it("presents the Resonic-inspired workspace regions", () => {
    render(<App />);

    expect(
      screen.getByRole("region", { name: "Waveform overview" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("toolbar", { name: "Playback controls" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Browser" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "File list" })).toBeInTheDocument();
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
      name: "Resize waveform",
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
            data: { roots: [{ path: "/music", name: "Music" }] },
          };
        case "list_recent_folders":
          return { version: 1, ok: true, data: { folders: [] } };
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

  it("groups output device and theme under the application menu", async () => {
    mockAppBackend();
    render(<App />);

    const menuButton = screen.getByLabelText("Open application menu");
    expect(menuButton).toBeInTheDocument();
    expect(screen.queryByLabelText("Theme")).not.toBeVisible();

    fireEvent.click(menuButton);

    expect(screen.getByLabelText("Output device")).toBeVisible();
    expect(screen.getByLabelText("Theme")).toBeVisible();
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
              roots: [{ path: "/music", name: "Music" }],
            });
          case "list_recent_folders":
            return envelopeResponse({ folders: [] });
          case "record_recent_folder":
          case "clear_recent_folders":
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

    await waitFor(() =>
      expect(screen.getByText("No recent folders yet.")).toBeInTheDocument(),
    );
  });

  it("records a selected folder and clears the history", async () => {
    render(<App />);

    await screen.findByText("Computer", { exact: true });
    fireEvent.click(screen.getByText("Computer", { exact: true }));
    await screen.findByText("Music", { exact: true });
    fireEvent.click(screen.getByText("Music", { exact: true }));

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
});
