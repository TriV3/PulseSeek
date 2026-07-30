import { renderHook, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { usePlaybackSelection } from "./usePlaybackSelection";

const playMock = vi.hoisted(() => vi.fn());
vi.mock("../api/commandEnvelope", () => ({ play: playMock }));

beforeEach(() => vi.resetAllMocks());

describe("usePlaybackSelection", () => {
  it("starts playback and marks the current entry playing", async () => {
    playMock.mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => usePlaybackSelection());

    await act(async () => {
      await result.current.select({
        id: "/music/a.wav",
        name: "a.wav",
        kind: "playable",
      });
    });

    expect(playMock).toHaveBeenCalledWith("/music/a.wav");
    expect(result.current.playback).toMatchObject({
      entryId: "/music/a.wav",
      status: "playing",
      error: null,
    });
  });

  it("ignores a stale play failure after a newer selection succeeds", async () => {
    let rejectFirst: (error: Error) => void = () => undefined;
    playMock
      .mockReturnValueOnce(
        new Promise<void>((_, reject) => {
          rejectFirst = reject;
        }),
      )
      .mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => usePlaybackSelection());

    await act(async () => {
      void result.current.select({
        id: "/music/a.wav",
        name: "a.wav",
        kind: "playable",
      });
      await result.current.select({
        id: "/music/b.wav",
        name: "b.wav",
        kind: "playable",
      });
    });
    await act(async () => rejectFirst(new Error("old failure")));

    expect(result.current.playback).toMatchObject({
      entryId: "/music/b.wav",
      status: "playing",
      error: null,
    });
  });

  it("reports current command errors", async () => {
    playMock.mockRejectedValueOnce(new Error("decoder unavailable"));
    const { result } = renderHook(() => usePlaybackSelection());

    await act(async () => {
      await result.current.select({
        id: "/music/bad.wav",
        name: "bad.wav",
        kind: "playable",
      });
    });

    expect(result.current.playback).toMatchObject({
      entryId: "/music/bad.wav",
      status: "failed",
      error: "decoder unavailable",
    });
  });
});
