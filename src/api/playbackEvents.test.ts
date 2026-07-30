import { describe, expect, it } from "vitest";
import { isFolderChunkPayload } from "./playbackEvents";

describe("folder chunk payload validation", () => {
  it("accepts partial playable metadata", () => {
    expect(
      isFolderChunkPayload({
        session_id: "session-1",
        entries: [
          {
            id: "/music/song.mp3",
            name: "song.mp3",
            kind: "playable",
            metadata: {
              duration_ms: 61_000,
              size_bytes: null,
              modified_at_ms: null,
              channels: 2,
              sample_rate: 44_100,
              bit_depth: null,
              codec: "MP3",
            },
          },
        ],
        done: false,
      }),
    ).toBe(true);
  });

  it("rejects unsafe or invalid metadata numbers", () => {
    expect(
      isFolderChunkPayload({
        session_id: "session-1",
        entries: [
          {
            id: "song.wav",
            name: "song.wav",
            kind: "playable",
            metadata: {
              duration_ms: Number.POSITIVE_INFINITY,
              size_bytes: -1,
              modified_at_ms: Number.MAX_SAFE_INTEGER + 1,
              channels: 2,
              sample_rate: 44_100,
              bit_depth: 16,
              codec: "PCM",
            },
          },
        ],
        done: true,
      }),
    ).toBe(false);
  });

  it("rejects unknown entry kinds", () => {
    expect(
      isFolderChunkPayload({
        session_id: "session-1",
        entries: [{ id: "x", name: "x", kind: "other" }],
        done: true,
      }),
    ).toBe(false);
  });

  it("rejects timestamps outside JavaScript Date range", () => {
    expect(
      isFolderChunkPayload({
        session_id: "session-1",
        entries: [
          {
            id: "song.wav",
            name: "song.wav",
            kind: "playable",
            metadata: {
              duration_ms: null,
              size_bytes: null,
              modified_at_ms: 8.64e15 + 1,
              channels: null,
              sample_rate: null,
              bit_depth: null,
              codec: null,
            },
          },
        ],
        done: true,
      }),
    ).toBe(false);
  });
});
