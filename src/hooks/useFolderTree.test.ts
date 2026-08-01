import { describe, expect, it } from "vitest";
import { INITIAL_FOLDER_TREE_STATE } from "../components/FolderTree/folderTreeTypes";
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
        },
      },
      playableEntries: {
        [path]: [{ id: "old", name: "old.wav", kind: "playable" }],
      },
    },
    { type: "START_ENUMERATING", path, sessionId: "session-1" },
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
      error: null,
    });

    const childLoading = folderTreeReducer(parent, {
      type: "START_ENUMERATING",
      path: "/test/music/one",
      sessionId: "session-2",
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
});
