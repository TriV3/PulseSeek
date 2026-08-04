import { describe, expect, it, vi } from "vitest";
import type { CommandResponse } from "./commandEnvelope";
import {
  CommandError,
  healthCheck,
  invokeCommand,
  loadPlayerPreferences,
  setPlaybackMode,
} from "./commandEnvelope";

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
      },
    });

    await expect(loadPlayerPreferences()).rejects.toThrow(
      "Invalid player preferences response.",
    );
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

describe("listBrowserRoots", () => {
  it("returns local and network-mounted roots", async () => {
    mockInvoke.mockResolvedValueOnce({
      version: 1,
      ok: true,
      data: {
        roots: [
          { path: "/", name: "System" },
          { path: "/Volumes/NAS", name: "NAS" },
        ],
      },
    });

    const { listBrowserRoots } = await import("./commandEnvelope");
    await expect(listBrowserRoots()).resolves.toEqual([
      { path: "/", name: "System" },
      { path: "/Volumes/NAS", name: "NAS" },
    ]);
    expect(mockInvoke).toHaveBeenCalledWith("invoke_command", {
      envelope: {
        version: 1,
        command: "list_browser_roots",
        payload: {},
      },
    });
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
        payload: { path: "/music", batch_size: 100, recursive: true },
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
        payload: { path: "/music", batch_size: undefined, recursive: false },
      },
    });
  });
});
