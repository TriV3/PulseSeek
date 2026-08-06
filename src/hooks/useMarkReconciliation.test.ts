import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";
import {
  INITIAL_FOLDER_TREE_STATE,
  type FolderTreeState,
} from "../components/FolderTree/folderTreeTypes";
import { useMarkReconciliation } from "./useMarkReconciliation";
import { useSessionMarks } from "./useSessionMarks";
import { folderTreeReducer } from "./useFolderTree";

const path = "/music";

const metadata = (sizeBytes: number, modifiedAtMs: number) => ({
  duration_ms: null,
  size_bytes: sizeBytes,
  modified_at_ms: modifiedAtMs,
  channels: null,
  sample_rate: null,
  bit_depth: null,
  codec: null,
});

const entry = (id: string, size: number, modified: number): BrowserEntry => ({
  id,
  name: id,
  kind: "playable",
  metadata: metadata(size, modified),
});

describe("useMarkReconciliation", () => {
  it("transfers a mark to the renamed entry after a completed enumeration", () => {
    const replaceMarks = vi.fn();
    const oldEntry = entry(`${path}/a.mp3`, 2048, 1_700_000_000_000);
    const newEntry = entry(`${path}/b.mp3`, 2048, 1_700_000_000_000);
    const { rerender } = renderHook(
      ({ entries }) =>
        useMarkReconciliation(
          path,
          "ready",
          { [path]: entries },
          { [`${path}/a.mp3`]: "keep" },
          replaceMarks,
        ),
      { initialProps: { entries: [oldEntry] } },
    );

    rerender({ entries: [newEntry] });

    expect(replaceMarks).toHaveBeenCalledTimes(1);
    expect(replaceMarks).toHaveBeenCalledWith({ [`${path}/b.mp3`]: "keep" });
  });

  it("does not transfer while an enumeration is still loading", () => {
    const replaceMarks = vi.fn();
    const oldEntry = entry(`${path}/a.mp3`, 2048, 1_700_000_000_000);
    const newEntry = entry(`${path}/b.mp3`, 2048, 1_700_000_000_000);
    const { rerender } = renderHook(
      (props: { entries: BrowserEntry[]; status: FolderTreeState["status"] }) =>
        useMarkReconciliation(
          path,
          props.status,
          { [path]: props.entries },
          { [`${path}/a.mp3`]: "keep" },
          replaceMarks,
        ),
      {
        initialProps: {
          entries: [oldEntry],
          status: "ready",
        },
      },
    );

    rerender({
      entries: [newEntry],
      status: "loading",
    });

    expect(replaceMarks).not.toHaveBeenCalled();
  });

  it("does not transfer when the entries are unchanged", () => {
    const replaceMarks = vi.fn();
    const oldEntry = entry(`${path}/a.mp3`, 2048, 1_700_000_000_000);
    const { rerender } = renderHook(
      ({ entries }) =>
        useMarkReconciliation(
          path,
          "ready",
          { [path]: entries },
          { [`${path}/a.mp3`]: "keep" },
          replaceMarks,
        ),
      { initialProps: { entries: [oldEntry] } },
    );

    rerender({ entries: [oldEntry] });

    expect(replaceMarks).not.toHaveBeenCalled();
  });

  it("reconciles only the selected folder", () => {
    const replaceMarks = vi.fn();
    const oldEntry = entry(`${path}/a.mp3`, 2048, 1_700_000_000_000);
    const { rerender } = renderHook(
      ({ selected }) =>
        useMarkReconciliation(
          selected,
          "ready",
          { [path]: [oldEntry] },
          { [`${path}/a.mp3`]: "keep" },
          replaceMarks,
        ),
      { initialProps: { selected: "/other" } },
    );

    act(() => {
      // The selected folder switches to the folder whose entries changed.
      rerender({ selected: path });
    });

    expect(replaceMarks).not.toHaveBeenCalled();
  });
});

describe("useMarkReconciliation with the real reducer flow", () => {
  function Harness() {
    const session = useSessionMarks();
    const [tree, setTree] = useState<FolderTreeState>(
      INITIAL_FOLDER_TREE_STATE,
    );
    useMarkReconciliation(
      tree.selectedPath,
      tree.status,
      tree.playableEntries,
      session.marks,
      session.replace,
    );
    return { session, tree, setTree };
  }

  const selectedState = () => ({
    ...INITIAL_FOLDER_TREE_STATE,
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
  });

  it("keeps the mark across an external rename in the full flow", () => {
    const { result } = renderHook(() => Harness());

    // Enumeration 1 completes with the original file.
    act(() => {
      result.current.setTree(() =>
        folderTreeReducer(selectedState(), {
          type: "START_ENUMERATING",
          path,
          sessionId: "session-1",
          recursive: false,
        }),
      );
    });
    act(() => {
      result.current.setTree((t) =>
        folderTreeReducer(t, {
          type: "ENUMERATION_CHUNK",
          path,
          entries: [entry(`${path}/a.mp3`, 2048, 1_700_000_000_000)],
          done: true,
        }),
      );
    });

    // The user marks the file.
    act(() => {
      result.current.session.setMark([`${path}/a.mp3`], "keep");
    });
    expect(result.current.session.marks).toEqual({ [`${path}/a.mp3`]: "keep" });

    // The watcher re-enumerates after an external rename (a.mp3 -> b.mp3).
    act(() => {
      result.current.setTree((t) =>
        folderTreeReducer(t, {
          type: "START_ENUMERATING",
          path,
          sessionId: "session-2",
          recursive: false,
        }),
      );
    });
    act(() => {
      result.current.setTree((t) =>
        folderTreeReducer(t, {
          type: "ENUMERATION_CHUNK",
          path,
          entries: [entry(`${path}/b.mp3`, 2048, 1_700_000_000_000)],
          done: true,
        }),
      );
    });

    expect(result.current.tree.playableEntries[path]).toEqual([
      entry(`${path}/b.mp3`, 2048, 1_700_000_000_000),
    ]);
    expect(result.current.session.marks).toEqual({ [`${path}/b.mp3`]: "keep" });
  });
});
