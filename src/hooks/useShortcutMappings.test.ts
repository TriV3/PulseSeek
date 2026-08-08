import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SHORTCUTS } from "../shortcuts/keyboardShortcuts";
import { useShortcutMappings } from "./useShortcutMappings";

const api = vi.hoisted(() => ({
  load: vi.fn(),
  save: vi.fn(),
  reset: vi.fn(),
}));

vi.mock("../api/commandEnvelope", () => ({
  loadShortcuts: api.load,
  saveShortcuts: api.save,
  resetShortcuts: api.reset,
}));

describe("useShortcutMappings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.load.mockResolvedValue(DEFAULT_SHORTCUTS);
  });

  it("uses defaults immediately and loads once", async () => {
    const { result, rerender } = renderHook(() => useShortcutMappings());

    expect(result.current.bindings).toEqual(DEFAULT_SHORTCUTS);
    expect(result.current.isLoading).toBe(true);
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    rerender();
    expect(api.load).toHaveBeenCalledOnce();
  });

  it("keeps defaults active and exposes safe load failure", async () => {
    api.load.mockRejectedValueOnce(new Error("private backend detail"));
    const { result } = renderHook(() => useShortcutMappings());

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.bindings).toEqual(DEFAULT_SHORTCUTS);
    expect(result.current.error).toBe("Keyboard shortcuts unavailable.");
  });

  it("applies only backend-confirmed saves", async () => {
    const requested = {
      ...DEFAULT_SHORTCUTS,
      open_folder: { key: "p", primary: true, shift: false, alt: false },
    };
    const confirmed = {
      ...DEFAULT_SHORTCUTS,
      open_folder: { key: "b", primary: true, shift: false, alt: false },
    };
    api.save.mockResolvedValueOnce(confirmed);
    const { result } = renderHook(() => useShortcutMappings());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(() => result.current.save(requested));

    expect(result.current.bindings).toEqual(confirmed);
  });

  it("keeps confirmed bindings when save fails", async () => {
    api.save.mockRejectedValueOnce(new Error("secret path"));
    const { result } = renderHook(() => useShortcutMappings());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(() => result.current.save(DEFAULT_SHORTCUTS));

    expect(result.current.bindings).toEqual(DEFAULT_SHORTCUTS);
    expect(result.current.error).toBe("Could not save keyboard shortcuts.");
  });

  it("applies backend-confirmed reset defaults", async () => {
    api.reset.mockResolvedValueOnce(DEFAULT_SHORTCUTS);
    const { result } = renderHook(() => useShortcutMappings());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(() => result.current.reset());

    expect(api.reset).toHaveBeenCalledOnce();
    expect(result.current.bindings).toEqual(DEFAULT_SHORTCUTS);
  });

  it("keeps confirmed bindings when reset fails", async () => {
    api.reset.mockRejectedValueOnce(new Error("private reset detail"));
    const { result } = renderHook(() => useShortcutMappings());
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(() => result.current.reset());

    expect(result.current.bindings).toEqual(DEFAULT_SHORTCUTS);
    expect(result.current.error).toBe("Could not reset keyboard shortcuts.");
  });
});
