import { describe, expect, it, vi } from "vitest";
import {
  isCopyItemResultData,
  isCopyProgressPayload,
  isFileChangePayload,
  isFolderChunkPayload,
  isMoveItemResultData,
  isMoveProgressPayload,
  isWaveformReadyPayload,
  onCopyProgress,
  onMoveProgress,
  onWaveformReady,
} from "./playbackEvents";

type EventHandler = (event: { payload: unknown }) => void;
const eventHandlers = new Map<string, EventHandler>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: EventHandler) => {
    eventHandlers.set(event, handler);
    return () => {
      eventHandlers.delete(event);
    };
  }),
}));

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

describe("file change payload validation", () => {
  it("accepts a valid file change payload", () => {
    expect(isFileChangePayload({ path: "/music" })).toBe(true);
  });

  it("rejects payloads without a path", () => {
    expect(isFileChangePayload({})).toBe(false);
    expect(isFileChangePayload(null)).toBe(false);
    expect(isFileChangePayload({ path: 42 })).toBe(false);
  });
});

describe("waveform ready events", () => {
  it("validates the selected source path", () => {
    expect(isWaveformReadyPayload({ path: "/music/track.wav" })).toBe(true);
    expect(isWaveformReadyPayload({ path: 42 })).toBe(false);
    expect(isWaveformReadyPayload(null)).toBe(false);
  });

  it("delivers only valid waveform:ready payloads", async () => {
    const handler = vi.fn();
    await onWaveformReady(handler);
    const emit = eventHandlers.get("waveform:ready");

    emit?.({ payload: { path: "/music/track.wav" } });
    emit?.({ payload: { path: 42 } });

    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ path: "/music/track.wav" });
  });
});

const validMovePayload = {
  session_id: "move-1",
  completed: 2,
  total: 2,
  done: true,
  results: [
    { path: "/music/a.wav", new_path: "/library/a.wav", ok: true },
    {
      path: "/music/b.wav",
      ok: false,
      category: "Conflict",
      message: "PulseSeek could not apply that change.",
      diagnostic_code: "file.operation",
    },
  ],
};

describe("move progress payload validation", () => {
  it("accepts a valid progress payload", () => {
    expect(isMoveProgressPayload(validMovePayload)).toBe(true);
  });

  it("accepts an in-progress payload without results", () => {
    expect(
      isMoveProgressPayload({
        session_id: "move-1",
        completed: 0,
        total: 5,
        done: false,
        results: [],
      }),
    ).toBe(true);
  });

  it("rejects non-objects and malformed fields", () => {
    expect(isMoveProgressPayload(null)).toBe(false);
    expect(isMoveProgressPayload("nope")).toBe(false);
    expect(isMoveProgressPayload({ ...validMovePayload, session_id: 7 })).toBe(
      false,
    );
    expect(isMoveProgressPayload({ ...validMovePayload, done: "yes" })).toBe(
      false,
    );
    expect(
      isMoveProgressPayload({ ...validMovePayload, results: [{ path: 1 }] }),
    ).toBe(false);
  });

  it("validates item results individually", () => {
    expect(isMoveItemResultData(validMovePayload.results[0])).toBe(true);
    expect(isMoveItemResultData(validMovePayload.results[1])).toBe(true);
    expect(isMoveItemResultData({ path: "/a.wav", ok: true })).toBe(true);
    expect(isMoveItemResultData({ path: 1, ok: true })).toBe(false);
    expect(isMoveItemResultData({ path: "/a.wav" })).toBe(false);
  });
});

describe("onMoveProgress", () => {
  it("registers a listener for browser:move-progress", async () => {
    const handler = vi.fn();
    await onMoveProgress(handler);
    expect(eventHandlers.has("browser:move-progress")).toBe(true);
  });

  it("invokes the handler with valid payloads only", async () => {
    const handler = vi.fn();
    await onMoveProgress(handler);
    const emit = eventHandlers.get("browser:move-progress");
    expect(emit).toBeDefined();

    emit?.({ payload: validMovePayload });
    emit?.({ payload: { session_id: 1 } });
    emit?.({ payload: null });

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(validMovePayload);
  });
});

const validCopyPayload = {
  session_id: "copy-1",
  completed: 2,
  total: 2,
  done: true,
  results: [
    { path: "/music/a.wav", new_path: "/library/a.wav", ok: true },
    {
      path: "/music/b.wav",
      ok: false,
      category: "Conflict",
      message: "PulseSeek could not apply that change.",
      diagnostic_code: "file.operation",
    },
  ],
};

describe("copy progress payload validation", () => {
  it("accepts a valid progress payload", () => {
    expect(isCopyProgressPayload(validCopyPayload)).toBe(true);
  });

  it("accepts an in-progress payload without results", () => {
    expect(
      isCopyProgressPayload({
        session_id: "copy-1",
        completed: 0,
        total: 5,
        done: false,
        results: [],
      }),
    ).toBe(true);
  });

  it("rejects non-objects and malformed fields", () => {
    expect(isCopyProgressPayload(null)).toBe(false);
    expect(isCopyProgressPayload("nope")).toBe(false);
    expect(isCopyProgressPayload({ ...validCopyPayload, session_id: 7 })).toBe(
      false,
    );
    expect(isCopyProgressPayload({ ...validCopyPayload, done: "yes" })).toBe(
      false,
    );
    expect(
      isCopyProgressPayload({ ...validCopyPayload, results: [{ path: 1 }] }),
    ).toBe(false);
  });

  it("validates item results individually", () => {
    expect(isCopyItemResultData(validCopyPayload.results[0])).toBe(true);
    expect(isCopyItemResultData(validCopyPayload.results[1])).toBe(true);
    expect(isCopyItemResultData({ path: "/a.wav", ok: true })).toBe(true);
    expect(isCopyItemResultData({ path: 1, ok: true })).toBe(false);
    expect(isCopyItemResultData({ path: "/a.wav" })).toBe(false);
  });
});

describe("onCopyProgress", () => {
  it("registers a listener for browser:copy-progress", async () => {
    const handler = vi.fn();
    await onCopyProgress(handler);
    expect(eventHandlers.has("browser:copy-progress")).toBe(true);
  });

  it("invokes the handler with valid payloads only", async () => {
    const handler = vi.fn();
    await onCopyProgress(handler);
    const emit = eventHandlers.get("browser:copy-progress");
    expect(emit).toBeDefined();

    emit?.({ payload: validCopyPayload });
    emit?.({ payload: { session_id: 1 } });
    emit?.({ payload: null });

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(validCopyPayload);
  });
});
