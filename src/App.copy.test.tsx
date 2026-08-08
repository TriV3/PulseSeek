import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
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

let sessionsByPath = new Map<string, string[]>();
const copyStarts: Array<{ paths: string[]; target_dir: string }> = [];

function installBackendMock() {
  sessionsByPath = new Map<string, string[]>();
  copyStarts.length = 0;
  vi.mocked(invoke).mockImplementation(async (command: string, args) => {
    if (command === "load_player_preferences") {
      return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
    }
    if (command === "save_player_preferences") {
      return { version: 1, preferences: DEFAULT_PLAYER_PREFERENCES };
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
        case "start_copy_files": {
          copyStarts.push({
            paths: envelope.payload.paths as string[],
            target_dir: String(envelope.payload.target_dir),
          });
          return ok({ session_id: "copy-1" });
        }
        case "cancel_copy_files":
          return ok({});
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

function emitCopyProgress(payload: unknown) {
  const handler = eventHandlers.get("browser:copy-progress");
  if (!handler) throw new Error("copy-progress listener not registered");
  act(() => {
    handler({ payload });
  });
}

async function openMusicFolder() {
  await waitFor(() => {
    expect(screen.getByText("Music")).toBeInTheDocument();
  });
  fireEvent.click(screen.getByText("Music"));

  await waitFor(() => {
    const sessions = sessionsByPath.get("/music") ?? [];
    expect(sessions.length).toBe(1);
  });
  emitFolderChunk(sessionsByPath.get("/music")?.[0] as string, [], false);
  emitFolderChunk(
    sessionsByPath.get("/music")?.[0] as string,
    [
      fileEntry("/music/a.mp3", 2048, 1_700_000_000_000),
      fileEntry("/music/b.mp3", 4096, 1_700_000_000_001),
    ],
    false,
  );
  emitFolderChunk(sessionsByPath.get("/music")?.[0] as string, [], true);
  await waitFor(() => {
    expect(screen.getByText("a.mp3")).toBeInTheDocument();
  });
}

// ── Test ───────────────────────────────────────────────────────────────

describe("copy keeps originals and reports partial failure separately", () => {
  it("copies selected files and keeps every row visible", async () => {
    installBackendMock();
    render(<App />);
    await openMusicFolder();

    // Multi-select both playable rows (FR-FM-005).
    fireEvent.click(screen.getByText("a.mp3"));
    fireEvent.click(screen.getByText("b.mp3"), { ctrlKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Copy…" }));

    const dialog = screen.getByRole("alertdialog");
    expect(dialog).toHaveTextContent("Copy 2 files into a folder.");
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Choose folder…" }),
    );
    await waitFor(() => {
      expect(within(dialog).getByText("/library")).toBeInTheDocument();
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Copy" }));

    await waitFor(() => {
      expect(copyStarts).toHaveLength(1);
    });
    expect(copyStarts[0].paths).toEqual(["/music/a.mp3", "/music/b.mp3"]);
    expect(copyStarts[0].target_dir).toBe("/library");

    // First file copies; second collides (partial failure).
    await waitFor(() => {
      expect(eventHandlers.has("browser:copy-progress")).toBe(true);
    });
    emitCopyProgress({
      session_id: "copy-1",
      completed: 1,
      total: 2,
      done: false,
      results: [{ path: "/music/a.mp3", new_path: "/library/a.mp3", ok: true }],
    });
    emitCopyProgress({
      session_id: "copy-1",
      completed: 2,
      total: 2,
      done: true,
      results: [
        { path: "/music/a.mp3", new_path: "/library/a.mp3", ok: true },
        {
          path: "/music/b.mp3",
          ok: false,
          category: "Conflict",
          message: "PulseSeek could not apply that change.",
          diagnostic_code: "file.operation",
        },
      ],
    });

    // Originals remain in the view: copy never removes a row.
    expect(screen.getAllByText("a.mp3").length).toBeGreaterThan(0);
    expect(screen.getAllByText("b.mp3").length).toBeGreaterThan(0);

    const summary = screen.getByRole("alertdialog");
    expect(within(summary).getByText("1 file copied.")).toBeInTheDocument();
    expect(
      within(summary).getByText("1 file could not be copied:"),
    ).toBeInTheDocument();
  });
});
