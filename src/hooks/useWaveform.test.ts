import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { useWaveform } from "./useWaveform";
import { getWaveform, type WaveformLevel } from "../api/waveform";
import {
  onWaveformReady,
  type WaveformReadyPayload,
} from "../api/playbackEvents";

vi.mock("../api/waveform", () => ({
  getWaveform: vi.fn(),
}));
vi.mock("../api/playbackEvents", () => ({
  onWaveformReady: vi.fn(),
}));

const LEVEL: WaveformLevel = {
  format_version: 1,
  channels: 1,
  samples_per_peak: 10,
  min: [-0.5, 0, 0.5],
  max: [-0.4, 0.1, 0.6],
};

let waveformReadyHandler: ((payload: WaveformReadyPayload) => void) | null;

function deferred() {
  let resolve!: (value: WaveformLevel) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<WaveformLevel>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getWaveform).mockReset();
  waveformReadyHandler = null;
  vi.mocked(onWaveformReady).mockImplementation(async (handler) => {
    waveformReadyHandler = handler;
    return () => {};
  });
});

describe("useWaveform", () => {
  it("subscribes for completion before starting the waveform request", async () => {
    let resolveListener!: (unlisten: () => void) => void;
    vi.mocked(onWaveformReady).mockReturnValue(
      new Promise((resolve) => {
        resolveListener = resolve;
      }),
    );
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);

    renderHook(() => useWaveform("/music/track.wav", 64));
    await waitFor(() => expect(onWaveformReady).toHaveBeenCalled());

    expect(getWaveform).not.toHaveBeenCalled();
    await act(async () => resolveListener(() => {}));
    await waitFor(() => expect(getWaveform).toHaveBeenCalledOnce());
  });

  it("stays idle without a file path and never fetches", () => {
    const { result } = renderHook(() => useWaveform(null, 64));
    expect(result.current.status).toBe("idle");
    expect(getWaveform).not.toHaveBeenCalled();
  });

  it("stays idle with a non-positive target", () => {
    const { result } = renderHook(() => useWaveform("/music/track.wav", 0));
    expect(result.current.status).toBe("idle");
    expect(getWaveform).not.toHaveBeenCalled();
  });

  it("loads the waveform for the selected file", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const { result } = renderHook(() => useWaveform("/music/track.wav", 64));

    expect(result.current.status).toBe("loading");
    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(result.current.waveform).toEqual(LEVEL);
    expect(result.current.error).toBeNull();
    expect(getWaveform).toHaveBeenCalledWith("/music/track.wav", 64);
  });

  it("reports a user-facing error when loading fails", async () => {
    vi.mocked(getWaveform).mockRejectedValue(new Error("corrupt file"));
    const { result } = renderHook(() => useWaveform("/music/track.wav", 64));

    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.error).toBe("corrupt file");
    expect(result.current.waveform).toBeNull();
  });

  it("re-fetches when the file path changes", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const { result, rerender } = renderHook(
      ({ path }) => useWaveform(path, 64),
      { initialProps: { path: "/music/a.wav" } },
    );
    await waitFor(() => expect(result.current.status).toBe("ready"));

    rerender({ path: "/music/b.wav" });
    expect(result.current.status).toBe("loading");
    expect(result.current.waveform).toBeNull();
    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(getWaveform).toHaveBeenCalledTimes(2);
    expect(getWaveform).toHaveBeenLastCalledWith("/music/b.wav", 64);
  });

  it("ignores a stale response from the previous file", async () => {
    const first = deferred();
    const second = deferred();
    vi.mocked(getWaveform)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);

    const { result, rerender } = renderHook(
      ({ path }) => useWaveform(path, 64),
      { initialProps: { path: "/music/a.wav" } },
    );
    expect(result.current.status).toBe("loading");
    await waitFor(() => expect(getWaveform).toHaveBeenCalledOnce());

    rerender({ path: "/music/b.wav" });
    first.resolve(LEVEL);
    await waitFor(() => expect(result.current.status).toBe("loading"));

    second.resolve({ ...LEVEL, samples_per_peak: 20 });
    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(result.current.waveform?.samples_per_peak).toBe(20);
  });

  it("re-fetches when the target resolution changes", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const { result, rerender } = renderHook(
      ({ target }) => useWaveform("/music/track.wav", target),
      { initialProps: { target: 64 } },
    );
    await waitFor(() => expect(result.current.status).toBe("ready"));

    rerender({ target: 128 });
    expect(result.current.status).toBe("loading");
    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(getWaveform).toHaveBeenLastCalledWith("/music/track.wav", 128);
  });

  it("keeps the preview visible while refining the same file", async () => {
    const refinement = deferred();
    vi.mocked(getWaveform)
      .mockResolvedValueOnce(LEVEL)
      .mockReturnValueOnce(refinement.promise);
    const { result, rerender } = renderHook(
      ({ target }) => useWaveform("/music/track.wav", target),
      { initialProps: { target: 128 } },
    );
    await waitFor(() => expect(result.current.status).toBe("ready"));

    rerender({ target: 600 });

    expect(result.current.status).toBe("loading");
    expect(result.current.waveform).toEqual(LEVEL);
  });

  it("replaces the preview when the exact waveform becomes ready", async () => {
    const exact = {
      ...LEVEL,
      samples_per_peak: 2,
      min: [-0.8, -0.4, 0.2, 0.5],
      max: [-0.2, 0.1, 0.6, 0.9],
    };
    vi.mocked(getWaveform)
      .mockResolvedValueOnce(LEVEL)
      .mockResolvedValueOnce(exact);
    const { result } = renderHook(() => useWaveform("/music/track.wav", 600));
    await waitFor(() => expect(result.current.waveform).toEqual(LEVEL));
    await waitFor(() => expect(waveformReadyHandler).not.toBeNull());

    act(() => waveformReadyHandler?.({ path: "/music/track.wav" }));

    await waitFor(() => expect(result.current.waveform).toEqual(exact));
    expect(getWaveform).toHaveBeenCalledTimes(2);
    expect(getWaveform).toHaveBeenLastCalledWith("/music/track.wav", 600);
  });

  it("returns to idle when the selection is cleared", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const { result, rerender } = renderHook(
      ({ path }: { path: string | null }) => useWaveform(path, 64),
      { initialProps: { path: "/music/a.wav" as string | null } },
    );
    await waitFor(() => expect(result.current.status).toBe("ready"));

    rerender({ path: null });
    expect(result.current.status).toBe("idle");
    expect(result.current.waveform).toBeNull();
  });
});
