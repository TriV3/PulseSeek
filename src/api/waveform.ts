import { invoke } from "@tauri-apps/api/core";

/** One waveform resolution level returned by the Rust backend. */
export interface WaveformLevel {
  format_version: number;
  channels: number;
  samples_per_peak: number;
  /** Lower envelope bound per peak, interleaved by bucket then channel. */
  min: number[];
  /** Upper envelope bound per peak, interleaved by bucket then channel. */
  max: number[];
}

/** Format version accepted by the renderer. */
export const WAVEFORM_FORMAT_VERSION = 1;

function isNumberArray(value: unknown): value is number[] {
  return (
    Array.isArray(value) &&
    value.every((item) => typeof item === "number" && Number.isFinite(item))
  );
}

/** Validates data crossing the Tauri boundary. */
export function isWaveformLevel(value: unknown): value is WaveformLevel {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    record.format_version === WAVEFORM_FORMAT_VERSION &&
    typeof record.channels === "number" &&
    Number.isInteger(record.channels) &&
    record.channels > 0 &&
    typeof record.samples_per_peak === "number" &&
    Number.isSafeInteger(record.samples_per_peak) &&
    record.samples_per_peak > 0 &&
    isNumberArray(record.min) &&
    isNumberArray(record.max) &&
    record.min.length === record.max.length &&
    record.min.length > 0
  );
}

/**
 * Requests a waveform level for `path` that fits roughly `targetPeaks` buckets
 * per channel. Throws when the backend response is not a valid waveform level.
 */
export async function getWaveform(
  path: string,
  targetPeaks: number,
): Promise<WaveformLevel> {
  const response = await invoke<unknown>("get_waveform", {
    path,
    targetPeaks,
  });
  if (!isWaveformLevel(response)) {
    throw new Error("Invalid waveform response.");
  }
  return response;
}
