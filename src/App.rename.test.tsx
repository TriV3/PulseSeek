import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { DEFAULT_PLAYER_PREFERENCES } from "./hooks/usePlayerPreferences";
import type { CommandResponse } from "./api/commandEnvelope";

// ── Mocks ──────────────────────────────────────────────────────────────

type EventHandler = (event: { payload: unknown }) => void;
const eventHandlers = new Map<string, EventHandler>();

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
    onDragDropEvent: () => Promise.resolve(() => {}),
  }),
}));

// jsdom has no layout, so the virtualizer would measure 0 height and render
// nothing. Render all rows synchronously like the FileList tests do.
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

const playableChunk = (
  sessionId: string,
  entries: unknown[],
  done: boolean,
) => ({
  session_id: sessionId,
  entries,
  folders_done: true,
  done,
});

function emitFolderChunk(sessionId: string, entries: unknown[], done: boolean) {
  const handler = eventHandlers.get("browser:folder-chunk");
  if (!handler) throw new Error("folder-chunk listener not registered");
  act(() => {
    handler({ payload: playableChunk(sessionId, entries, done) });
  });
}

function emitFileChanged() {
  const handler = eventHandlers.get("browser:file-change");
  if (!handler) throw new Error("file-change listener not registered");
  act(() => {
    handler({ payload: { path: "/music" } });
  });
}

const fileEntry = (id: string, sizeBytes: number, modifiedAtMs: number) => ({
  id,
  name: id.split("/").filter(Boolean).at(-1) ?? id,
  kind: "playable",
  metadata: {
    duration_ms: 60_000,
    size_bytes: sizeBytes,
    modified_at_ms: modifiedAtMs,
    channels: 2,
    sample_rate: 44_100,
    bit_depth: 16,
    codec: "mp3",
  },
});

/** Enumeration mock: records requested paths and hands back session ids. */
let sessionsByPath = new Map<string, string[]>();

function installBackendMock() {
  sessionsByPath = new Map<string, string[]>();
  vi.mocked(invoke).mockImplementation(async (command: string, args) => {
    if (command === "load_player_preferences") {
      return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
    }
    if (command === "save_player_preferences") {
      return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
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
        case "record_recent_folder":
          return ok({});
        case "list_folder_bookmarks":
          return ok({ bookmarks: [] });
        case "cancel_enumeration":
          return ok({});
        case "start_enumeration": {
          const path = String(envelope.payload.path);
          const sessionId = `folder-${sessionsByPath.get(path)?.length ?? 0}`;
          sessionsByPath.set(path, [
            ...(sessionsByPath.get(path) ?? []),
            sessionId,
          ]);
          return ok({ session_id: sessionId });
        }
        default:
          return undefined;
      }
    }
    return undefined;
  });
}

// ── Test ───────────────────────────────────────────────────────────────

describe("external rename keeps the session mark and refreshes the row", () => {
  it("transfers the mark to the renamed row after a Finder rename", async () => {
    installBackendMock();
    render(<App />);

    // Mounted roots are visible immediately; open /music directly.
    await waitFor(() => {
      expect(screen.getByText("Music")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("Music"));

    // Enumeration 1 streams a.mp3 and completes (two-phase: preview folders,
    // then verified files with metadata, then the done marker).
    await waitFor(() => {
      const sessions = sessionsByPath.get("/music") ?? [];
      expect(sessions.length).toBe(1);
    });
    emitFolderChunk(sessionsByPath.get("/music")?.[0] as string, [], false);
    emitFolderChunk(
      sessionsByPath.get("/music")?.[0] as string,
      [fileEntry("/music/a.mp3", 2048, 1_700_000_000_000)],
      false,
    );
    emitFolderChunk(sessionsByPath.get("/music")?.[0] as string, [], true);

    // Select and mark the file.
    await waitFor(() => {
      expect(screen.getByText("a.mp3")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("a.mp3"));
    fireEvent.click(screen.getByRole("button", { name: "Mark Keep" }));

    // External rename in Finder triggers the watcher refresh.
    emitFileChanged();

    // Enumeration 2 streams the renamed file b.mp3 (same size/mtime).
    await waitFor(() => {
      const sessions = sessionsByPath.get("/music") ?? [];
      expect(sessions.length).toBe(2);
    });
    emitFolderChunk(sessionsByPath.get("/music")?.[1] as string, [], false);
    emitFolderChunk(
      sessionsByPath.get("/music")?.[1] as string,
      [fileEntry("/music/b.mp3", 2048, 1_700_000_000_000)],
      false,
    );
    emitFolderChunk(sessionsByPath.get("/music")?.[1] as string, [], true);

    // The row is renamed and still carries the mark.
    await waitFor(() => {
      expect(screen.getByText("b.mp3")).toBeInTheDocument();
    });
    expect(screen.queryByText("a.mp3")).not.toBeInTheDocument();
    const row = screen.getByRole("row", { name: /b\.mp3/ });
    expect(row.querySelector(".file-list-mark-dot--keep")).not.toBeNull();
  });
});
