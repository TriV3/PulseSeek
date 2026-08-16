import { renderHook, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";
import { usePlaybackTransport } from "./usePlaybackTransport";

const pauseMock = vi.hoisted(() => vi.fn());
const resumeMock = vi.hoisted(() => vi.fn());
const stopMock = vi.hoisted(() => vi.fn());
const seekMock = vi.hoisted(() => vi.fn());
const setVolumeMock = vi.hoisted(() => vi.fn());
const setLoopRegionMock = vi.hoisted(() => vi.fn());
const clearLoopRegionMock = vi.hoisted(() => vi.fn());
const onStateChangedMock = vi.hoisted(() => vi.fn());
const onPositionMock = vi.hoisted(() => vi.fn());
const onCompletedMock = vi.hoisted(() => vi.fn());
const prepareNextMock = vi.hoisted(() => vi.fn());
const onTrackChangedMock = vi.hoisted(() => vi.fn());
const clearPreparedMock = vi.hoisted(() => vi.fn());

vi.mock("../api/commandEnvelope", () => ({
  pause: pauseMock,
  resume: resumeMock,
  stop: stopMock,
  seek: seekMock,
  setVolume: setVolumeMock,
  setLoopRegion: setLoopRegionMock,
  clearLoopRegion: clearLoopRegionMock,
  prepareNext: prepareNextMock,
  clearPrepared: clearPreparedMock,
}));
vi.mock("../api/playbackEvents", () => ({
  onStateChanged: onStateChangedMock,
  onPosition: onPositionMock,
  onCompleted: onCompletedMock,
  onTrackChanged: onTrackChangedMock,
}));

const entries: BrowserEntry[] = [
  { id: "a.wav", name: "a.wav", kind: "playable" },
  { id: "b.wav", name: "b.wav", kind: "playable" },
];

beforeEach(() => {
  vi.resetAllMocks();
  onStateChangedMock.mockResolvedValue(() => undefined);
  onPositionMock.mockResolvedValue(() => undefined);
  onCompletedMock.mockResolvedValue(() => undefined);
  prepareNextMock.mockResolvedValue(undefined);
  onTrackChangedMock.mockResolvedValue(() => undefined);
  clearPreparedMock.mockResolvedValue(undefined);
});

describe("usePlaybackTransport", () => {
  it("pauses playing playback and resumes paused playback", async () => {
    pauseMock.mockResolvedValue(undefined);
    resumeMock.mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ status }: { status: "playing" | "paused" }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId: "a.wav",
          playbackStatus: status,
          onSelectEntry: vi.fn(),
        }),
      {
        initialProps: {
          status: "playing" as "playing" | "paused",
        },
      },
    );

    await act(async () => result.current.togglePlayPause());
    expect(pauseMock).toHaveBeenCalledOnce();
    rerender({ status: "playing" });
    await act(async () => result.current.togglePlayPause());
    expect(resumeMock).toHaveBeenCalledOnce();
  });

  it("dispatches stop and resets position", async () => {
    stopMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.handleStop());

    expect(stopMock).toHaveBeenCalledOnce();
    expect(result.current.positionMs).toBe(0);
    expect(result.current.status).toBe("idle");
  });

  it("starts the selected track again after stop", async () => {
    stopMock.mockResolvedValue(undefined);
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "one-shot",
        onSelectEntry,
      }),
    );

    await act(async () => result.current.handleStop());
    await act(async () => result.current.togglePlayPause());

    expect(onSelectEntry).toHaveBeenCalledWith(entries[0]);
    expect(result.current.hasSelection).toBe(true);
  });

  it("advances to the next track after completion in sequential mode", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "sequential",
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(entries[1]);
  });

  it("applies gapless track identity, position, and duration atomically", async () => {
    let trackChanged:
      | ((payload: { path: string; duration_ms: number | null }) => void)
      | undefined;
    onTrackChangedMock.mockImplementationOnce(
      async (
        handler: (payload: {
          path: string;
          duration_ms: number | null;
        }) => void,
      ) => {
        trackChanged = handler;
        return () => undefined;
      },
    );
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ selectedEntryId }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackStatus: "playing",
          playbackMode: "sequential",
          onSelectEntry,
        }),
      { initialProps: { selectedEntryId: "a.wav" } },
    );
    await act(async () => undefined);

    await act(async () => {
      trackChanged?.({ path: "b.wav", duration_ms: 4_000 });
    });
    rerender({ selectedEntryId: "b.wav" });

    expect(onSelectEntry).toHaveBeenCalledWith(entries[1], {
      alreadyPlaying: true,
    });
    expect(result.current.positionMs).toBe(0);
    expect(result.current.durationMs).toBe(4_000);
  });

  it("keeps the completion listener while switching from one-shot to sequential", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(
      ({ playbackMode }: { playbackMode: "one-shot" | "sequential" }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId: "a.wav",
          playbackStatus: "playing",
          playbackMode,
          onSelectEntry,
        }),
      {
        initialProps: {
          playbackMode: "one-shot" as "one-shot" | "sequential",
        },
      },
    );

    rerender({ playbackMode: "sequential" });
    await act(async () => complete?.());

    expect(onCompletedMock).toHaveBeenCalledOnce();
    expect(onSelectEntry).toHaveBeenCalledWith(entries[1]);
  });

  it("selects another track after completion in random mode", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const random = vi.fn().mockReturnValue(0);
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "random",
        random,
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(entries[1]);
    expect(random).toHaveBeenCalledOnce();
  });

  it("selects a deterministic random alternative without immediate repeat", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const random = vi.fn().mockReturnValue(0.99);
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries: [...entries, { id: "c.wav", name: "c.wav", kind: "playable" }],
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "random",
        random,
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(
      expect.objectContaining({ id: "c.wav" }),
    );
  });

  it("reselects the only playable item in random mode", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const random = vi.fn().mockReturnValue(0.5);
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries: [entries[0]],
        selectedEntryId: entries[0].id,
        playbackStatus: "playing",
        playbackMode: "random",
        random,
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(entries[0]);
  });

  it("chooses from current visible playable entries after removal", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const random = vi.fn().mockReturnValue(0);
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(
      ({ currentEntries }: { currentEntries: BrowserEntry[] }) =>
        usePlaybackTransport({
          entries: currentEntries,
          selectedEntryId: "a.wav",
          playbackStatus: "playing",
          playbackMode: "random",
          random,
          onSelectEntry,
        }),
      { initialProps: { currentEntries: entries } },
    );

    rerender({
      currentEntries: [
        entries[0],
        { id: "c.wav", name: "c.wav", kind: "playable" },
      ],
    });
    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(
      expect.objectContaining({ id: "c.wav" }),
    );
    expect(onSelectEntry).not.toHaveBeenCalledWith(entries[1]);
  });

  it("does not advance after stop when a stale completion arrives", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "sequential",
        onSelectEntry,
      }),
    );

    stopMock.mockResolvedValue(undefined);
    await act(async () => result.current.handleStop());
    await act(async () => complete?.());

    expect(onSelectEntry).not.toHaveBeenCalled();
  });

  it("does not advance a newer session from an older completion", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementation(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(
      ({
        selectedEntryId,
        generation,
      }: {
        selectedEntryId: string;
        generation: number;
      }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackGeneration: generation,
          playbackStatus: "playing",
          playbackMode: "sequential",
          onSelectEntry,
        }),
      { initialProps: { selectedEntryId: "a.wav", generation: 1 } },
    );

    rerender({ selectedEntryId: "b.wav", generation: 2 });
    await act(async () => complete?.());

    expect(onSelectEntry).not.toHaveBeenCalled();
  });

  it("invalidates completion before an in-flight Stop command resolves", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    let resolveStop: (() => void) | undefined;
    stopMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveStop = resolve;
      }),
    );
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "sequential",
        onSelectEntry,
      }),
    );

    let stopping: Promise<void>;
    act(() => {
      stopping = result.current.handleStop();
    });
    await act(async () => complete?.());
    expect(onSelectEntry).not.toHaveBeenCalled();

    await act(async () => {
      resolveStop?.();
      await stopping!;
    });
  });

  it("follows the supplied browser ordering rather than entry identifiers", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const orderedEntries = [entries[1], entries[0]];
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries: orderedEntries,
        selectedEntryId: "b.wav",
        playbackStatus: "playing",
        playbackMode: "sequential",
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(entries[0]);
  });

  it("advances through the visible list after filtering removes an entry", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const filteredEntries: BrowserEntry[] = [
      entries[0],
      { id: "c.wav", name: "c.wav", kind: "playable" },
    ];
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries: filteredEntries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "sequential",
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(filteredEntries[1]);
  });

  it("skips a removed next item and advances to the next visible playable item", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { rerender } = renderHook(
      ({ entries: currentEntries }: { entries: BrowserEntry[] }) =>
        usePlaybackTransport({
          entries: currentEntries,
          selectedEntryId: "a.wav",
          playbackStatus: "playing",
          playbackMode: "sequential",
          onSelectEntry,
        }),
      { initialProps: { entries } },
    );

    rerender({
      entries: [entries[0], { id: "c.wav", name: "c.wav", kind: "playable" }],
    });
    await act(async () => complete?.());

    expect(onSelectEntry).toHaveBeenCalledWith(
      expect.objectContaining({ id: "c.wav" }),
    );
  });

  it("stops at the last visible playable item", async () => {
    let complete: (() => void) | undefined;
    onCompletedMock.mockImplementationOnce(async (handler: () => void) => {
      complete = handler;
      return () => undefined;
    });
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    renderHook(() =>
      usePlaybackTransport({
        entries: [
          { id: "folder", name: "folder", kind: "folder" },
          { id: "a.wav", name: "a.wav", kind: "playable" },
        ],
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        playbackMode: "sequential",
        onSelectEntry,
      }),
    );

    await act(async () => complete?.());

    expect(onSelectEntry).not.toHaveBeenCalled();
  });

  it("seeks, changes volume, and mutes", async () => {
    seekMock.mockResolvedValue(12_000);
    setVolumeMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.handleSeek(12_000));
    await act(async () => result.current.handleVolume(0.5));
    await act(async () => result.current.toggleMute());

    expect(seekMock).toHaveBeenCalledWith(12_000);
    expect(setVolumeMock).toHaveBeenNthCalledWith(1, 0.5, false);
    expect(setVolumeMock).toHaveBeenNthCalledWith(2, 0.5, true);
  });

  it("surfaces a failed seek without discarding the selection", async () => {
    seekMock.mockRejectedValue(new Error("seek failed"));
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.handleSeek(4_000));

    expect(seekMock).toHaveBeenCalledWith(4_000);
    expect(result.current.error).toBe("seek failed");
    expect(result.current.hasSelection).toBe(true);
  });

  it("selects previous and next visible entries", async () => {
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ selectedEntryId }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackStatus: "playing",
          onSelectEntry,
        }),
      { initialProps: { selectedEntryId: "a.wav" } },
    );

    await act(async () => result.current.handleNext());
    expect(onSelectEntry).toHaveBeenCalledWith(entries[1]);
    rerender({ selectedEntryId: "b.wav" });
    expect(result.current.canPrevious).toBe(true);
    expect(result.current.canNext).toBe(false);
  });

  it("skips folder entries when advancing to the next file", async () => {
    const mixedEntries: BrowserEntry[] = [
      { id: "folder1", name: "folder1", kind: "folder" },
      { id: "a.wav", name: "a.wav", kind: "playable" },
      { id: "folder2", name: "folder2", kind: "folder" },
      { id: "b.wav", name: "b.wav", kind: "playable" },
    ];
    const onSelectEntry = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries: mixedEntries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry,
      }),
    );

    expect(result.current.canNext).toBe(true);
    await act(async () => result.current.handleNext());

    expect(onSelectEntry).toHaveBeenCalledWith(
      expect.objectContaining({ id: "b.wav" }),
    );
  });

  it("keeps command errors visible", async () => {
    stopMock.mockRejectedValue(new Error("stop failed"));
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.handleStop());

    expect(result.current.error).toBe("stop failed");
  });

  it("rolls back optimistic volume changes when command fails", async () => {
    setVolumeMock.mockRejectedValueOnce(new Error("volume failed"));
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.handleVolume(0.5));

    expect(result.current.volume).toBe(1);
    expect(result.current.error).toBe("volume failed");
  });

  it("resets position metadata when selection changes", () => {
    const { result, rerender } = renderHook(
      ({ selectedEntryId }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackStatus: "playing",
          onSelectEntry: vi.fn(),
        }),
      { initialProps: { selectedEntryId: "a.wav" } },
    );

    rerender({ selectedEntryId: "b.wav" });

    expect(result.current.positionMs).toBe(0);
    expect(result.current.durationMs).toBeNull();
  });

  it("places a single A-B point without confirming a region", async () => {
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));

    expect(result.current.abPoints).toEqual({ startMs: 1_000, endMs: null });
    expect(result.current.loopRegion).toBeNull();
    expect(setLoopRegionMock).not.toHaveBeenCalled();
  });

  it("keeps a complete pending region without an active session", async () => {
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "idle",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));

    expect(setLoopRegionMock).not.toHaveBeenCalled();
    expect(result.current.abPoints).toEqual({ startMs: 1_000, endMs: 5_000 });
    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 5_000 });
    expect(result.current.abError).toBeNull();
  });

  it("confirms a region once both valid points are placed", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));

    expect(setLoopRegionMock).toHaveBeenCalledWith(1_000, 5_000);
    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 5_000 });
    expect(result.current.abError).toBeNull();
  });

  it("activates completed A-B points immediately while playing", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));

    expect(setLoopRegionMock).toHaveBeenLastCalledWith(1_000, 5_000);
    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 5_000 });
  });

  it("rejects a reversed A-B pair and keeps the previous points", async () => {
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 5_000));
    await act(async () => result.current.setAbPoint("b", 1_000));

    expect(setLoopRegionMock).not.toHaveBeenCalled();
    expect(result.current.abPoints).toEqual({ startMs: 5_000, endMs: null });
    expect(result.current.loopRegion).toBeNull();
    expect(result.current.abError).toMatch(/after the A point/i);
  });

  it("rejects an equal A-B pair", async () => {
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 2_000));
    await act(async () => result.current.setAbPoint("b", 2_000));

    expect(setLoopRegionMock).not.toHaveBeenCalled();
    expect(result.current.abPoints).toEqual({ startMs: 2_000, endMs: null });
  });

  it("reverts the placed point and surfaces the error when the backend rejects", async () => {
    setLoopRegionMock.mockRejectedValue(new Error("region out of bounds"));
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));

    expect(result.current.loopRegion).toBeNull();
    expect(result.current.abPoints).toEqual({ startMs: 1_000, endMs: null });
    expect(result.current.abError).toBe("region out of bounds");
  });

  it("clears the region and points through the backend", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    clearLoopRegionMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));

    await act(async () => result.current.clearAB());

    expect(clearLoopRegionMock).toHaveBeenCalledOnce();
    expect(result.current.loopRegion).toBeNull();
    expect(result.current.abPoints).toEqual({ startMs: null, endMs: null });
  });

  it("keeps the region when a confirmed seek lands inside it", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    seekMock.mockResolvedValue(2_000);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    await act(async () => result.current.handleSeek(2_000));

    expect(clearLoopRegionMock).not.toHaveBeenCalled();
    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 5_000 });
  });

  it("restores region when seek lands outside it", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    clearLoopRegionMock.mockResolvedValue(undefined);
    seekMock.mockResolvedValue(8_000);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    await act(async () => result.current.handleSeek(8_000));
    await act(async () => undefined);

    expect(clearLoopRegionMock).not.toHaveBeenCalled();
    expect(setLoopRegionMock).toHaveBeenCalledTimes(2);
    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 5_000 });
    expect(result.current.abPoints).toEqual({ startMs: 1_000, endMs: 5_000 });
  });

  it("does not restore A-B after clear wins the seek race", async () => {
    let resolveRestore: (() => void) | undefined;
    setLoopRegionMock.mockResolvedValueOnce(1_000).mockResolvedValueOnce(
      new Promise<number>((resolve) => {
        resolveRestore = () => resolve(1_000);
      }),
    );
    seekMock.mockResolvedValue(8_000);
    clearLoopRegionMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    await act(async () => result.current.handleSeek(8_000));
    await act(async () => result.current.clearAB());
    await act(async () => {
      resolveRestore?.();
      await Promise.resolve();
    });

    expect(result.current.abPoints).toEqual({ startMs: null, endMs: null });
    expect(result.current.loopRegion).toBeNull();
  });

  it("resets A-B points and region when the selection changes", async () => {
    const { result, rerender } = renderHook(
      ({ selectedEntryId }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackStatus: "playing",
          onSelectEntry: vi.fn(),
        }),
      { initialProps: { selectedEntryId: "a.wav" } },
    );
    await act(async () => result.current.setAbPoint("a", 1_000));

    rerender({ selectedEntryId: "b.wav" });

    expect(result.current.abPoints).toEqual({ startMs: null, endMs: null });
    expect(result.current.loopRegion).toBeNull();
  });

  it("preserves A-B points, region, and duration on stop", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    stopMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    act(() => result.current.restorePosition("a.wav", 2_000, 10_000));
    await act(async () => result.current.handleStop());

    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 5_000 });
    expect(result.current.abPoints).toEqual({ startMs: 1_000, endMs: 5_000 });
    expect(result.current.durationMs).toBe(10_000);
  });

  it("places B while stopped and keeps the pending region", async () => {
    stopMock.mockResolvedValue(undefined);
    setLoopRegionMock.mockResolvedValue(1_000);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    act(() => result.current.restorePosition("a.wav", 4_000, 10_000));
    await act(async () => result.current.handleStop());
    await act(async () => result.current.setAbPoint("b", 4_000));

    expect(setLoopRegionMock).toHaveBeenCalledTimes(0);
    expect(result.current.abPoints).toEqual({ startMs: 1_000, endMs: 4_000 });
    expect(result.current.loopRegion).toEqual({ startMs: 1_000, endMs: 4_000 });
  });

  it("moves the playhead locally while stopped so a point can be placed", async () => {
    stopMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );
    act(() => result.current.restorePosition("a.wav", 2_000, 10_000));
    await act(async () => result.current.handleStop());

    let confirmed: number | null = null;
    await act(async () => {
      confirmed = await result.current.handleSeek(4_000);
    });

    expect(seekMock).not.toHaveBeenCalled();
    expect(confirmed).toBe(4_000);
    expect(result.current.positionMs).toBe(4_000);
  });

  it("reapplies a stopped A-B region to the next playback worker", async () => {
    stopMock.mockResolvedValue(undefined);
    setLoopRegionMock.mockResolvedValue(1_000);
    const { result, rerender } = renderHook(
      ({
        status,
        generation,
      }: {
        status: "playing" | "loading";
        generation: number;
      }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId: "a.wav",
          playbackStatus: status,
          playbackGeneration: generation,
          onSelectEntry: vi.fn(),
        }),
      {
        initialProps: {
          status: "playing" as "playing" | "loading",
          generation: 1,
        },
      },
    );
    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    await act(async () => result.current.handleStop());
    setLoopRegionMock.mockClear();

    rerender({ status: "loading", generation: 2 });
    rerender({ status: "playing", generation: 2 });
    await act(async () => undefined);

    expect(setLoopRegionMock).toHaveBeenCalledOnce();
    expect(setLoopRegionMock).toHaveBeenCalledWith(1_000, 5_000);
  });

  it("applies a region created while idle when playback starts", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    const { result, rerender } = renderHook(
      ({
        status,
        generation,
      }: {
        status: "idle" | "playing";
        generation: number;
      }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId: "a.wav",
          playbackStatus: status,
          playbackGeneration: generation,
          onSelectEntry: vi.fn(),
        }),
      {
        initialProps: {
          status: "idle" as "idle" | "playing",
          generation: 1,
        },
      },
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    expect(setLoopRegionMock).not.toHaveBeenCalled();

    rerender({ status: "playing", generation: 2 });
    await act(async () => undefined);

    expect(setLoopRegionMock).toHaveBeenCalledOnce();
    expect(setLoopRegionMock).toHaveBeenCalledWith(1_000, 5_000);
  });

  it("never combines hidden points from two selected files", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    const { result, rerender } = renderHook(
      ({ selectedEntryId }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackStatus: "playing",
          onSelectEntry: vi.fn(),
        }),
      { initialProps: { selectedEntryId: "a.wav" } },
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    rerender({ selectedEntryId: "b.wav" });
    await act(async () => result.current.setAbPoint("b", 5_000));

    expect(setLoopRegionMock).not.toHaveBeenCalled();
    expect(result.current.abPoints).toEqual({ startMs: null, endMs: 5_000 });
  });

  it("ignores a stale region response after selection changes", async () => {
    let resolveRegion: ((value: number) => void) | undefined;
    setLoopRegionMock.mockImplementation(
      () =>
        new Promise<number>((resolve) => {
          resolveRegion = resolve;
        }),
    );
    const { result, rerender } = renderHook(
      ({ selectedEntryId }) =>
        usePlaybackTransport({
          entries,
          selectedEntryId,
          playbackStatus: "playing",
          onSelectEntry: vi.fn(),
        }),
      { initialProps: { selectedEntryId: "a.wav" } },
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    let pending: Promise<boolean>;
    act(() => {
      pending = result.current.setAbPoint("b", 5_000);
    });
    rerender({ selectedEntryId: "b.wav" });
    await act(async () => {
      resolveRegion?.(1_000);
      await pending!;
    });

    expect(result.current.abPoints).toEqual({ startMs: null, endMs: null });
    expect(result.current.loopRegion).toBeNull();
  });

  it("toggles an active A-B region off and re-activates a pending pair", async () => {
    setLoopRegionMock.mockResolvedValue(1_000);
    clearLoopRegionMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      usePlaybackTransport({
        entries,
        selectedEntryId: "a.wav",
        playbackStatus: "playing",
        onSelectEntry: vi.fn(),
      }),
    );

    await act(async () => result.current.setAbPoint("a", 1_000));
    await act(async () => result.current.setAbPoint("b", 5_000));
    await act(async () => result.current.toggleAbRepeat());

    expect(clearLoopRegionMock).toHaveBeenCalledOnce();
    expect(result.current.loopRegion).toBeNull();
  });
});
