import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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

export interface DeviceLostPayload {
  previous_device_id: string;
}

export interface BrowserEntryData {
  id: string;
  name: string;
  kind: "folder" | "playable" | "unsupported" | "inaccessible";
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
  return (
    typeof value.id === "string" &&
    typeof value.name === "string" &&
    validKind &&
    validMetadata
  );
}

// ── Event names ───────────────────────────────────────────────────────

export const EVENT_STATE_CHANGED = "playback:state-changed";
export const EVENT_POSITION = "playback:position";
export const EVENT_DEVICE_LOST = "audio:device-lost";
export const EVENT_FOLDER_CHUNK = "browser:folder-chunk";

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
