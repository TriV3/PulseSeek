import { describe, expect, it } from "vitest";
import { INITIAL_FOLDER_TREE_STATE } from "../components/FolderTree/folderTreeTypes";
import { folderTreeReducer } from "./useFolderTree";

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
