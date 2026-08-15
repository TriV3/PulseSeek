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

let openedPaths: string[] = [];
let probeKinds = new Map<string, string>();
const playCalls: string[] = [];

// `start_enumeration` only needs a stable id in this suite.
const OPENED_FILES_SESSION = "opened-files-session";

function installBackendMock() {
  openedPaths = [];
  probeKinds = new Map<string, string>();
  playCalls.length = 0;
  webviewMock.onDragDropEvent.mockResolvedValue(() => {});

  vi.mocked(invoke).mockImplementation(async (command: string, args) => {
    if (command === "load_player_preferences") {
      return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
    }
    if (command === "save_player_preferences") {
      const preferences = (args as { preferences: Record<string, unknown> })
        .preferences;
      return {
        version: 1,
        preferences: { ...DEFAULT_PLAYER_PREFERENCES, ...preferences },
      };
    }
    if (command === "opened_audio_files") {
      // Mirror the backend drain: pending paths are returned once and cleared.
      const pending = openedPaths;
      openedPaths = [];
      return pending;
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
        case "list_folder_bookmarks":
          return ok({ bookmarks: [] });
        case "cancel_enumeration":
          return ok({});
        case "probe_path": {
          const path = String(envelope.payload.path);
          return ok({ kind: probeKinds.get(path) ?? "unsupported" });
        }
        case "start_enumeration":
          return ok({ session_id: `folder-${OPENED_FILES_SESSION}` });
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

async function renderApp(options?: {
  opened?: string[];
  kinds?: Record<string, string>;
}) {
  installBackendMock();
  if (options?.opened) openedPaths = options.opened;
  if (options?.kinds) probeKinds = new Map(Object.entries(options.kinds));
  render(<App />);
  await waitFor(() => {
    expect(screen.getByText("Music")).toBeInTheDocument();
  });
}

function emitOpenedFiles(paths: string[]) {
  const handler = eventHandlers.get("browser:opened-files");
  if (!handler) throw new Error("opened-files listener not registered");
  act(() => {
    handler({ payload: { paths } });
  });
}

describe("operating-system opened files", () => {
  it("plays the first compatible file delivered on a cold start", async () => {
    await renderApp({
      opened: ["/music/notes.txt", "/music/song.ogg", "/music/song.flac"],
      kinds: {
        "/music/song.ogg": "playable",
        "/music/song.flac": "playable",
      },
    });

    await waitFor(() => {
      expect(playCalls).toEqual(["/music/song.ogg"]);
    });
  });

  it("plays the first compatible file delivered while running", async () => {
    await renderApp({ kinds: { "/music/a.wav": "playable" } });

    emitOpenedFiles(["/music/a.wav", "/music/b.mp3"]);

    await waitFor(() => {
      expect(playCalls).toEqual(["/music/a.wav"]);
    });
  });

  it("ignores opened files that are not decodable", async () => {
    await renderApp({ opened: ["/music/notes.txt"] });

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(playCalls).toEqual([]);
  });

  it("plays nothing when no file was opened", async () => {
    await renderApp();

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(playCalls).toEqual([]);
  });
});
