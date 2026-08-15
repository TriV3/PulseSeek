import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { DEFAULT_PLAYER_PREFERENCES } from "./hooks/usePlayerPreferences";
import type { CommandResponse } from "./api/commandEnvelope";

type EventHandler = (event: { payload: unknown }) => void;
const eventHandlers = new Map<string, EventHandler>();

const webviewMock = vi.hoisted(() => ({
  onDragDropEvent: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: EventHandler) => {
    eventHandlers.set(event, handler);
    return () => {
      eventHandlers.delete(event);
    };
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: webviewMock.onDragDropEvent,
  }),
}));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: vi.fn(
    (opts: { count: number; estimateSize: () => number }) => {
      const items = Array.from(
        { length: Math.min(opts.count, 20) },
        (_, i) => ({
          key: i,
          index: i,
          start: i * opts.estimateSize(),
          size: opts.estimateSize(),
        }),
      );
      return {
        getVirtualItems: () => items,
        getTotalSize: () => items.length * opts.estimateSize(),
        scrollToIndex: vi.fn(),
      };
    },
  ),
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockReset();
  eventHandlers.clear();
});

const ok = (data: unknown): CommandResponse => ({
  version: 1,
  ok: true,
  data,
});

const fileEntry = (id: string) => ({
  id,
  name: id.split("/").filter(Boolean).at(-1) ?? id,
  kind: "playable",
  metadata: {
    duration_ms: 60_000,
    size_bytes: 2048,
    modified_at_ms: 1_700_000_000_000,
    channels: 2,
    sample_rate: 44_100,
    bit_depth: 16,
    codec: "wav",
  },
});

let sessionsByPath = new Map<string, string[]>();
let probeKinds = new Map<string, string>();
const playCalls: string[] = [];
const recentFolderCalls: string[] = [];
const savedPreferences: Array<Record<string, unknown>> = [];

function installBackendMock() {
  sessionsByPath = new Map<string, string[]>();
  probeKinds = new Map<string, string>();
  playCalls.length = 0;
  recentFolderCalls.length = 0;
  savedPreferences.length = 0;
  webviewMock.onDragDropEvent.mockResolvedValue(() => {});

  vi.mocked(invoke).mockImplementation(async (command: string, args) => {
    if (command === "load_player_preferences") {
      return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
    }
    if (command === "save_player_preferences") {
      const preferences = (args as { preferences: Record<string, unknown> })
        .preferences;
      savedPreferences.push(preferences);
      return {
        version: 1,
        preferences: { ...DEFAULT_PLAYER_PREFERENCES, ...preferences },
      };
    }
    if (command === "pick_folder_dialog") {
      return { path: "/library" };
    }
    if (command === "invoke_command") {
      const envelope = (
        args as {
          envelope: { command: string; payload: Record<string, unknown> };
        }
      ).envelope;
      switch (envelope.command) {
        case "list_browser_roots":
          return ok({
            roots: [{ path: "/music", name: "Music", kind: "physical" }],
            libraries: [],
          });
        case "record_recent_folder": {
          recentFolderCalls.push(String(envelope.payload.path));
          return ok({});
        }
        case "list_folder_bookmarks":
          return ok({ bookmarks: [] });
        case "cancel_enumeration":
          return ok({});
        case "probe_path": {
          const path = String(envelope.payload.path);
          const kind = probeKinds.get(path) ?? "unsupported";
          if (kind === "throw") {
            throw new Error("probe failed");
          }
          return ok({ kind });
        }
        case "start_enumeration": {
          const path = String(envelope.payload.path);
          const sessionId = `folder-${sessionsByPath.get(path)?.length ?? 0}`;
          sessionsByPath.set(path, [
            ...(sessionsByPath.get(path) ?? []),
            sessionId,
          ]);
          return ok({ session_id: sessionId });
        }
        case "play": {
          playCalls.push(String(envelope.payload.path));
          return ok({});
        }
        default:
          return undefined;
      }
    }
    return undefined;
  });
}

function emitFolderChunk(sessionId: string, entries: unknown[], done: boolean) {
  const handler = eventHandlers.get("browser:folder-chunk");
  if (!handler) throw new Error("folder-chunk listener not registered");
  act(() => {
    handler({
      payload: {
        session_id: sessionId,
        entries,
        folders_done: true,
        done,
      },
    });
  });
}

function dropHandler(): (event: {
  payload: { type: string; paths: string[] };
}) => void {
  const handler = webviewMock.onDragDropEvent.mock.calls[0]?.[0];
  if (!handler) throw new Error("drop handler not registered");
  return handler;
}

function fireDrop(paths: string[]) {
  act(() => dropHandler()({ payload: { type: "drop", paths } }));
}

function enumerateDroppedFolder(path: string, entries: unknown[]) {
  const sessionId = sessionsByPath.get(path)?.[0];
  if (!sessionId) throw new Error(`no session for ${path}`);
  emitFolderChunk(sessionId, entries, false);
  emitFolderChunk(sessionId, [], true);
}

async function renderApp() {
  installBackendMock();
  render(<App />);
  await waitFor(() => {
    expect(screen.getByText("Music")).toBeInTheDocument();
  });
}

// ── Tests ──────────────────────────────────────────────────────────────

describe("external file drag-in", () => {
  it("plays a dropped audio file and reveals its parent folder", async () => {
    await renderApp();
    probeKinds.set("/music/song.wav", "playable");

    fireDrop(["/music/song.wav"]);

    await waitFor(() => {
      expect(playCalls).toEqual(["/music/song.wav"]);
    });
    await waitFor(() => {
      expect(recentFolderCalls).toContain("/music");
    });
    await waitFor(() => {
      expect(sessionsByPath.get("/music")?.length ?? 0).toBeGreaterThan(0);
    });

    enumerateDroppedFolder("/music", [fileEntry("/music/song.wav")]);
    await waitFor(() => {
      // The file appears in the file list and in the player header.
      expect(screen.getAllByText("song.wav").length).toBeGreaterThan(0);
    });
    await waitFor(() => {
      expect(
        savedPreferences.some(
          (prefs) => prefs.last_played_file_path === "/music/song.wav",
        ),
      ).toBe(true);
    });
  });

  it("reveals a dropped folder without playing anything", async () => {
    await renderApp();
    probeKinds.set("/downloads/project", "directory");

    fireDrop(["/downloads/project"]);

    await waitFor(() => {
      expect(recentFolderCalls).toContain("/downloads/project");
    });
    await waitFor(() => {
      expect(
        sessionsByPath.get("/downloads/project")?.length ?? 0,
      ).toBeGreaterThan(0);
    });
    expect(playCalls).toEqual([]);
    await waitFor(() => {
      expect(
        savedPreferences.some(
          (prefs) => prefs.selected_folder_path === "/downloads/project",
        ),
      ).toBe(true);
    });
  });

  it("ignores non-audio and missing targets", async () => {
    await renderApp();
    probeKinds.set("/notes.txt", "unsupported");
    probeKinds.set("/gone.wav", "missing");

    fireDrop(["/notes.txt", "/gone.wav"]);

    await waitFor(() => {
      expect(playCalls).toEqual([]);
      expect(recentFolderCalls).toEqual([]);
      expect(sessionsByPath.size).toBe(0);
    });
  });

  it("gives a dropped folder priority over dropped audio", async () => {
    await renderApp();
    probeKinds.set("/downloads/project", "directory");
    probeKinds.set("/music/song.wav", "playable");

    fireDrop(["/music/song.wav", "/downloads/project"]);

    await waitFor(() => {
      expect(recentFolderCalls).toContain("/downloads/project");
    });
    expect(playCalls).toEqual([]);
  });

  it("plays only the first audio file of a multi-file drop", async () => {
    await renderApp();
    probeKinds.set("/music/a.wav", "playable");
    probeKinds.set("/music/b.wav", "playable");

    fireDrop(["/music/a.wav", "/music/b.wav"]);

    await waitFor(() => {
      expect(playCalls).toEqual(["/music/a.wav"]);
    });
  });

  it("ignores the whole drop when probing fails", async () => {
    await renderApp();
    probeKinds.set("/music/song.wav", "throw");

    fireDrop(["/music/song.wav"]);

    await waitFor(() => {
      expect(playCalls).toEqual([]);
      expect(recentFolderCalls).toEqual([]);
      expect(sessionsByPath.size).toBe(0);
    });
  });
});
