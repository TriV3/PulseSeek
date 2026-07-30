import { renderHook, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";
import { usePlaybackTransport } from "./usePlaybackTransport";

const pauseMock = vi.hoisted(() => vi.fn());
const resumeMock = vi.hoisted(() => vi.fn());
const stopMock = vi.hoisted(() => vi.fn());
const seekMock = vi.hoisted(() => vi.fn());
const setVolumeMock = vi.hoisted(() => vi.fn());
const onStateChangedMock = vi.hoisted(() => vi.fn());
const onPositionMock = vi.hoisted(() => vi.fn());

vi.mock("../api/commandEnvelope", () => ({
  pause: pauseMock,
  resume: resumeMock,
  stop: stopMock,
  seek: seekMock,
  setVolume: setVolumeMock,
}));
vi.mock("../api/playbackEvents", () => ({
  onStateChanged: onStateChangedMock,
  onPosition: onPositionMock,
}));

const entries: BrowserEntry[] = [
  { id: "a.wav", name: "a.wav", kind: "playable" },
  { id: "b.wav", name: "b.wav", kind: "playable" },
];

beforeEach(() => {
  vi.resetAllMocks();
  onStateChangedMock.mockResolvedValue(() => undefined);
  onPositionMock.mockResolvedValue(() => undefined);
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
});
