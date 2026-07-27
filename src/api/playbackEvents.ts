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

// ── Event names ───────────────────────────────────────────────────────

export const EVENT_STATE_CHANGED = "playback:state-changed";
export const EVENT_POSITION = "playback:position";

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
