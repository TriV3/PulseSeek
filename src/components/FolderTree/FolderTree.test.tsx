import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { FolderTree } from "./FolderTree";

// ── Mock Tauri APIs ────────────────────────────────────────────────────

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Mock } from "vitest";
import type { FolderChunkPayload } from "../../api/playbackEvents";
import type { Event as TauriEvent } from "@tauri-apps/api/event";

// ── Test helpers ───────────────────────────────────────────────────────

type ChunkHandler = (event: TauriEvent<FolderChunkPayload>) => void;
let folderChunkHandler: ChunkHandler | undefined;

/**
 * Configures mocks so the component can communicate with the fake backend.
 *
 * Callers must trigger chunk events themselves via `emitChunk()` after
 * the folder has been picked and enumeration has started.
 */
function setupMocks() {
  folderChunkHandler = undefined;

  (listen as Mock).mockImplementation(
    async (event: string, handler: unknown) => {
      if (event === "browser:folder-chunk") {
        folderChunkHandler = handler as ChunkHandler;
      }
      return () => {};
    },
  );

  (invoke as Mock).mockImplementation(async (cmd: string, args: unknown) => {
    if (cmd === "invoke_command") {
      const envelope = (
        args as {
          envelope: { command: string; payload: Record<string, unknown> };
        }
      ).envelope;

      if (envelope.command === "pick_folder") {
        return { version: 1, ok: true, data: { path: "/test/music" } };
      }

      if (envelope.command === "start_enumeration") {
        const payload = envelope.payload as { path: string };
        return {
          version: 1,
          ok: true,
          data: { session_id: `session-${payload.path}` },
        };
      }

      if (envelope.command === "cancel_enumeration") {
        return { version: 1, ok: true, data: {} };
      }
    }

    throw new Error(`unexpected invoke: ${cmd}`);
  });
}

/** Simulate a `browser:folder-chunk` event from the Rust backend. */
function emitChunk(
  sessionId: string,
  entries: Array<{ name: string; kind: "folder" | "playable" }>,
  done: boolean,
) {
  folderChunkHandler?.({
    payload: {
      session_id: sessionId,
      entries: entries.map((e) => ({ id: e.name, name: e.name, kind: e.kind })),
      done,
    },
    id: 1,
    event: "browser:folder-chunk",
  });
}

// ── Tests ──────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks();
  (invoke as Mock).mockImplementation(async () => {
    throw new Error("unexpected invoke");
  });
  (listen as Mock).mockImplementation(async () => () => {});
});

describe("FolderTree — initial state", () => {
  it("renders an Open Folder button when no folder is selected", () => {
    render(<FolderTree />);
    expect(
      screen.getByRole("button", { name: "Open Folder" }),
    ).toBeInTheDocument();
  });

  it("renders a tree with accessible label", () => {
    render(<FolderTree />);
    expect(
      screen.getByRole("tree", { name: "Folder browser" }),
    ).toBeInTheDocument();
  });
});

describe("FolderTree — opening a folder", () => {
  it("opens the folder picker when the button is clicked", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    // Wait for the picker to return — root path shown in toolbar.
    await waitFor(() => {
      expect(screen.getByText("/test/music")).toBeInTheDocument();
    });

    expect(screen.getByText("music")).toBeInTheDocument();
  });

  it("disables the button while picking", () => {
    setupMocks();

    render(<FolderTree />);
    const btn = screen.getByRole("button", { name: "Open Folder" });

    fireEvent.click(btn);
    expect(btn).toBeDisabled();
  });
});

describe("FolderTree — folder expansion", () => {
  it("shows subfolder children after expanding", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    // Wait for root to load, then emit child folder names.
    await waitFor(() => {
      expect(screen.getByText("music")).toBeInTheDocument();
    });

    emitChunk(
      "session-/test/music",
      [
        { name: "Sub1", kind: "folder" },
        { name: "Sub2", kind: "folder" },
      ],
      true,
    );

    await waitFor(() => {
      expect(screen.getByText("Sub1")).toBeInTheDocument();
      expect(screen.getByText("Sub2")).toBeInTheDocument();
    });

    const toggles = screen.getAllByRole("button", { name: "Expand folder" });
    expect(toggles.length).toBeGreaterThan(0);
  });

  it("collapses children when toggle is clicked again", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    await waitFor(() => {
      expect(screen.getByText("music")).toBeInTheDocument();
    });

    emitChunk("session-/test/music", [{ name: "Sub1", kind: "folder" }], true);

    await waitFor(() => {
      expect(screen.getByText("Sub1")).toBeInTheDocument();
    });

    // Collapse the root folder.
    const collapseBtn = screen.getByRole("button", { name: "Collapse folder" });
    fireEvent.click(collapseBtn);

    expect(screen.queryByText("Sub1")).not.toBeInTheDocument();
  });
});

describe("FolderTree — selection", () => {
  it("selects a folder when its name is clicked", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    await waitFor(() => {
      expect(screen.getByText("music")).toBeInTheDocument();
    });

    emitChunk("session-/test/music", [{ name: "Sub1", kind: "folder" }], true);

    await waitFor(() => {
      expect(screen.getByText("Sub1")).toBeInTheDocument();
    });

    // Click the subfolder name.
    fireEvent.click(screen.getByText("Sub1"));

    // Sub1 treeitem should be selected.
    const items = screen.getAllByRole("treeitem");
    const selectedItem = items.find(
      (item) => item.getAttribute("aria-selected") === "true",
    );
    expect(selectedItem).toBeTruthy();
    expect(selectedItem).toHaveTextContent("Sub1");
  });
});

describe("FolderTree — keyboard navigation", () => {
  it("moves selection with arrow keys", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    await waitFor(() => {
      expect(screen.getByText("music")).toBeInTheDocument();
    });

    emitChunk(
      "session-/test/music",
      [
        { name: "Sub1", kind: "folder" },
        { name: "Sub2", kind: "folder" },
      ],
      true,
    );

    await waitFor(() => {
      expect(screen.getByText("Sub1")).toBeInTheDocument();
      expect(screen.getByText("Sub2")).toBeInTheDocument();
    });

    // Focus the tree and press ArrowDown.
    const tree = screen.getByRole("tree");
    tree.focus();
    fireEvent.keyDown(tree, { key: "ArrowDown" });

    const items = screen.getAllByRole("treeitem");
    const selected = items.find(
      (item) => item.getAttribute("aria-selected") === "true",
    );
    expect(selected).toBeTruthy();
  });
});

describe("FolderTree — loading state", () => {
  it("shows a loading indicator during enumeration", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    // After picker returns but before chunk, root folder is loading.
    await waitFor(() => {
      expect(screen.getByLabelText("Loading")).toBeInTheDocument();
    });
  });
});

describe("FolderTree — error state", () => {
  it("displays an error banner when enumeration fails", async () => {
    (invoke as Mock).mockImplementation(async (cmd: string, args: unknown) => {
      if (cmd === "invoke_command") {
        const envelope = (
          args as { envelope: { command: string; payload: unknown } }
        ).envelope;

        if (envelope.command === "pick_folder") {
          return { version: 1, ok: true, data: { path: "/test/music" } };
        }

        if (envelope.command === "start_enumeration") {
          return {
            version: 1,
            ok: false,
            error: {
              category: "PermissionDenied",
              message: "Cannot read folder.",
              diagnostic_code: "browser.read",
            },
          };
        }
      }

      throw new Error(`unexpected invoke: ${cmd}`);
    });

    (listen as Mock).mockImplementation(async () => () => {});

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    await waitFor(() => {
      expect(screen.getAllByRole("alert").length).toBe(1);
    });

    // Error message appears in both the banner and the folder node.
    expect(
      screen.getAllByText("Cannot read folder.").length,
    ).toBeGreaterThanOrEqual(1);
  });
});

describe("FolderTree — empty folder", () => {
  it("shows (empty) placeholder when no subfolders exist", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    await waitFor(() => {
      expect(screen.getByText("music")).toBeInTheDocument();
    });

    // Emit chunk with no folder entries.
    emitChunk("session-/test/music", [], true);

    await waitFor(() => {
      expect(screen.getByText("(empty)")).toBeInTheDocument();
    });
  });
});

describe("FolderTree — navigate up", () => {
  it("renders a Go Up button when a folder is selected", async () => {
    setupMocks();

    render(<FolderTree />);
    fireEvent.click(screen.getByRole("button", { name: "Open Folder" }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Go to parent folder" }),
      ).toBeInTheDocument();
    });
  });
});
