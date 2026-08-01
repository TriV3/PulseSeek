import { describe, expect, it, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getWaveform,
  isWaveformLevel,
  WAVEFORM_FORMAT_VERSION,
  type WaveformLevel,
} from "./waveform";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const VALID = {
  format_version: WAVEFORM_FORMAT_VERSION,
  channels: 2,
  samples_per_peak: 4,
  min: [-0.5, -0.25, -0.1, 0.1],
  max: [0.5, 0.75, 0.9, 0.95],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockReset();
});

describe("isWaveformLevel", () => {
  it("accepts a valid waveform level", () => {
    expect(isWaveformLevel(VALID)).toBe(true);
  });

  it("rejects non-objects and null", () => {
    expect(isWaveformLevel(null)).toBe(false);
    expect(isWaveformLevel("wave")).toBe(false);
    expect(isWaveformLevel(undefined)).toBe(false);
  });

  it("rejects an unknown format version", () => {
    expect(isWaveformLevel({ ...VALID, format_version: 2 })).toBe(false);
  });

  it("rejects zero or fractional channels", () => {
    expect(isWaveformLevel({ ...VALID, channels: 0 })).toBe(false);
    expect(isWaveformLevel({ ...VALID, channels: 1.5 })).toBe(false);
  });

  it("rejects non-positive samples per peak", () => {
    expect(isWaveformLevel({ ...VALID, samples_per_peak: 0 })).toBe(false);
  });

  it("rejects non-finite or mismatched peak arrays", () => {
    expect(isWaveformLevel({ ...VALID, min: [NaN] })).toBe(false);
    expect(isWaveformLevel({ ...VALID, max: [1] })).toBe(false);
    expect(isWaveformLevel({ ...VALID, min: [], max: [] })).toBe(false);
  });

  it("rejects missing fields", () => {
    const withoutMin: Partial<WaveformLevel> = { ...VALID };
    delete withoutMin.min;
    expect(isWaveformLevel(withoutMin)).toBe(false);
  });
});

describe("getWaveform", () => {
  it("requests the waveform with the given target and returns it", async () => {
    vi.mocked(invoke).mockResolvedValue(VALID);

    const level = await getWaveform("/music/track.wav", 64);

    expect(invoke).toHaveBeenCalledWith("get_waveform", {
      path: "/music/track.wav",
      targetPeaks: 64,
    });
    expect(level).toEqual(VALID);
  });

  it("throws on an invalid response", async () => {
    vi.mocked(invoke).mockResolvedValue({ format_version: 99 });
    await expect(getWaveform("/music/track.wav", 64)).rejects.toThrow(
      "Invalid waveform response.",
    );
  });

  it("propagates backend command failures", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("backend failed"));
    await expect(getWaveform("/music/track.wav", 64)).rejects.toThrow(
      "backend failed",
    );
  });
});
