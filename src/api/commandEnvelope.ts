import { invoke } from "@tauri-apps/api/core";

// ── Types mirroring the Rust command envelope ──────────────────────────

/** Error returned by the envelope boundary layer. */
export interface BoundaryError {
  category: string;
  message: string;
  diagnostic_code: string;
}

/** Response envelope from the Rust backend. */
export interface CommandResponse<T = unknown> {
  version: number;
  ok: boolean;
  data?: T;
  error?: BoundaryError;
}

/** Structured error thrown when a command response is not ok. */
export class CommandError extends Error {
  readonly category: string;
  readonly diagnosticCode: string;

  constructor(boundary: BoundaryError) {
    super(boundary.message);
    this.name = "CommandError";
    this.category = boundary.category;
    this.diagnosticCode = boundary.diagnostic_code;
  }
}

// ── Health command ────────────────────────────────────────────────────

export interface HealthResponse {
  ready: boolean;
}

/** Current version of the command envelope protocol. */
export const CURRENT_COMMAND_VERSION = 1;

// ── Playback command types ────────────────────────────────────────────

export interface PlayRequest {
  path: string;
}

export type PlayResponse = Record<string, never>;
export type PauseResponse = Record<string, never>;
export type ResumeResponse = Record<string, never>;
export type StopResponse = Record<string, never>;

export interface SeekRequest {
  position_ms: number;
}

export interface SeekResponse {
  position_ms: number;
}

export interface VolumeRequest {
  gain: number;
  muted: boolean;
}

export type VolumeResponse = Record<string, never>;

export type PlaybackMode =
  "one-shot" | "loop-current" | "sequential" | "random";

export type ThemePreference = "system" | "light" | "dark";

export interface SetPlaybackModeRequest {
  mode: PlaybackMode;
}

export interface SetPlaybackModeResponse {
  mode: PlaybackMode;
}

// ── Audio device command types ─────────────────────────────────────────

export interface DeviceInfoData {
  id: string;
  name: string;
}

export interface ListDevicesResponse {
  devices: DeviceInfoData[];
}

export interface CurrentDeviceResponse {
  device: DeviceInfoData | null;
}

export interface SelectDeviceRequest {
  device_id: string;
}

// ── Player preferences ────────────────────────────────────────────────

export interface PlayerPreferences {
  schema_version: 1;
  revision: number;
  playback_mode: PlaybackMode;
  output_device_id: string | null;
  volume: number;
  muted: boolean;
  waveform_size: number;
  browser_size: number;
  selected_folder_path: string | null;
  expanded_folder_paths: string[];
  last_played_file_path: string | null;
  theme: ThemePreference;
}

interface PlayerPreferencesResponse {
  version: number;
  preferences: PlayerPreferences;
}

function isPlayerPreferences(value: unknown): value is PlayerPreferences {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return (
    candidate.schema_version === 1 &&
    typeof candidate.revision === "number" &&
    ["one-shot", "loop-current", "sequential", "random"].includes(
      String(candidate.playback_mode),
    ) &&
    (candidate.output_device_id === null ||
      typeof candidate.output_device_id === "string") &&
    typeof candidate.volume === "number" &&
    typeof candidate.muted === "boolean" &&
    typeof candidate.waveform_size === "number" &&
    typeof candidate.browser_size === "number" &&
    (candidate.selected_folder_path === null ||
      typeof candidate.selected_folder_path === "string") &&
    Array.isArray(candidate.expanded_folder_paths) &&
    candidate.expanded_folder_paths.every((path) => typeof path === "string") &&
    (candidate.last_played_file_path === null ||
      typeof candidate.last_played_file_path === "string") &&
    ["system", "light", "dark"].includes(String(candidate.theme))
  );
}

function preferencesFromResponse(
  response: PlayerPreferencesResponse,
): PlayerPreferences {
  if (!isPlayerPreferences(response?.preferences)) {
    throw new Error("Invalid player preferences response.");
  }
  return response.preferences;
}

export async function loadPlayerPreferences(): Promise<PlayerPreferences> {
  const response = await invoke<PlayerPreferencesResponse>(
    "load_player_preferences",
  );
  return preferencesFromResponse(response);
}

export async function savePlayerPreferences(
  preferences: PlayerPreferences,
): Promise<PlayerPreferences> {
  const response = await invoke<PlayerPreferencesResponse>(
    "save_player_preferences",
    { preferences },
  );
  return preferencesFromResponse(response);
}

export type SelectDeviceResponse = Record<string, never>;

// ── Typed invoke wrapper ──────────────────────────────────────────────

/**
 * Sends a versioned command to the Rust backend through the envelope system.
 *
 * Throws `CommandError` when the backend returns `ok: false`.
 */
export async function invokeCommand<T>(
  command: string,
  payload: unknown,
): Promise<T> {
  const response = await invoke<CommandResponse<T>>("invoke_command", {
    envelope: {
      version: CURRENT_COMMAND_VERSION,
      command,
      payload,
    },
  });

  if (!response.ok || response.error) {
    throw new CommandError(
      response.error ?? {
        category: "Internal",
        message: "Command failed with no error details.",
        diagnostic_code: "command.missing_error",
      },
    );
  }

  return response.data as T;
}

// ── Typed playback command wrappers ───────────────────────────────────

/**
 * Health check command. Returns `true` when the Rust backend responds.
 */
export async function healthCheck(): Promise<boolean> {
  const response = await invokeCommand<HealthResponse>("health", {});
  return response.ready;
}

/** Starts playback of the given file path. */
export async function play(path: string): Promise<void> {
  await invokeCommand<PlayResponse>("play", { path } satisfies PlayRequest);
}

/** Pauses current playback. */
export async function pause(): Promise<void> {
  await invokeCommand<PauseResponse>("pause", {});
}

/** Resumes paused playback. */
export async function resume(): Promise<void> {
  await invokeCommand<ResumeResponse>("resume", {});
}

/** Stops current playback. */
export async function stop(): Promise<void> {
  await invokeCommand<StopResponse>("stop", {});
}

/** Seeks to the given millisecond position. */
export async function seek(position_ms: number): Promise<number> {
  const response = await invokeCommand<SeekResponse>("seek", {
    position_ms,
  } satisfies SeekRequest);
  return response.position_ms;
}

/** Sets volume gain and mute state. */
export async function setVolume(gain: number, muted: boolean): Promise<void> {
  await invokeCommand<VolumeResponse>("volume", {
    gain,
    muted,
  } satisfies VolumeRequest);
}

/** Sets and confirms end-of-file playback mode. */
export async function setPlaybackMode(
  mode: PlaybackMode,
): Promise<PlaybackMode> {
  const response = await invokeCommand<SetPlaybackModeResponse>(
    "set_playback_mode",
    { mode } satisfies SetPlaybackModeRequest,
  );
  return response.mode;
}

// ── Typed audio device command wrappers ────────────────────────────────

/** Returns all available audio output devices. */
export async function listDevices(): Promise<DeviceInfoData[]> {
  const response = await invokeCommand<ListDevicesResponse>("list_devices", {});
  return response.devices;
}

/** Returns the currently selected audio output device, or null. */
export async function currentDevice(): Promise<DeviceInfoData | null> {
  const response = await invokeCommand<CurrentDeviceResponse>(
    "current_device",
    {},
  );
  return response.device;
}

/** Selects an audio output device by its stable identifier. */
export async function selectDevice(device_id: string): Promise<void> {
  await invokeCommand<SelectDeviceResponse>("select_device", {
    device_id,
  } satisfies SelectDeviceRequest);
}

// ── Folder picker types ───────────────────────────────────────────────

export interface PickFolderResponse {
  path: string | null;
}

// ── Typed folder enumeration command wrappers ─────────────────────────

export interface StartEnumerationRequest {
  path: string;
  batch_size?: number;
  show_unsupported?: boolean;
}

export interface BrowserRoot {
  path: string;
  name: string;
}

export interface ListBrowserRootsResponse {
  roots: BrowserRoot[];
}

/** Lists local disks and network volumes currently mounted by the OS. */
export async function listBrowserRoots(): Promise<BrowserRoot[]> {
  const response = await invokeCommand<ListBrowserRootsResponse>(
    "list_browser_roots",
    {},
  );
  return response.roots;
}

export interface StartEnumerationResponse {
  session_id: string;
}

export interface CancelEnumerationRequest {
  session_id: string;
}

export type CancelEnumerationResponse = Record<string, never>;

/** Starts enumerating a folder. Results arrive via onFolderChunk events. */
export async function startEnumeration(
  path: string,
  batch_size?: number,
): Promise<string> {
  const response = await invokeCommand<StartEnumerationResponse>(
    "start_enumeration",
    { path, batch_size } satisfies StartEnumerationRequest,
  );
  return response.session_id;
}

/** Cancels a running folder enumeration. */
export async function cancelEnumeration(session_id: string): Promise<void> {
  await invokeCommand<CancelEnumerationResponse>("cancel_enumeration", {
    session_id,
  } satisfies CancelEnumerationRequest);
}

/** Opens the native OS folder picker dialog.
 *
 * Uses a dedicated async Tauri command to avoid macOS dialog deadlocks.
 * Returns the selected folder path, or `null` if the user cancelled.
 * Throws `CommandError` if the dialog encounters a system-level failure.
 */
export async function pickFolder(): Promise<string | null> {
  const response = await invoke<PickFolderResponse>("pick_folder_dialog", {});
  return response.path;
}

// ── Move-to-trash types ────────────────────────────────────────────────

export interface MoveToTrashRequest {
  paths: string[];
}

export interface MoveToTrashItemResult {
  path: string;
  ok: boolean;
  category?: string;
  message?: string;
  diagnostic_code?: string;
}

export interface MoveToTrashResponse {
  results: MoveToTrashItemResult[];
}

/** Moves the given file paths to the operating system trash.
 *
 * Returns per-file results. Throws `CommandError` on invalid input.
 */
export async function moveToTrash(
  paths: string[],
): Promise<MoveToTrashItemResult[]> {
  const response = await invokeCommand<MoveToTrashResponse>("move_to_trash", {
    paths,
  } satisfies MoveToTrashRequest);
  return response.results;
}
