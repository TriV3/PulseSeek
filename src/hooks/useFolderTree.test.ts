import { describe, expect, it } from "vitest";
import {
  collectFolderEntries,
  INITIAL_FOLDER_TREE_STATE,
  relativeEntryPath,
} from "../components/FolderTree/folderTreeTypes";
import { folderTreeReducer, getRestorationPaths } from "./useFolderTree";

const path = "/test/music";

function enumeratingState() {
  return folderTreeReducer(
    {
      ...INITIAL_FOLDER_TREE_STATE,
      rootPath: path,
      selectedPath: path,
      folders: {
        [path]: {
          expanded: true,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
      playableEntries: {
        [path]: [{ id: "old", name: "old.wav", kind: "playable" }],
      },
    },
    {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-1",
      recursive: false,
    },
  );
}

describe("folderTreeReducer playable entries", () => {
  it("never scans the synthetic Computer root during restoration", () => {
    expect(getRestorationPaths("/Users/me/Music")).toEqual([
      "/",
      "/Users",
      "/Users/me",
      "/Users/me/Music",
    ]);
  });

  it("checks every parent so a missing saved folder can fall back", () => {
    expect(getRestorationPaths("/Volumes/NAS/music/album")).toEqual([
      "/",
      "/Volumes",
      "/Volumes/NAS",
      "/Volumes/NAS/music",
      "/Volumes/NAS/music/album",
    ]);
  });

  it("loads every system root into a collapsed Computer root", () => {
    const state = folderTreeReducer(INITIAL_FOLDER_TREE_STATE, {
      type: "ROOTS_LOADED",
      roots: [
        { path: "/", name: "System" },
        { path: "/Volumes/NAS", name: "NAS" },
      ],
    });

    expect(state.rootPath).toBe("computer://");
    expect(state.folders["computer://"]?.children).toEqual([
      { id: "/", name: "System", kind: "folder" },
      { id: "/Volumes/NAS", name: "NAS", kind: "folder" },
    ]);
    expect(state.folders["computer://"]?.expanded).toBe(false);
    expect(state.selectedPath).toBe("computer://");
    expect(state.folders["/"]?.expanded).toBe(false);
  });

  it("restores the selected folder and only the saved expanded paths", () => {
    const roots = folderTreeReducer(INITIAL_FOLDER_TREE_STATE, {
      type: "ROOTS_LOADED",
      roots: [{ path: "/", name: "System" }],
    });

    const restored = folderTreeReducer(roots, {
      type: "RESTORE_CONTEXT",
      selectedPath: "/music/album",
      expandedPaths: ["computer://", "/", "/music"],
    });

    expect(restored.selectedPath).toBe("/music/album");
    expect(restored.folders["computer://"]?.expanded).toBe(true);
    expect(restored.folders["/music"]?.expanded).toBe(true);
    expect(restored.folders["/music/album"]).toBeUndefined();
  });

  it("resets stale entries when enumeration starts", () => {
    const state = enumeratingState();

    expect(state.playableEntries[path]).toEqual([]);
  });

  it("accumulates playable entries across incremental chunks", () => {
    const firstChunk = folderTreeReducer(enumeratingState(), {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: "one", name: "one.wav", kind: "playable" },
        { id: "folder", name: "Samples", kind: "folder" },
      ],
      done: false,
    });

    const complete = folderTreeReducer(firstChunk, {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: "two", name: "two.flac", kind: "playable" },
        { id: "hidden", name: "notes.txt", kind: "unsupported" },
      ],
      done: true,
    });

    expect(complete.playableEntries[path]?.map((entry) => entry.name)).toEqual([
      "one.wav",
      "two.flac",
    ]);
    expect(complete.folders[path]?.children).toEqual([
      { id: "folder", name: "Samples", kind: "folder" },
    ]);
    expect(complete.folders[path]?.isLoading).toBe(false);
  });

  it("stays loading after the folder preview until file scanning is complete", () => {
    const preview = folderTreeReducer(enumeratingState(), {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [],
      foldersDone: true,
      done: false,
    });

    expect(preview.folders[path]?.isLoading).toBe(true);
    expect(preview.status).toBe("loading");
  });

  it("replaces preview entries with metadata and removes rejected candidates", () => {
    const preview = folderTreeReducer(enumeratingState(), {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: "preview", name: "preview.wav", kind: "playable" },
        { id: "rejected", name: "rejected.wav", kind: "playable" },
      ],
      done: false,
      foldersDone: true,
    });
    const verified = folderTreeReducer(preview, {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        {
          id: "preview",
          name: "preview.wav",
          kind: "playable",
          metadata: {
            duration_ms: 1_000,
            size_bytes: 100,
            modified_at_ms: null,
            channels: 2,
            sample_rate: 44_100,
            bit_depth: 16,
            codec: "wav",
          },
        },
        { id: "rejected", name: "rejected.wav", kind: "unsupported" },
      ],
      done: true,
      foldersDone: true,
    });

    expect(verified.playableEntries[path]).toHaveLength(1);
    expect(verified.playableEntries[path]?.[0].metadata?.duration_ms).toBe(
      1_000,
    );
    expect(verified.folders[path]?.isLoading).toBe(false);
  });

  it("registers discovered folders so navigation can continue past two levels", () => {
    const parent = folderTreeReducer(enumeratingState(), {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [{ id: "/test/music/one", name: "one", kind: "folder" }],
      done: true,
    });

    expect(parent.folders["/test/music/one"]).toEqual({
      expanded: false,
      children: [],
      isLoading: false,
      hasLoaded: false,
      hasSubfolders: null,
      error: null,
      recursive: false,
    });

    const childLoading = folderTreeReducer(parent, {
      type: "START_ENUMERATING",
      path: "/test/music/one",
      sessionId: "session-2",
      recursive: false,
    });
    const child = folderTreeReducer(childLoading, {
      type: "ENUMERATION_CHUNK",
      path: "/test/music/one",
      entries: [{ id: "/test/music/one/two", name: "two", kind: "folder" }],
      done: true,
    });

    expect(child.folders["/test/music/one/two"]).toBeDefined();
  });

  it("removes trashed entries without changing unrelated entries", () => {
    const state = {
      ...INITIAL_FOLDER_TREE_STATE,
      playableEntries: {
        [path]: [
          { id: "keep", name: "keep.wav", kind: "playable" as const },
          { id: "trash", name: "trash.wav", kind: "playable" as const },
        ],
        "/other": [
          { id: "other", name: "other.wav", kind: "playable" as const },
        ],
      },
    };

    const next = folderTreeReducer(state, {
      type: "REMOVE_ENTRIES",
      path,
      entryIds: ["trash"],
    });

    expect(next.playableEntries[path]).toEqual([
      { id: "keep", name: "keep.wav", kind: "playable" },
    ]);
    expect(next.playableEntries["/other"]).toEqual([
      { id: "other", name: "other.wav", kind: "playable" },
    ]);
  });

  it("renames an entry id and name in place", () => {
    const state = {
      ...INITIAL_FOLDER_TREE_STATE,
      folders: {
        [path]: {
          expanded: true,
          children: [
            { id: "folder-a", name: "folder-a", kind: "folder" as const },
            { id: "song.wav", name: "song.wav", kind: "playable" as const },
          ],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
      playableEntries: {
        [path]: [
          { id: "song.wav", name: "song.wav", kind: "playable" as const },
        ],
      },
    };

    const next = folderTreeReducer(state, {
      type: "RENAME_ENTRY",
      path,
      oldId: "song.wav",
      newId: "renamed.wav",
      newName: "renamed.wav",
    });

    expect(next.playableEntries[path]).toEqual([
      { id: "renamed.wav", name: "renamed.wav", kind: "playable" },
    ]);
    expect(next.folders[path]?.children).toEqual([
      { id: "folder-a", name: "folder-a", kind: "folder" },
      { id: "renamed.wav", name: "renamed.wav", kind: "playable" },
    ]);
  });

  it("keeps metadata when renaming an entry", () => {
    const state = {
      ...INITIAL_FOLDER_TREE_STATE,
      playableEntries: {
        [path]: [
          {
            id: "song.wav",
            name: "song.wav",
            kind: "playable" as const,
            metadata: {
              duration_ms: 1234,
              size_bytes: 99,
              modified_at_ms: null,
              channels: null,
              sample_rate: null,
              bit_depth: null,
              codec: null,
            },
          },
        ],
      },
    };

    const next = folderTreeReducer(state, {
      type: "RENAME_ENTRY",
      path,
      oldId: "song.wav",
      newId: "renamed.wav",
      newName: "renamed.wav",
    });

    expect(next.playableEntries[path]?.[0]?.metadata).toEqual({
      duration_ms: 1234,
      size_bytes: 99,
      modified_at_ms: null,
      channels: null,
      sample_rate: null,
      bit_depth: null,
      codec: null,
    });
  });

  it("is a no-op when the folder is unknown", () => {
    const next = folderTreeReducer(INITIAL_FOLDER_TREE_STATE, {
      type: "RENAME_ENTRY",
      path,
      oldId: "song.wav",
      newId: "renamed.wav",
      newName: "renamed.wav",
    });
    expect(next).toBe(INITIAL_FOLDER_TREE_STATE);
  });

  it("rebuilds entries with the new name after an external rename", () => {
    // An external rename (Finder) changes the stable entry id. The watcher
    // triggers a fresh enumeration which resets the list and streams the
    // renamed entry under its new id, so the visible name refreshes.
    const state = {
      ...INITIAL_FOLDER_TREE_STATE,
      folders: {
        [path]: {
          expanded: true,
          children: [],
          isLoading: false,
          error: null,
          recursive: false,
        },
      },
      playableEntries: {
        [path]: [
          { id: "/test/music/a.mp3", name: "a.mp3", kind: "playable" as const },
        ],
      },
    };

    const started = folderTreeReducer(state, {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-2",
      recursive: false,
    });
    expect(started.playableEntries[path]).toEqual([]);

    const rebuilt = folderTreeReducer(started, {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: "/test/music/b.mp3", name: "b.mp3", kind: "playable" as const },
      ],
      done: true,
    });

    expect(rebuilt.playableEntries[path]).toEqual([
      { id: "/test/music/b.mp3", name: "b.mp3", kind: "playable" },
    ]);
  });

  it("marks the folder recursive when a recursive enumeration starts", () => {
    const state = folderTreeReducer(INITIAL_FOLDER_TREE_STATE, {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-1",
      recursive: true,
    });

    expect(state.folders[path]?.recursive).toBe(true);
    expect(state.playableEntries[path]).toEqual([]);
  });

  it("accumulates recursive subtree files across chunks", () => {
    const started = folderTreeReducer(INITIAL_FOLDER_TREE_STATE, {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-1",
      recursive: true,
    });

    const first = folderTreeReducer(started, {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: "/test/music/a/one.wav", name: "one.wav", kind: "playable" },
      ],
      done: false,
    });
    const complete = folderTreeReducer(first, {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: "/test/music/two.wav", name: "two.wav", kind: "playable" },
      ],
      done: true,
    });

    expect(complete.playableEntries[path]?.map((entry) => entry.id)).toEqual([
      "/test/music/a/one.wav",
      "/test/music/two.wav",
    ]);
    expect(complete.folders[path]?.isLoading).toBe(false);
    expect(complete.folders[path]?.recursive).toBe(true);
  });

  it("re-enumerating without recursion clears the recursive flag", () => {
    const recursive = folderTreeReducer(INITIAL_FOLDER_TREE_STATE, {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-1",
      recursive: true,
    });

    const flat = folderTreeReducer(recursive, {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-2",
      recursive: false,
    });

    expect(flat.folders[path]?.recursive).toBe(false);
  });

  it("keeps stable playable entries when re-enumerating after a file change", () => {
    // Folder already shows one playable file when an external change event
    // triggers a re-enumeration (FR-BR-008). The stable entry id must survive
    // the refresh so the current selection is retained.
    const loaded = folderTreeReducer(enumeratingState(), {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: `${path}/stable.wav`, name: "stable.wav", kind: "playable" },
      ],
      foldersDone: true,
      done: true,
    });

    const restart = folderTreeReducer(loaded, {
      type: "START_ENUMERATING",
      path,
      sessionId: "session-2",
      recursive: false,
    });
    expect(restart.playableEntries[path]).toEqual([]);

    const refreshed = folderTreeReducer(restart, {
      type: "ENUMERATION_CHUNK",
      path,
      entries: [
        { id: `${path}/stable.wav`, name: "stable.wav", kind: "playable" },
        { id: `${path}/new.wav`, name: "new.wav", kind: "playable" },
      ],
      foldersDone: true,
      done: true,
    });

    expect(refreshed.playableEntries[path]?.map((entry) => entry.id)).toEqual([
      `${path}/new.wav`,
      `${path}/stable.wav`,
    ]);
  });
});

describe("relativeEntryPath", () => {
  it("returns the id relative to the selected folder", () => {
    expect(relativeEntryPath("/music/album/one.wav", "/music/album")).toBe(
      "one.wav",
    );
    expect(relativeEntryPath("/music/album/sub/two.wav", "/music/album")).toBe(
      "sub/two.wav",
    );
  });

  it("falls back to the file name for entries outside the selected folder", () => {
    expect(relativeEntryPath("/other/x.wav", "/music/album")).toBe("x.wav");
  });

  it("returns the id when no folder is selected", () => {
    expect(relativeEntryPath("/music/one.wav", "")).toBe("one.wav");
  });
});

describe("collectFolderEntries", () => {
  it("collects folders from every explored folder, not only the current one", () => {
    const folders = {
      "/music": {
        expanded: true,
        children: [
          { id: "/music/a", name: "a", kind: "folder" as const },
          {
            id: "/music/song.wav",
            name: "song.wav",
            kind: "playable" as const,
          },
        ],
        isLoading: false,
        error: null,
        recursive: false,
      },
      "/music/a": {
        expanded: true,
        children: [
          {
            id: "/music/a/Downloads",
            name: "Downloads",
            kind: "folder" as const,
          },
        ],
        isLoading: false,
        error: null,
        recursive: false,
      },
    };

    const collected = collectFolderEntries(folders);

    expect(collected.map((entry) => entry.id)).toEqual([
      "/music/a",
      "/music/a/Downloads",
    ]);
  });

  it("deduplicates folders that appear in more than one explored folder", () => {
    const folders = {
      "/music": {
        expanded: true,
        children: [
          { id: "/music/loop", name: "loop", kind: "folder" as const },
        ],
        isLoading: false,
        error: null,
        recursive: false,
      },
      "/sounds": {
        expanded: true,
        children: [
          { id: "/music/loop", name: "loop", kind: "folder" as const },
        ],
        isLoading: false,
        error: null,
        recursive: false,
      },
    };

    expect(collectFolderEntries(folders)).toEqual([
      { id: "/music/loop", name: "loop", kind: "folder" },
    ]);
  });

  it("returns an empty array when nothing is explored", () => {
    expect(collectFolderEntries({})).toEqual([]);
  });
});
