import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  isCopyItemResultData,
  isCopyProgressPayload,
  isFileChangePayload,
  isFolderChunkPayload,
  isMoveItemResultData,
  isMoveProgressPayload,
  isMusicalSpectrumFramePayload,
  isSpectrumFramePayload,
  isWaveformReadyPayload,
  onCopyProgress,
  onMoveProgress,
  onMusicalSpectrumFrame,
  onSpectrumFrame,
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

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
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

const validSpectrumPayload = {
  format_version: 1,
  sequence: 7,
  position_frames: 2_048,
  sample_rate: 48_000,
  fft_size: 8,
  magnitudes: [0, 0.1, 0.4, 0.2, 0],
};

describe("spectrum frame events", () => {
  it("accepts only finite versioned FFT payloads with the expected bin count", () => {
    expect(isSpectrumFramePayload(validSpectrumPayload)).toBe(true);
    expect(
      isSpectrumFramePayload({ ...validSpectrumPayload, format_version: 2 }),
    ).toBe(false);
    expect(
      isSpectrumFramePayload({ ...validSpectrumPayload, fft_size: 6 }),
    ).toBe(false);
    expect(
      isSpectrumFramePayload({ ...validSpectrumPayload, magnitudes: [0, 1] }),
    ).toBe(false);
    expect(
      isSpectrumFramePayload({
        ...validSpectrumPayload,
        magnitudes: [0, Number.NaN, 0, 0, 0],
      }),
    ).toBe(false);
  });

  it("accepts the higher-resolution FFT used for low frequencies", () => {
    expect(
      isSpectrumFramePayload({
        ...validSpectrumPayload,
        fft_size: 4_096,
        magnitudes: Array.from({ length: 2_049 }, () => 0),
      }),
    ).toBe(true);
  });

  it("delivers only valid visualization:spectrum payloads", async () => {
    const handler = vi.fn();
    await onSpectrumFrame(handler);
    const emit = eventHandlers.get("visualization:spectrum");

    emit?.({ payload: validSpectrumPayload });
    emit?.({ payload: { ...validSpectrumPayload, sample_rate: 0 } });

    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(validSpectrumPayload);
    expect(invoke).toHaveBeenCalledWith("subscribe_spectrum_events");
    expect(invoke).toHaveBeenCalledWith("acknowledge_spectrum_frame");
  });

  it("unsubscribes the native stream when its listener is removed", async () => {
    const unlisten = await onSpectrumFrame(vi.fn());

    unlisten();

    expect(invoke).toHaveBeenCalledWith("unsubscribe_spectrum_events");
  });
});

const validMusicalSpectrumPayload = {
  format_version: 1,
  sequence: 8,
  position_frames: 4_096,
  sample_rate: 48_000,
  tuning_reference_hz: 440,
  bands: [
    {
      note_number: 68,
      lower_frequency_hz: 403.48,
      center_frequency_hz: 415.3,
      upper_frequency_hz: 427.47,
      magnitude: 0.2,
    },
    {
      note_number: 69,
      lower_frequency_hz: 427.47,
      center_frequency_hz: 440,
      upper_frequency_hz: 452.89,
      magnitude: 0.8,
    },
  ],
};

describe("musical spectrum frame events", () => {
  it("accepts only finite, ordered, versioned musical bands", () => {
    expect(isMusicalSpectrumFramePayload(validMusicalSpectrumPayload)).toBe(
      true,
    );
    expect(
      isMusicalSpectrumFramePayload({
        ...validMusicalSpectrumPayload,
        format_version: 2,
      }),
    ).toBe(false);
    expect(
      isMusicalSpectrumFramePayload({
        ...validMusicalSpectrumPayload,
        tuning_reference_hz: Number.NaN,
      }),
    ).toBe(false);
    expect(
      isMusicalSpectrumFramePayload({
        ...validMusicalSpectrumPayload,
        bands: validMusicalSpectrumPayload.bands.map((band) => ({
          ...band,
          magnitude: -1,
        })),
      }),
    ).toBe(false);
    expect(
      isMusicalSpectrumFramePayload({
        ...validMusicalSpectrumPayload,
        bands: [...validMusicalSpectrumPayload.bands].reverse(),
      }),
    ).toBe(false);
  });

  it("delivers and acknowledges only valid musical spectrum payloads", async () => {
    const handler = vi.fn();
    await onMusicalSpectrumFrame(handler);
    const emit = eventHandlers.get("visualization:musical-spectrum");

    emit?.({ payload: validMusicalSpectrumPayload });
    emit?.({
      payload: { ...validMusicalSpectrumPayload, tuning_reference_hz: 0 },
    });

    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(validMusicalSpectrumPayload);
    expect(invoke).toHaveBeenCalledWith("subscribe_musical_spectrum_events");
    expect(invoke).toHaveBeenCalledWith("acknowledge_musical_spectrum_frame");
  });

  it("unsubscribes the native musical stream with its listener", async () => {
    const unlisten = await onMusicalSpectrumFrame(vi.fn());

    unlisten();

    expect(invoke).toHaveBeenCalledWith("unsubscribe_musical_spectrum_events");
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
