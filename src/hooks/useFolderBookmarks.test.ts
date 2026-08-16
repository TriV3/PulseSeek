import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useFolderBookmarks } from "./useFolderBookmarks";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  add: vi.fn(),
  remove: vi.fn(),
}));

vi.mock("../api/commandEnvelope", () => ({
  listFolderBookmarks: api.list,
  addFolderBookmark: api.add,
  removeFolderBookmark: api.remove,
}));

describe("useFolderBookmarks", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.list.mockResolvedValue([]);
    api.add.mockResolvedValue(undefined);
    api.remove.mockResolvedValue(undefined);
  });

  it("persists a bookmark and removes it with the same toggle", async () => {
    const { result } = renderHook(() => useFolderBookmarks());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(() => result.current.toggle("/Music/Synths"));
    expect(api.add).toHaveBeenCalledWith("/Music/Synths");
    expect(result.current.isBookmarked("/Music/Synths")).toBe(true);

    await act(() => result.current.toggle("/Music/Synths"));
    expect(api.remove).toHaveBeenCalledWith("/Music/Synths");
    expect(result.current.isBookmarked("/Music/Synths")).toBe(false);
  });

  it("keeps the UI unchanged when persistence fails", async () => {
    api.add.mockRejectedValueOnce(new Error("cache unavailable"));
    const { result } = renderHook(() => useFolderBookmarks());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(() => result.current.toggle("/Music"));
    expect(result.current.bookmarks).toEqual([]);
    expect(result.current.error).toBe("Could not update bookmarks.");
  });
});
