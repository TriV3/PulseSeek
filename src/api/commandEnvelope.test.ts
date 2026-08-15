import { describe, expect, it, vi } from "vitest";
import type { CommandResponse } from "./commandEnvelope";
import {
  CommandError,
  clearLoopRegion,
  healthCheck,
  invokeCommand,
  loadPlayerPreferences,
  openedAudioFiles,
  setLoopRegion,
  setPlaybackMode,
  loadShortcuts,
  resetShortcuts,
  saveShortcuts,
  loadVisualizationSettings,
  saveVisualizationSettings,
} from "./commandEnvelope";
import { DEFAULT_SHORTCUTS } from "../shortcuts/keyboardShortcuts";

const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));

describe("invokeCommand", () => {
  it("returns data on successful response", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { ready: true },
    } satisfies CommandResponse<{ ready: boolean }>);

    const result = await invokeCommand<{ ready: boolean }>("health", {});

    expect(result).toEqual({ ready: true });
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: { version: 1, command: "health", payload: {} },
    });
  });

  it("throws CommandError when response is not ok", async () => {
    mockInvoke.mockResolvedValue({
      version: 1,
      ok: false,
      error: {
        category: "Unsupported",
        message: "Unknown command: nonexistent",
        diagnostic_code: "command.unknown",
      },
    } satisfies CommandResponse);

    await expect(invokeCommand("nonexistent", {})).rejects.toThrow(
      CommandError,
    );

    await expect(invokeCommand("nonexistent", {})).rejects.toThrow(
      "Unknown command: nonexistent",
    );
  });

  it("throws CommandError with category and diagnostic code", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "InvalidInput",
        message: "Invalid payload",
        diagnostic_code: "command.payload",
      },
    } satisfies CommandResponse);

    try {
      await invokeCommand("health", "bad_payload");
      expect.unreachable("should have thrown");
    } catch (error) {
      expect(error).toBeInstanceOf(CommandError);
      if (error instanceof CommandError) {
        expect(error.category).toBe("InvalidInput");
        expect(error.diagnosticCode).toBe("command.payload");
        expect(error.message).toBe("Invalid payload");
      }
    }
  });

  it("throws CommandError when error field is missing on non-ok response", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
    } satisfies CommandResponse);

    await expect(invokeCommand("health", {})).rejects.toThrow(CommandError);
  });
});

describe("opened audio files boundary", () => {
  it("returns the pending opened file paths", async () => {
    mockInvoke.mockResolvedValueOnce(["/music/a.wav", "/music/b.mp3"]);

    const paths = await openedAudioFiles();

    expect(paths).toEqual(["/music/a.wav", "/music/b.mp3"]);
    expect(mockInvoke).toHaveBeenCalledWith("opened_audio_files");
  });

  it("rejects a malformed response", async () => {
    mockInvoke.mockResolvedValueOnce(["/music/a.wav", 42]);

    await expect(openedAudioFiles()).rejects.toThrow(
      "Invalid opened files response.",
    );
  });
});

describe("player preferences boundary", () => {
  it("rejects malformed persisted state instead of leaking undefined into React", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1 });

    await expect(loadPlayerPreferences()).rejects.toThrow(
      "Invalid player preferences response.",
    );
  });

  it("accepts a waveform style in the valid set", async () => {
    for (const waveform_style of ["solid", "gradient", "outline"]) {
      mockInvoke.mockResolvedValueOnce({
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
          last_played_file_path: null,
          last_played_position_ms: 0,
          last_played_duration_ms: null,
          theme: "system",
          waveform_style,
          seek_step_mode: "auto",
          show_hidden_folders: false,
        },
      });

      await expect(loadPlayerPreferences()).resolves.toMatchObject({
        waveform_style,
      });
    }
  });

  it("rejects an unknown waveform style", async () => {
    mockInvoke.mockResolvedValueOnce({
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
        last_played_file_path: null,
        last_played_position_ms: 0,
        last_played_duration_ms: null,
        theme: "system",
        waveform_style: "neon",
        seek_step_mode: "auto",
        show_hidden_folders: false,
      },
    });

    await expect(loadPlayerPreferences()).rejects.toThrow(
      "Invalid player preferences response.",
    );
  });

  it("defaults compact_mode to false for legacy files without the field", async () => {
    mockInvoke.mockResolvedValueOnce({
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
        last_played_file_path: null,
        last_played_position_ms: 0,
        last_played_duration_ms: null,
        theme: "system",
        waveform_style: "outline",
        seek_step_mode: "auto",
        show_hidden_folders: false,
      },
    });

    await expect(loadPlayerPreferences()).resolves.toMatchObject({
      compact_mode: false,
    });
  });

  it("accepts a compact_mode boolean and rejects non-boolean values", async () => {
    const base = {
      schema_version: 1,
      revision: 0,
      playback_mode: "one-shot" as const,
      output_device_id: null,
      volume: 1,
      muted: false,
      waveform_size: 38,
      browser_size: 24,
      selected_folder_path: null,
      expanded_folder_paths: [],
      last_played_file_path: null,
      last_played_position_ms: 0,
      last_played_duration_ms: null,
      theme: "system" as const,
      waveform_style: "outline" as const,
      seek_step_mode: "auto" as const,
      show_hidden_folders: false,
    };

    mockInvoke.mockResolvedValueOnce({
      version: 1,
      preferences: { ...base, compact_mode: true },
    });
    await expect(loadPlayerPreferences()).resolves.toMatchObject({
      compact_mode: true,
    });

    mockInvoke.mockResolvedValueOnce({
      version: 1,
      preferences: { ...base, compact_mode: "yes" },
    });
    await expect(loadPlayerPreferences()).rejects.toThrow(
      "Invalid player preferences response.",
    );
  });

  it("defaults window size to null and accepts persisted logical sizes", async () => {
    const base = {
      schema_version: 1,
      revision: 0,
      playback_mode: "one-shot" as const,
      output_device_id: null,
      volume: 1,
      muted: false,
      waveform_size: 38,
      browser_size: 24,
      selected_folder_path: null,
      expanded_folder_paths: [],
      last_played_file_path: null,
      last_played_position_ms: 0,
      last_played_duration_ms: null,
      theme: "system" as const,
      waveform_style: "outline" as const,
      seek_step_mode: "auto" as const,
      show_hidden_folders: false,
    };

    mockInvoke.mockResolvedValueOnce({ version: 1, preferences: base });
    await expect(loadPlayerPreferences()).resolves.toMatchObject({
      window_width: null,
      window_height: null,
    });

    mockInvoke.mockResolvedValueOnce({
      version: 1,
      preferences: { ...base, window_width: 960, window_height: 640 },
    });
    await expect(loadPlayerPreferences()).resolves.toMatchObject({
      window_width: 960,
      window_height: 640,
    });

    mockInvoke.mockResolvedValueOnce({
      version: 1,
      preferences: { ...base, window_width: "wide" },
    });
    await expect(loadPlayerPreferences()).rejects.toThrow(
      "Invalid player preferences response.",
    );
  });

  it("defaults compact window size to null and accepts persisted logical sizes", async () => {
    const base = {
      schema_version: 1,
      revision: 0,
      playback_mode: "one-shot" as const,
      output_device_id: null,
      volume: 1,
      muted: false,
      waveform_size: 38,
      browser_size: 24,
      selected_folder_path: null,
      expanded_folder_paths: [],
      last_played_file_path: null,
      last_played_position_ms: 0,
      last_played_duration_ms: null,
      theme: "system" as const,
      waveform_style: "outline" as const,
      seek_step_mode: "auto" as const,
      show_hidden_folders: false,
    };

    mockInvoke.mockResolvedValueOnce({ version: 1, preferences: base });
    await expect(loadPlayerPreferences()).resolves.toMatchObject({
      compact_window_width: null,
      compact_window_height: null,
    });

    mockInvoke.mockResolvedValueOnce({
      version: 1,
      preferences: {
        ...base,
        compact_window_width: 440,
        compact_window_height: 600,
      },
    });
    await expect(loadPlayerPreferences()).resolves.toMatchObject({
      compact_window_width: 440,
      compact_window_height: 600,
    });

    mockInvoke.mockResolvedValueOnce({
      version: 1,
      preferences: { ...base, compact_window_width: "wide" },
    });
    await expect(loadPlayerPreferences()).rejects.toThrow(
      "Invalid player preferences response.",
    );
  });
});

describe("visualization settings boundary", () => {
  it("loads validated settings with the reduced-motion state", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      settings: { enabled: true, mode: "musical", quality: "high" },
    });

    await expect(loadVisualizationSettings(true)).resolves.toEqual({
      enabled: true,
      mode: "musical",
      quality: "high",
    });
    expect(mockInvoke).toHaveBeenCalledWith("load_visualization_settings", {
      reducedMotion: true,
    });
  });

  it("rejects unknown settings and saves only the supported contract", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      settings: { enabled: true, mode: "plugin", quality: "extreme" },
    });
    await expect(loadVisualizationSettings(false)).rejects.toThrow(
      "Invalid visualization settings response.",
    );

    const settings = {
      enabled: false,
      mode: "linear" as const,
      quality: "low" as const,
    };
    mockInvoke.mockResolvedValueOnce({ version: 1, settings });
    await expect(saveVisualizationSettings(settings, false)).resolves.toEqual(
      settings,
    );
    expect(mockInvoke).toHaveBeenCalledWith("save_visualization_settings", {
      settings,
      reducedMotion: false,
    });
  });
});

describe("healthCheck", () => {
  it("returns true when backend responds", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { ready: true },
    } satisfies CommandResponse<{ ready: boolean }>);

    await expect(healthCheck()).resolves.toBe(true);
  });

  it("throws CommandError on backend failure", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "Unsupported",
        message: "Unsupported command version",
        diagnostic_code: "command.version",
      },
    } satisfies CommandResponse);

    await expect(healthCheck()).rejects.toThrow(CommandError);
  });
});

describe("setPlaybackMode", () => {
  it("returns confirmed mode from Rust", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { mode: "random" },
    });

    await expect(setPlaybackMode("random")).resolves.toBe("random");
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "set_playback_mode",
        payload: { mode: "random" },
      },
    });
  });
});

describe("setLoopRegion", () => {
  it("returns the confirmed start position from Rust", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { start_ms: 1_000 },
    });

    await expect(setLoopRegion(1_000, 5_000)).resolves.toBe(1_000);
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "set_loop_region",
        payload: { start_ms: 1_000, end_ms: 5_000 },
      },
    });
  });

  it("throws CommandError when the backend rejects the region", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "InvalidInput",
        message: "loop region start 5000ms must be before end 1000ms",
        diagnostic_code: "playback.control",
      },
    });

    await expect(setLoopRegion(5_000, 1_000)).rejects.toThrow(CommandError);
  });
});

describe("clearLoopRegion", () => {
  it("sends the clear command and resolves", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {},
    });

    await expect(clearLoopRegion()).resolves.toBeUndefined();
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "clear_loop_region",
        payload: {},
      },
    });
  });
});

describe("shortcut mappings boundary", () => {
  const profile = {
    mappings: Object.entries(DEFAULT_SHORTCUTS)
      .filter((entry): entry is [string, NonNullable<(typeof entry)[1]>] =>
        Boolean(entry[1]),
      )
      .map(([action_id, binding]) => ({ action_id, ...binding })),
    unavailable_action_ids: [],
  };

  it("loads and converts every known backend mapping", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data: profile });

    await expect(loadShortcuts()).resolves.toEqual(DEFAULT_SHORTCUTS);
  });

  it("saves only mappings and returns the confirmed profile", async () => {
    const confirmed = {
      ...profile,
      mappings: profile.mappings.map((mapping) =>
        mapping.action_id === "open_folder"
          ? { ...mapping, key: "p" }
          : mapping,
      ),
    };
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: confirmed,
    });

    await expect(saveShortcuts(DEFAULT_SHORTCUTS)).resolves.toMatchObject({
      open_folder: { key: "p" },
      set_ab_start: { key: "[", primary: false, shift: false, alt: false },
    });
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "save_shortcuts",
        payload: { mappings: profile.mappings },
      },
    });
  });

  it("resets through the versioned command", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data: profile });

    await expect(resetShortcuts()).resolves.toEqual(DEFAULT_SHORTCUTS);
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: { version: 1, command: "reset_shortcuts", payload: {} },
    });
  });

  it.each([
    undefined,
    { mappings: [], unavailable_action_ids: [] },
    {
      ...profile,
      mappings: [...profile.mappings, profile.mappings[0]],
    },
    {
      ...profile,
      mappings: profile.mappings.map((mapping, index) =>
        index === 1
          ? {
              ...mapping,
              key: profile.mappings[0].key,
              primary: profile.mappings[0].primary,
              shift: profile.mappings[0].shift,
              alt: profile.mappings[0].alt,
            }
          : mapping,
      ),
    },
    {
      ...profile,
      mappings: profile.mappings.map((mapping, index) =>
        index === 0 ? { ...mapping, action_id: "future_action" } : mapping,
      ),
    },
    {
      ...profile,
      unavailable_action_ids: ["set_ab_start", "set_ab_start"],
    },
    {
      ...profile,
      mappings: profile.mappings.map((mapping, index) =>
        index === 0 ? { ...mapping, primary: "yes" } : mapping,
      ),
    },
    {
      ...profile,
      mappings: profile.mappings.map((mapping, index) =>
        index === 0 ? { ...mapping, key: "Control" } : mapping,
      ),
    },
  ])("rejects malformed or duplicate runtime profiles", async (data) => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data });

    await expect(loadShortcuts()).rejects.toThrow(
      "Invalid shortcut mappings response.",
    );
  });
});

describe("moveToTrash", () => {
  it("sends paths and returns per-file results", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {
        results: [
          { path: "/music/a.wav", ok: true },
          {
            path: "/music/b.wav",
            ok: false,
            category: "NotFound",
            message: "not found",
            diagnostic_code: "file.operation",
          },
        ],
      },
    });

    const { moveToTrash } = await import("./commandEnvelope");
    const results = await moveToTrash(["/music/a.wav", "/music/b.wav"]);

    expect(results).toHaveLength(2);
    expect(results[0].ok).toBe(true);
    expect(results[1].ok).toBe(false);
    expect(results[1].category).toBe("NotFound");
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "move_to_trash",
        payload: { paths: ["/music/a.wav", "/music/b.wav"] },
      },
    });
  });
});

describe("startMoveFiles", () => {
  it("sends paths and target dir, returning the session id", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { session_id: "move-3" },
    });

    const { startMoveFiles } = await import("./commandEnvelope");
    const sessionId = await startMoveFiles(
      ["/music/a.wav", "/music/b.wav"],
      "/library",
    );

    expect(sessionId).toBe("move-3");
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "start_move_files",
        payload: {
          paths: ["/music/a.wav", "/music/b.wav"],
          target_dir: "/library",
        },
      },
    });
  });

  it("propagates command errors for invalid targets", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "InvalidInput",
        message: "PulseSeek received invalid input.",
        diagnostic_code: "file.operation",
      },
    });

    const { startMoveFiles, CommandError } = await import("./commandEnvelope");
    await expect(startMoveFiles(["/music/a.wav"], "/missing")).rejects.toThrow(
      CommandError,
    );
  });
});

describe("cancelMoveFiles", () => {
  it("sends the session id", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data: {} });

    const { cancelMoveFiles } = await import("./commandEnvelope");
    await cancelMoveFiles("move-3");

    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "cancel_move_files",
        payload: { session_id: "move-3" },
      },
    });
  });
});

describe("startCopyFiles", () => {
  it("sends paths and target dir, returning the session id", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { session_id: "copy-3" },
    });

    const { startCopyFiles } = await import("./commandEnvelope");
    const sessionId = await startCopyFiles(
      ["/music/a.wav", "/music/b.wav"],
      "/library",
    );

    expect(sessionId).toBe("copy-3");
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "start_copy_files",
        payload: {
          paths: ["/music/a.wav", "/music/b.wav"],
          target_dir: "/library",
        },
      },
    });
  });

  it("propagates command errors for invalid targets", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "InvalidInput",
        message: "PulseSeek received invalid input.",
        diagnostic_code: "file.operation",
      },
    });

    const { startCopyFiles, CommandError } = await import("./commandEnvelope");
    await expect(startCopyFiles(["/music/a.wav"], "/missing")).rejects.toThrow(
      CommandError,
    );
  });
});

describe("cancelCopyFiles", () => {
  it("sends the session id", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data: {} });

    const { cancelCopyFiles } = await import("./commandEnvelope");
    await cancelCopyFiles("copy-3");

    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "cancel_copy_files",
        payload: { session_id: "copy-3" },
      },
    });
  });
});

describe("listBrowserRoots", () => {
  it("returns home, local, and network-mounted roots", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {
        roots: [
          { path: "/", name: "System", kind: "system" },
          { path: "/Users/test", name: "Home", kind: "home" },
          { path: "/Volumes/NAS", name: "NAS", kind: "network" },
        ],
        libraries: [
          { path: "/Users/test/Music", name: "Music", kind: "music" },
        ],
      },
    });

    const { listBrowserRoots } = await import("./commandEnvelope");
    await expect(listBrowserRoots()).resolves.toEqual({
      roots: [
        { path: "/", name: "System", kind: "system" },
        { path: "/Users/test", name: "Home", kind: "home" },
        { path: "/Volumes/NAS", name: "NAS", kind: "network" },
      ],
      libraries: [{ path: "/Users/test/Music", name: "Music", kind: "music" }],
    });
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "list_browser_roots",
        payload: {},
      },
    });
  });

  it("rejects an unknown root kind at the Tauri boundary", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {
        roots: [{ path: "/Volumes/Cloud", name: "Cloud", kind: "cloud" }],
        libraries: [],
      },
    });

    const { listBrowserRoots } = await import("./commandEnvelope");
    await expect(listBrowserRoots()).rejects.toThrow(
      "Invalid browser roots response.",
    );
  });
});

describe("startEnumeration", () => {
  it("sends the recursive flag when a recursive view is requested", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { session_id: "session-1" },
    });

    const { startEnumeration } = await import("./commandEnvelope");
    await expect(startEnumeration("/music", 100, true)).resolves.toBe(
      "session-1",
    );
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "start_enumeration",
        payload: {
          path: "/music",
          batch_size: 100,
          recursive: true,
          show_hidden: false,
        },
      },
    });
  });

  it("defaults recursive to false when omitted", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { session_id: "session-1" },
    });

    const { startEnumeration } = await import("./commandEnvelope");
    await startEnumeration("/music");
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "start_enumeration",
        payload: {
          path: "/music",
          batch_size: undefined,
          recursive: false,
          show_hidden: false,
        },
      },
    });
  });
});

describe("recent folders", () => {
  it("lists recent folders from the backend", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {
        folders: [
          { path: "/music/project", name: "project", last_opened_ms: 42 },
        ],
      },
    });

    const { listRecentFolders } = await import("./commandEnvelope");
    await expect(listRecentFolders()).resolves.toEqual([
      { path: "/music/project", name: "project", last_opened_ms: 42 },
    ]);
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "list_recent_folders",
        payload: {},
      },
    });
  });

  it("records a folder path", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data: {} });

    const { recordRecentFolder } = await import("./commandEnvelope");
    await recordRecentFolder("/music/project");
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "record_recent_folder",
        payload: { path: "/music/project" },
      },
    });
  });

  it("clears the recent-folder history", async () => {
    mockInvoke.mockResolvedValueOnce({ version: 1, ok: true, data: {} });

    const { clearRecentFolders } = await import("./commandEnvelope");
    await clearRecentFolders();
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "clear_recent_folders",
        payload: {},
      },
    });
  });

  it("surfaces backend rejection as CommandError", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "InvalidInput",
        message: "PulseSeek received invalid input.",
        diagnostic_code: "browser.read",
      },
    });

    const { recordRecentFolder, CommandError } =
      await import("./commandEnvelope");
    await expect(recordRecentFolder("/missing")).rejects.toBeInstanceOf(
      CommandError,
    );
  });

  it("renames a file and returns the new path and playing flag", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {
        old_path: "/music/song.mp3",
        new_path: "/music/renamed.mp3",
        was_playing: true,
      },
    });

    const { renameFile } = await import("./commandEnvelope");
    const outcome = await renameFile("/music/song.mp3", "renamed.mp3");

    expect(outcome).toEqual({
      old_path: "/music/song.mp3",
      new_path: "/music/renamed.mp3",
      was_playing: true,
    });
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "rename_file",
        payload: { path: "/music/song.mp3", new_name: "renamed.mp3" },
      },
    });
  });

  it("surfaces rename collision as CommandError", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "Conflict",
        message: "PulseSeek could not apply that change.",
        diagnostic_code: "file.operation",
      },
    });

    const { renameFile, CommandError } = await import("./commandEnvelope");
    await expect(renameFile("/music/song.mp3", "taken.mp3")).rejects.toThrow(
      CommandError,
    );
  });
});

describe("probePath", () => {
  it("sends the path and returns the classified kind", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: { kind: "playable" },
    });

    const { probePath } = await import("./commandEnvelope");
    await expect(probePath("/music/song.wav")).resolves.toBe("playable");

    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "probe_path",
        payload: { path: "/music/song.wav" },
      },
    });
  });

  it("surfaces inspection failure as CommandError", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: false,
      error: {
        category: "PermissionDenied",
        message: "PulseSeek could not access that file.",
        diagnostic_code: "file.operation",
      },
    });

    const { probePath, CommandError } = await import("./commandEnvelope");
    await expect(probePath("/private/music/song.wav")).rejects.toThrow(
      CommandError,
    );
  });
});
