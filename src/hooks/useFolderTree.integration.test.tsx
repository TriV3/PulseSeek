import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  FileChangePayload,
  FolderChunkPayload,
} from "../api/playbackEvents";
import { useFolderTree } from "./useFolderTree";

const mocks = vi.hoisted(() => ({
  cancelEnumeration: vi.fn(),
  listBrowserRoots: vi.fn(),
  startEnumeration: vi.fn(),
  folderChunkHandler: null as ((payload: FolderChunkPayload) => void) | null,
  fileChangeHandler: null as ((payload: FileChangePayload) => void) | null,
}));

vi.mock("../api/commandEnvelope", () => ({
  cancelEnumeration: mocks.cancelEnumeration,
  listBrowserRoots: mocks.listBrowserRoots,
  startEnumeration: mocks.startEnumeration,
}));

vi.mock("../api/playbackEvents", () => ({
  onFileChanged: vi.fn((handler: (payload: FileChangePayload) => void) => {
    mocks.fileChangeHandler = handler;
    return Promise.resolve(() => {});
  }),
  onFolderChunk: vi.fn((handler: (payload: FolderChunkPayload) => void) => {
    mocks.folderChunkHandler = handler;
    return Promise.resolve(() => {});
  }),
}));

describe("useFolderTree leaf selection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.folderChunkHandler = null;
    mocks.fileChangeHandler = null;
    mocks.listBrowserRoots.mockResolvedValue({
      roots: [{ path: "/music", name: "Music", kind: "physical" }],
      libraries: [],
    });
    mocks.startEnumeration.mockResolvedValue("folder-1");
  });

  it("enumerates audio files when selecting a known leaf folder", async () => {
    const { result } = renderHook(() => useFolderTree());

    await waitFor(() => {
      expect(result.current.state.folders["/music"]).toBeDefined();
      expect(mocks.folderChunkHandler).not.toBeNull();
    });

    act(() => {
      result.current.toggleExpand("/music");
    });
    await waitFor(() => expect(mocks.startEnumeration).toHaveBeenCalled());

    act(() => {
      mocks.folderChunkHandler?.({
        session_id: "folder-1",
        entries: [
          {
            id: "/music/leaf",
            name: "leaf",
            kind: "folder",
            has_subfolders: false,
          },
        ],
        folders_done: true,
        done: true,
      });
    });
    await waitFor(() => {
      expect(result.current.state.folders["/music/leaf"]?.hasSubfolders).toBe(
        false,
      );
    });

    mocks.startEnumeration.mockResolvedValueOnce("folder-2");
    act(() => {
      result.current.selectFolder("/music/leaf");
    });

    await waitFor(() => {
      expect(mocks.startEnumeration).toHaveBeenCalledWith(
        "/music/leaf",
        undefined,
        false,
        false,
      );
    });
  });

  it("coalesces watcher refreshes and ignores an event for a non-selected folder", async () => {
    const { result } = renderHook(() => useFolderTree());
    await waitFor(() => expect(mocks.fileChangeHandler).not.toBeNull());

    act(() => result.current.selectFolder("/music"));

    let resolveRefresh: ((session: string) => void) | undefined;
    mocks.startEnumeration.mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveRefresh = resolve;
        }),
    );

    act(() => {
      mocks.fileChangeHandler?.({ path: "/music" });
      mocks.fileChangeHandler?.({ path: "/music" });
      mocks.fileChangeHandler?.({ path: "/old-folder" });
    });

    await waitFor(() =>
      expect(mocks.startEnumeration).toHaveBeenCalledTimes(1),
    );
    act(() => resolveRefresh?.("refresh-1"));

    await act(async () => {
      await Promise.resolve();
      mocks.fileChangeHandler?.({ path: "/music" });
    });
    expect(mocks.startEnumeration).toHaveBeenCalledTimes(1);

    act(() => {
      mocks.folderChunkHandler?.({
        session_id: "refresh-1",
        entries: [],
        folders_done: true,
        done: true,
      });
    });
    mocks.startEnumeration.mockResolvedValueOnce("refresh-2");
    act(() => mocks.fileChangeHandler?.({ path: "/music" }));
    await waitFor(() =>
      expect(mocks.startEnumeration).toHaveBeenCalledTimes(2),
    );
  });
});
