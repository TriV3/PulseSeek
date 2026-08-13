import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// ── Event envelope ────────────────────────────────────────────────────

/** Mirrors the Rust EventEnvelope. */
export interface EventEnvelope<T = unknown> {
  version: number;
  event: string;
  payload: T;
}

// ── Event payloads ────────────────────────────────────────────────────

export interface StateChangedPayload {
  state: "stopped" | "loading" | "playing" | "paused" | "failed";
}

export interface PositionPayload {
  position_ms: number;
  duration_ms: number | null;
}

export interface TrackChangedPayload {
  path: string;
  duration_ms: number | null;
}

export interface DeviceLostPayload {
  previous_device_id: string;
}

export interface BrowserEntryData {
  id: string;
  name: string;
  kind: "folder" | "playable" | "unsupported" | "inaccessible";
  has_subfolders?: boolean | null;
  metadata?: PlayableFileMetadataData | null;
}

export interface PlayableFileMetadataData {
  duration_ms: number | null;
  size_bytes: number | null;
  modified_at_ms: number | null;
  channels: number | null;
  sample_rate: number | null;
  bit_depth: number | null;
  codec: string | null;
}

export interface FolderChunkPayload {
  session_id: string;
  entries: BrowserEntryData[];
  folders_done?: boolean;
  done: boolean;
}

export interface FileChangePayload {
  path: string;
}

export interface WaveformReadyPayload {
  path: string;
}

export interface SpectrumFramePayload {
  format_version: 1;
  sequence: number;
  position_frames: number;
  sample_rate: number;
  fft_size: number;
  magnitudes: number[];
}

export interface MusicalBandPayload {
  note_number: number;
  lower_frequency_hz: number;
  center_frequency_hz: number;
  upper_frequency_hz: number;
  magnitude: number;
}

export interface MusicalSpectrumFramePayload {
  format_version: 1;
  sequence: number;
  position_frames: number;
  sample_rate: number;
  tuning_reference_hz: number;
  bands: MusicalBandPayload[];
}

export interface MoveItemResultData {
  /** Source path of the file before the move. */
  path: string;
  /** Full path after a successful move; absent when the file failed. */
  new_path?: string;
  ok: boolean;
  category?: string;
  message?: string;
  diagnostic_code?: string;
}

export interface MoveProgressPayload {
  session_id: string;
  completed: number;
  total: number;
  done: boolean;
  /** Per-file results in batch order; only populated when `done` is true. */
  results: MoveItemResultData[];
}

export function isMoveItemResultData(
  value: unknown,
): value is MoveItemResultData {
  if (!isRecord(value)) return false;
  return (
    typeof value.path === "string" &&
    (value.new_path === undefined || typeof value.new_path === "string") &&
    typeof value.ok === "boolean" &&
    (value.category === undefined || typeof value.category === "string") &&
    (value.message === undefined || typeof value.message === "string") &&
    (value.diagnostic_code === undefined ||
      typeof value.diagnostic_code === "string")
  );
}

export function isMoveProgressPayload(
  value: unknown,
): value is MoveProgressPayload {
  if (!isRecord(value)) return false;
  return (
    typeof value.session_id === "string" &&
    isOptionalSafeInteger(value.completed) &&
    isOptionalSafeInteger(value.total) &&
    typeof value.done === "boolean" &&
    Array.isArray(value.results) &&
    value.results.every(isMoveItemResultData)
  );
}

export interface CopyItemResultData {
  /** Source path of the file being copied. */
  path: string;
  /** Full path of the created copy; absent when the file failed. */
  new_path?: string;
  ok: boolean;
  category?: string;
  message?: string;
  diagnostic_code?: string;
}

export interface CopyProgressPayload {
  session_id: string;
  completed: number;
  total: number;
  done: boolean;
  /** Per-file results in batch order; only populated when `done` is true. */
  results: CopyItemResultData[];
}

export function isCopyItemResultData(
  value: unknown,
): value is CopyItemResultData {
  if (!isRecord(value)) return false;
  return (
    typeof value.path === "string" &&
    (value.new_path === undefined || typeof value.new_path === "string") &&
    typeof value.ok === "boolean" &&
    (value.category === undefined || typeof value.category === "string") &&
    (value.message === undefined || typeof value.message === "string") &&
    (value.diagnostic_code === undefined ||
      typeof value.diagnostic_code === "string")
  );
}

export function isCopyProgressPayload(
  value: unknown,
): value is CopyProgressPayload {
  if (!isRecord(value)) return false;
  return (
    typeof value.session_id === "string" &&
    isOptionalSafeInteger(value.completed) &&
    isOptionalSafeInteger(value.total) &&
    typeof value.done === "boolean" &&
    Array.isArray(value.results) &&
    value.results.every(isCopyItemResultData)
  );
}

export function isFileChangePayload(
  value: unknown,
): value is FileChangePayload {
  return isRecord(value) && typeof value.path === "string";
}

export function isWaveformReadyPayload(
  value: unknown,
): value is WaveformReadyPayload {
  return isRecord(value) && typeof value.path === "string";
}

export function isSpectrumFramePayload(
  value: unknown,
): value is SpectrumFramePayload {
  if (!isRecord(value)) return false;
  if (
    value.format_version !== 1 ||
    !isNonNegativeSafeInteger(value.sequence) ||
    !isNonNegativeSafeInteger(value.position_frames) ||
    !isPositiveSafeInteger(value.sample_rate) ||
    !isPositiveSafeInteger(value.fft_size) ||
    value.fft_size > 8_192 ||
    !isPowerOfTwo(value.fft_size) ||
    !Array.isArray(value.magnitudes) ||
    value.magnitudes.length !== value.fft_size / 2 + 1
  ) {
    return false;
  }
  return value.magnitudes.every(
    (magnitude) =>
      typeof magnitude === "number" &&
      Number.isFinite(magnitude) &&
      magnitude >= 0,
  );
}

export function isMusicalSpectrumFramePayload(
  value: unknown,
): value is MusicalSpectrumFramePayload {
  if (!isRecord(value)) return false;
  if (
    value.format_version !== 1 ||
    !isNonNegativeSafeInteger(value.sequence) ||
    !isNonNegativeSafeInteger(value.position_frames) ||
    !isPositiveSafeInteger(value.sample_rate) ||
    !isPositiveFiniteNumber(value.tuning_reference_hz) ||
    !Array.isArray(value.bands) ||
    value.bands.length === 0 ||
    value.bands.length > 256 ||
    !value.bands.every(isMusicalBandPayload)
  ) {
    return false;
  }
  return value.bands.every((band, index, bands) => {
    if (band.center_frequency_hz >= Number(value.sample_rate) / 2) return false;
    const previous = bands[index - 1];
    if (!previous) return true;
    const tolerance = Math.max(0.01, previous.upper_frequency_hz * 1e-5);
    return (
      band.note_number === previous.note_number + 1 &&
      Math.abs(band.lower_frequency_hz - previous.upper_frequency_hz) <=
        tolerance
    );
  });
}

function isMusicalBandPayload(value: unknown): value is MusicalBandPayload {
  if (!isRecord(value)) return false;
  return (
    Number.isSafeInteger(value.note_number) &&
    isPositiveFiniteNumber(value.lower_frequency_hz) &&
    isPositiveFiniteNumber(value.center_frequency_hz) &&
    isPositiveFiniteNumber(value.upper_frequency_hz) &&
    Number(value.lower_frequency_hz) < Number(value.center_frequency_hz) &&
    Number(value.center_frequency_hz) < Number(value.upper_frequency_hz) &&
    typeof value.magnitude === "number" &&
    Number.isFinite(value.magnitude) &&
    value.magnitude >= 0
  );
}

export function isFolderChunkPayload(
  value: unknown,
): value is FolderChunkPayload {
  if (!isRecord(value)) return false;
  return (
    typeof value.session_id === "string" &&
    (value.folders_done === undefined ||
      typeof value.folders_done === "boolean") &&
    typeof value.done === "boolean" &&
    Array.isArray(value.entries) &&
    value.entries.every(isBrowserEntryData)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isOptionalSafeInteger(value: unknown): boolean {
  return value === null || (Number.isSafeInteger(value) && Number(value) >= 0);
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}

function isPositiveFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function isPowerOfTwo(value: number): boolean {
  if (value < 2) return false;
  let remaining = value;
  while (remaining % 2 === 0) remaining /= 2;
  return remaining === 1;
}

function isOptionalTimestamp(value: unknown): boolean {
  return (
    value === null ||
    (Number.isSafeInteger(value) &&
      Number(value) >= 0 &&
      Number(value) <= 8.64e15)
  );
}

function isPlayableFileMetadataData(
  value: unknown,
): value is PlayableFileMetadataData {
  if (!isRecord(value)) return false;
  return (
    isOptionalSafeInteger(value.duration_ms) &&
    isOptionalSafeInteger(value.size_bytes) &&
    isOptionalTimestamp(value.modified_at_ms) &&
    isOptionalSafeInteger(value.channels) &&
    isOptionalSafeInteger(value.sample_rate) &&
    isOptionalSafeInteger(value.bit_depth) &&
    (value.codec === null || typeof value.codec === "string")
  );
}

function isBrowserEntryData(value: unknown): value is BrowserEntryData {
  if (!isRecord(value)) return false;
  const validKind =
    value.kind === "folder" ||
    value.kind === "playable" ||
    value.kind === "unsupported" ||
    value.kind === "inaccessible";
  const validMetadata =
    value.metadata === undefined ||
    value.metadata === null ||
    isPlayableFileMetadataData(value.metadata);
  const validHasSubfolders =
    value.has_subfolders === undefined ||
    value.has_subfolders === null ||
    typeof value.has_subfolders === "boolean";
  return (
    typeof value.id === "string" &&
    typeof value.name === "string" &&
    validKind &&
    validHasSubfolders &&
    validMetadata
  );
}

// ── Event names ───────────────────────────────────────────────────────

export const EVENT_STATE_CHANGED = "playback:state-changed";
export const EVENT_POSITION = "playback:position";
export const EVENT_COMPLETED = "playback:completed";
export const EVENT_TRACK_CHANGED = "playback:track-changed";
export const EVENT_DEVICE_LOST = "audio:device-lost";
export const EVENT_FOLDER_CHUNK = "browser:folder-chunk";
export const EVENT_FILE_CHANGE = "browser:file-change";
export const EVENT_WAVEFORM_READY = "waveform:ready";
export const EVENT_SPECTRUM_FRAME = "visualization:spectrum";
export const EVENT_MUSICAL_SPECTRUM_FRAME = "visualization:musical-spectrum";
export const EVENT_MOVE_PROGRESS = "browser:move-progress";
export const EVENT_COPY_PROGRESS = "browser:copy-progress";

// ── Typed event listeners ─────────────────────────────────────────────

/**
 * Listens for playback state changes.
 *
 * Returns an `unlisten` function to stop listening.
 */
export function onStateChanged(
  handler: (payload: StateChangedPayload) => void,
): Promise<UnlistenFn> {
  return listen<StateChangedPayload>(EVENT_STATE_CHANGED, (event) => {
    handler(event.payload);
  });
}

/**
 * Listens for throttled playback position updates.
 *
 * Returns an `unlisten` function to stop listening.
 */
export function onPosition(
  handler: (payload: PositionPayload) => void,
): Promise<UnlistenFn> {
  return listen<PositionPayload>(EVENT_POSITION, (event) => {
    handler(event.payload);
  });
}

/** Listens for natural end-of-track completion. */
export function onCompleted(handler: () => void): Promise<UnlistenFn> {
  return listen(EVENT_COMPLETED, () => handler());
}

export function onTrackChanged(
  handler: (payload: TrackChangedPayload) => void,
): Promise<UnlistenFn> {
  return listen<TrackChangedPayload>(EVENT_TRACK_CHANGED, (event) => {
    if (typeof event.payload?.path === "string") handler(event.payload);
  });
}

/** Listens for completion of an exact waveform cache extraction. */
export function onWaveformReady(
  handler: (payload: WaveformReadyPayload) => void,
): Promise<UnlistenFn> {
  return listen<WaveformReadyPayload>(EVENT_WAVEFORM_READY, (event) => {
    if (isWaveformReadyPayload(event.payload)) {
      handler(event.payload);
    }
  });
}

/** Listens for validated, versioned FFT frames from the native worker. */
export function onSpectrumFrame(
  handler: (payload: SpectrumFramePayload) => void,
): Promise<UnlistenFn> {
  return subscribeToSpectrumFrames(handler);
}

async function subscribeToSpectrumFrames(
  handler: (payload: SpectrumFramePayload) => void,
): Promise<UnlistenFn> {
  const unlisten = await listen<SpectrumFramePayload>(
    EVENT_SPECTRUM_FRAME,
    (event) => {
      try {
        if (isSpectrumFramePayload(event.payload)) {
          handler(event.payload);
        }
      } finally {
        void invoke("acknowledge_spectrum_frame");
      }
    },
  );
  try {
    await invoke("subscribe_spectrum_events");
  } catch (error) {
    unlisten();
    throw error;
  }
  return () => {
    unlisten();
    void invoke("unsubscribe_spectrum_events");
  };
}

/** Listens for validated pitch-band frames from the native analyzer. */
export function onMusicalSpectrumFrame(
  handler: (payload: MusicalSpectrumFramePayload) => void,
): Promise<UnlistenFn> {
  return subscribeToMusicalSpectrumFrames(handler);
}

async function subscribeToMusicalSpectrumFrames(
  handler: (payload: MusicalSpectrumFramePayload) => void,
): Promise<UnlistenFn> {
  const unlisten = await listen<MusicalSpectrumFramePayload>(
    EVENT_MUSICAL_SPECTRUM_FRAME,
    (event) => {
      try {
        if (isMusicalSpectrumFramePayload(event.payload)) {
          handler(event.payload);
        }
      } finally {
        void invoke("acknowledge_musical_spectrum_frame");
      }
    },
  );
  try {
    await invoke("subscribe_musical_spectrum_events");
  } catch (error) {
    unlisten();
    throw error;
  }
  return () => {
    unlisten();
    void invoke("unsubscribe_musical_spectrum_events");
  };
}

/**
 * Listens for audio device loss events.
 *
 * Returns an `unlisten` function to stop listening.
 */
export function onDeviceLost(
  handler: (payload: DeviceLostPayload) => void,
): Promise<UnlistenFn> {
  return listen<DeviceLostPayload>(EVENT_DEVICE_LOST, (event) => {
    handler(event.payload);
  });
}

/**
 * Listens for batched folder enumeration results.
 *
 * Returns an `unlisten` function to stop listening.
 */
export function onFolderChunk(
  handler: (payload: FolderChunkPayload) => void,
): Promise<UnlistenFn> {
  return listen<FolderChunkPayload>(EVENT_FOLDER_CHUNK, (event) => {
    if (isFolderChunkPayload(event.payload)) {
      handler(event.payload);
    }
  });
}

/**
 * Listens for per-file move progress.
 *
 * Intermediate events carry only `completed`/`total`; the final `done` event
 * carries the full per-file `results` list. Returns an `unlisten` function.
 */
export function onMoveProgress(
  handler: (payload: MoveProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<MoveProgressPayload>(EVENT_MOVE_PROGRESS, (event) => {
    if (isMoveProgressPayload(event.payload)) {
      handler(event.payload);
    }
  });
}

/**
 * Listens for per-file copy progress.
 *
 * Intermediate events carry only `completed`/`total`; the final `done` event
 * carries the full per-file `results` list. Returns an `unlisten` function.
 */
export function onCopyProgress(
  handler: (payload: CopyProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<CopyProgressPayload>(EVENT_COPY_PROGRESS, (event) => {
    if (isCopyProgressPayload(event.payload)) {
      handler(event.payload);
    }
  });
}

/**
 * Listens for filesystem changes in the watched folder.
 *
 * The frontend re-reads the folder so external edits appear without a manual
 * refresh (FR-BR-008). Returns an `unlisten` function to stop listening.
 */
export function onFileChanged(
  handler: (payload: FileChangePayload) => void,
): Promise<UnlistenFn> {
  return listen<FileChangePayload>(EVENT_FILE_CHANGE, (event) => {
    if (isFileChangePayload(event.payload)) {
      handler(event.payload);
    }
  });
}
