import { renderHook, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { usePlaybackMode } from "./usePlaybackMode";

const setPlaybackModeMock = vi.hoisted(() => vi.fn());
vi.mock("../api/commandEnvelope", () => ({
  setPlaybackMode: setPlaybackModeMock,
}));

beforeEach(() => vi.resetAllMocks());

describe("usePlaybackMode", () => {
  it("starts in one-shot and reflects confirmed mode", async () => {
    setPlaybackModeMock.mockResolvedValue("loop-current");
    const { result } = renderHook(() => usePlaybackMode());

    expect(result.current.mode).toBe("one-shot");
    await act(async () => result.current.selectMode("loop-current"));

    expect(setPlaybackModeMock).toHaveBeenCalledWith("loop-current");
    expect(result.current.mode).toBe("loop-current");
  });

  it("rolls back and reports command failure", async () => {
    setPlaybackModeMock.mockRejectedValue(new Error("mode failed"));
    const { result } = renderHook(() => usePlaybackMode());

    await act(async () => result.current.selectMode("random"));

    expect(result.current.mode).toBe("one-shot");
    expect(result.current.error).toBe("mode failed");
    expect(result.current.isChanging).toBe(false);
  });

  it("ignores stale mode results", async () => {
    let resolveFirst: (mode: "loop-current") => void = () => undefined;
    setPlaybackModeMock
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValueOnce("random");
    const { result } = renderHook(() => usePlaybackMode());

    await act(async () => {
      void result.current.selectMode("loop-current");
      await result.current.selectMode("random");
    });
    await act(async () => resolveFirst("loop-current"));

    expect(result.current.mode).toBe("random");
    expect(result.current.error).toBeNull();
    expect(result.current.isChanging).toBe(false);
  });
});
