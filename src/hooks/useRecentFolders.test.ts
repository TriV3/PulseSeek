import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RECENT_FOLDERS_LIMIT, useRecentFolders } from "./useRecentFolders";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  record: vi.fn(),
  clear: vi.fn(),
}));

vi.mock("../api/commandEnvelope", () => ({
  listRecentFolders: api.list,
  recordRecentFolder: api.record,
  clearRecentFolders: api.clear,
}));

beforeEach(() => {
  vi.resetAllMocks();
  api.list.mockResolvedValue([]);
  api.record.mockResolvedValue(undefined);
  api.clear.mockResolvedValue(undefined);
});

describe("useRecentFolders", () => {
  it("loads the persisted history on mount", async () => {
    api.list.mockResolvedValueOnce([
      { path: "/music/project", name: "project", last_opened_ms: 200 },
      { path: "/music/album", name: "album", last_opened_ms: 100 },
    ]);

    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.folders).toEqual([
      { path: "/music/project", name: "project", last_opened_ms: 200 },
      { path: "/music/album", name: "album", last_opened_ms: 100 },
    ]);
  });

  it("records a folder optimistically at the front", async () => {
    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.record("/music/new", "new"));

    expect(result.current.folders[0]).toMatchObject({
      path: "/music/new",
      name: "new",
    });
    expect(api.record).toHaveBeenCalledWith("/music/new");
  });

  it("deduplicates and bounds the optimistic list", async () => {
    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    for (let index = 0; index < RECENT_FOLDERS_LIMIT + 3; index += 1) {
      act(() =>
        result.current.record(`/music/folder-${index}`, `folder-${index}`),
      );
    }
    act(() => result.current.record("/music/folder-5", "folder-5"));

    expect(result.current.folders).toHaveLength(RECENT_FOLDERS_LIMIT);
    expect(result.current.folders[0]).toMatchObject({
      path: "/music/folder-5",
    });
    expect(
      result.current.folders.some(
        (folder) => folder.path === "/music/folder-0",
      ),
    ).toBe(false);
  });

  it("keeps browsing when a record write fails", async () => {
    api.record.mockRejectedValueOnce(new Error("backend unavailable"));
    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.record("/music/project", "project"));

    expect(result.current.folders[0].path).toBe("/music/project");
    expect(result.current.error).toBeNull();
  });

  it("never records virtual browser roots", async () => {
    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.record("computer://"));

    expect(result.current.folders).toEqual([]);
    expect(api.record).not.toHaveBeenCalled();
  });

  it("clears the history only after backend confirmation", async () => {
    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.record("/music/project", "project"));

    let cleared = false;
    await act(async () => {
      cleared = await result.current.clear();
    });

    expect(cleared).toBe(true);
    expect(result.current.folders).toEqual([]);
    expect(api.clear).toHaveBeenCalledTimes(1);
  });

  it("reports a failed clear without wiping the local list", async () => {
    api.clear.mockRejectedValueOnce(new Error("cache unavailable"));
    const { result } = renderHook(() => useRecentFolders());
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    act(() => result.current.record("/music/project", "project"));

    let cleared = true;
    await act(async () => {
      cleared = await result.current.clear();
    });

    expect(cleared).toBe(false);
    expect(result.current.folders).toHaveLength(1);
    expect(result.current.error).toBe("cache unavailable");
  });
});
