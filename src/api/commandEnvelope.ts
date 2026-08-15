import { invoke } from "@tauri-apps/api/core";
import type { SeekStepMode } from "../shortcuts/seekStep";
import {
  getShortcutPlatform,
  SHORTCUT_ACTIONS,
  validateShortcutBindings,
  type ShortcutActionId,
  type ShortcutBindings,
  type ShortcutChord,
} from "../shortcuts/keyboardShortcuts";

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

export type ThemePreference =
  "system" | "light" | "dark" | "midnight" | "high-contrast";

export type WaveformStyle = "solid" | "gradient" | "outline";

export interface SetPlaybackModeRequest {
  mode: PlaybackMode;
}

export interface SetPlaybackModeResponse {
  mode: PlaybackMode;
}

export interface SetLoopRegionRequest {
  start_ms: number;
  end_ms: number;
}

export interface SetLoopRegionResponse {
  start_ms: number;
}

export type ClearLoopRegionRequest = Record<string, never>;
export type ClearLoopRegionResponse = Record<string, never>;

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
  last_played_position_ms: number;
  last_played_duration_ms: number | null;
  theme: ThemePreference;
  waveform_style: WaveformStyle;
  seek_step_mode: SeekStepMode;
  show_hidden_folders: boolean;
  gapless_playback: boolean;
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
    typeof candidate.last_played_position_ms === "number" &&
    Number.isSafeInteger(candidate.last_played_position_ms) &&
    candidate.last_played_position_ms >= 0 &&
    (candidate.last_played_duration_ms === null ||
      (typeof candidate.last_played_duration_ms === "number" &&
        Number.isSafeInteger(candidate.last_played_duration_ms) &&
        candidate.last_played_duration_ms >= 0)) &&
    ["system", "light", "dark", "midnight", "high-contrast"].includes(
      String(candidate.theme),
    ) &&
    ["solid", "gradient", "outline"].includes(
      String(candidate.waveform_style),
    ) &&
    (candidate.seek_step_mode === "auto" ||
      ["1s", "2s", "5s", "10s", "15s", "20s", "30s"].includes(
        String(candidate.seek_step_mode),
      )) &&
    typeof candidate.show_hidden_folders === "boolean" &&
    (candidate.gapless_playback === undefined ||
      typeof candidate.gapless_playback === "boolean")
  );
}

function preferencesFromResponse(
  response: PlayerPreferencesResponse,
): PlayerPreferences {
  if (!isPlayerPreferences(response?.preferences)) {
    throw new Error("Invalid player preferences response.");
  }
  return {
    ...response.preferences,
    gapless_playback: response.preferences.gapless_playback ?? true,
  };
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

// ── Visualization settings ──────────────────────────────────────────

export type VisualizationMode =
  "waveform" | "logarithmic" | "linear" | "musical";
export type VisualizationQuality = "low" | "balanced" | "high";

export interface VisualizationSettings {
  enabled: boolean;
  mode: VisualizationMode;
  quality: VisualizationQuality;
}

interface VisualizationSettingsResponse {
  version: number;
  settings: VisualizationSettings;
}

function isVisualizationSettings(
  value: unknown,
): value is VisualizationSettings {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.enabled === "boolean" &&
    ["waveform", "logarithmic", "linear", "musical"].includes(
      String(candidate.mode),
    ) &&
    ["low", "balanced", "high"].includes(String(candidate.quality))
  );
}

function visualizationSettingsFromResponse(
  response: VisualizationSettingsResponse,
): VisualizationSettings {
  if (!isVisualizationSettings(response?.settings)) {
    throw new Error("Invalid visualization settings response.");
  }
  return response.settings;
}

export async function loadVisualizationSettings(
  reducedMotion: boolean,
): Promise<VisualizationSettings> {
  const response = await invoke<VisualizationSettingsResponse>(
    "load_visualization_settings",
    { reducedMotion },
  );
  return visualizationSettingsFromResponse(response);
}

export async function saveVisualizationSettings(
  settings: VisualizationSettings,
  reducedMotion: boolean,
): Promise<VisualizationSettings> {
  const response = await invoke<VisualizationSettingsResponse>(
    "save_visualization_settings",
    { settings, reducedMotion },
  );
  return visualizationSettingsFromResponse(response);
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

/** Prepares next file for gapless sequential playback. */
export async function prepareNext(path: string): Promise<void> {
  await invokeCommand<Record<string, never>>("prepare_next", { path });
}

export async function clearPrepared(): Promise<void> {
  await invokeCommand<Record<string, never>>("clear_prepared", {});
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

/** Activates an A–B repeat region and returns the confirmed start. */
export async function setLoopRegion(
  start_ms: number,
  end_ms: number,
): Promise<number> {
  const response = await invokeCommand<SetLoopRegionResponse>(
    "set_loop_region",
    { start_ms, end_ms } satisfies SetLoopRegionRequest,
  );
  return response.start_ms;
}

/** Deactivates the active A–B repeat region. */
export async function clearLoopRegion(): Promise<void> {
  await invokeCommand<ClearLoopRegionResponse>("clear_loop_region", {});
}

// ── Shortcut mappings ─────────────────────────────────────────────────

export interface ShortcutMappingData extends ShortcutChord {
  action_id: ShortcutActionId;
}

interface ShortcutMappingsResponse {
  mappings: ShortcutMappingData[];
  unavailable_action_ids: ShortcutActionId[];
}

const SHORTCUT_ACTION_IDS = new Set<string>(
  SHORTCUT_ACTIONS.map((action) => action.id),
);
const SHORTCUT_ACTIONS_BY_ID = new Map(
  SHORTCUT_ACTIONS.map((action) => [action.id, action]),
);

function shortcutBindingsFromResponse(value: unknown): ShortcutBindings {
  if (!value || typeof value !== "object") return invalidShortcutResponse();
  const candidate = value as Record<string, unknown>;
  if (
    !Array.isArray(candidate.mappings) ||
    !Array.isArray(candidate.unavailable_action_ids)
  ) {
    return invalidShortcutResponse();
  }

  const seen = new Set<string>();
  const chords = new Set<string>();
  const bindings = {} as ShortcutBindings;
  for (const value of candidate.mappings) {
    if (!value || typeof value !== "object") return invalidShortcutResponse();
    const mapping = value as Record<string, unknown>;
    if (
      typeof mapping.action_id !== "string" ||
      !SHORTCUT_ACTION_IDS.has(mapping.action_id) ||
      !SHORTCUT_ACTIONS_BY_ID.get(mapping.action_id as ShortcutActionId)
        ?.available ||
      seen.has(mapping.action_id) ||
      typeof mapping.key !== "string" ||
      mapping.key.trim() === "" ||
      typeof mapping.primary !== "boolean" ||
      typeof mapping.shift !== "boolean" ||
      typeof mapping.alt !== "boolean"
    ) {
      return invalidShortcutResponse();
    }
    const signature = `${mapping.primary}:${mapping.shift}:${mapping.alt}:${mapping.key.trim().toLocaleLowerCase()}`;
    if (chords.has(signature)) return invalidShortcutResponse();
    chords.add(signature);
    seen.add(mapping.action_id);
    bindings[mapping.action_id as ShortcutActionId] = {
      key: mapping.key,
      primary: mapping.primary,
      shift: mapping.shift,
      alt: mapping.alt,
    };
  }

  for (const actionId of candidate.unavailable_action_ids) {
    // Every shipped action is now available (PR-089), so any id in the
    // unavailable list makes the response inconsistent and must be rejected;
    // the seen-size guard below catches the extra entry.
    if (
      typeof actionId !== "string" ||
      !SHORTCUT_ACTION_IDS.has(actionId) ||
      seen.has(actionId)
    ) {
      return invalidShortcutResponse();
    }
    seen.add(actionId);
    bindings[actionId as ShortcutActionId] = null;
  }
  if (seen.size !== SHORTCUT_ACTIONS.length) return invalidShortcutResponse();
  if (
    Object.keys(validateShortcutBindings(bindings, getShortcutPlatform()))
      .length > 0
  ) {
    return invalidShortcutResponse();
  }
  return bindings;
}

function invalidShortcutResponse(): never {
  throw new Error("Invalid shortcut mappings response.");
}

function shortcutMappingsFromBindings(
  bindings: ShortcutBindings,
): ShortcutMappingData[] {
  return SHORTCUT_ACTIONS.flatMap((action) => {
    const binding = bindings[action.id];
    return binding ? [{ action_id: action.id, ...binding }] : [];
  });
}

export async function loadShortcuts(): Promise<ShortcutBindings> {
  const response = await invokeCommand<ShortcutMappingsResponse>(
    "load_shortcuts",
    {},
  );
  return shortcutBindingsFromResponse(response);
}

export async function saveShortcuts(
  bindings: ShortcutBindings,
): Promise<ShortcutBindings> {
  const response = await invokeCommand<ShortcutMappingsResponse>(
    "save_shortcuts",
    { mappings: shortcutMappingsFromBindings(bindings) },
  );
  return shortcutBindingsFromResponse(response);
}

export async function resetShortcuts(): Promise<ShortcutBindings> {
  const response = await invokeCommand<ShortcutMappingsResponse>(
    "reset_shortcuts",
    {},
  );
  return shortcutBindingsFromResponse(response);
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
  recursive?: boolean;
  show_hidden?: boolean;
}

export type BrowserRootKind = "system" | "home" | "physical" | "network";

export interface BrowserRoot {
  path: string;
  name: string;
  kind: BrowserRootKind;
}

export type BrowserLibraryKind =
  "documents" | "music" | "pictures" | "videos" | "downloads";

export interface BrowserLibrary {
  path: string;
  name: string;
  kind: BrowserLibraryKind;
}

export interface BrowserLocations {
  roots: BrowserRoot[];
  libraries: BrowserLibrary[];
}

export interface ListBrowserRootsResponse {
  roots: BrowserRoot[];
  libraries: BrowserLibrary[];
}

function isBrowserLibrary(value: unknown): value is BrowserLibrary {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.path === "string" &&
    typeof candidate.name === "string" &&
    ["documents", "music", "pictures", "videos", "downloads"].includes(
      String(candidate.kind),
    )
  );
}

function isBrowserRoot(value: unknown): value is BrowserRoot {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.path === "string" &&
    candidate.path.length > 0 &&
    typeof candidate.name === "string" &&
    candidate.name.length > 0 &&
    (candidate.kind === "system" ||
      candidate.kind === "home" ||
      candidate.kind === "physical" ||
      candidate.kind === "network")
  );
}

/** Lists local disks and network volumes currently mounted by the OS. */
export async function listBrowserRoots(): Promise<BrowserLocations> {
  const response = await invokeCommand<ListBrowserRootsResponse>(
    "list_browser_roots",
    {},
  );
  if (
    !Array.isArray(response?.roots) ||
    !response.roots.every(isBrowserRoot) ||
    !Array.isArray(response?.libraries) ||
    !response.libraries.every(isBrowserLibrary)
  ) {
    throw new Error("Invalid browser roots response.");
  }
  return response;
}

export interface StartEnumerationResponse {
  session_id: string;
}

export interface CancelEnumerationRequest {
  session_id: string;
}

export type CancelEnumerationResponse = Record<string, never>;

/** Starts enumerating a folder. Results arrive via onFolderChunk events.
 *
 * When `recursive` is true, the backend walks the whole subtree below `path`
 * with cycle protection and streams every playable file it finds.
 */
export async function startEnumeration(
  path: string,
  batch_size?: number,
  recursive = false,
  show_hidden = false,
): Promise<string> {
  const response = await invokeCommand<StartEnumerationResponse>(
    "start_enumeration",
    {
      path,
      batch_size,
      recursive,
      show_hidden,
    } satisfies StartEnumerationRequest,
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

// ── Rename-file types ──────────────────────────────────────────────────

export interface RenameFileRequest {
  path: string;
  new_name: string;
}

export interface RenameFileResponse {
  old_path: string;
  new_path: string;
  /** True when the renamed file is the currently playing file (FR-FM-009). */
  was_playing: boolean;
}

/** Renames `path` to `new_name` within the same directory (FR-FM-004).
 *
 * Returns the old and new paths and whether the renamed file was playing.
 * Throws `CommandError` on invalid names, collisions, or filesystem errors.
 */
export async function renameFile(
  path: string,
  newName: string,
): Promise<RenameFileResponse> {
  const response = await invokeCommand<RenameFileResponse>("rename_file", {
    path,
    new_name: newName,
  } satisfies RenameFileRequest);
  return response;
}

// ── Move files types ───────────────────────────────────────────────────

export interface StartMoveFilesRequest {
  paths: string[];
  target_dir: string;
}

export interface StartMoveFilesResponse {
  session_id: string;
}

export interface CancelMoveFilesRequest {
  session_id: string;
}

export type CancelMoveFilesResponse = Record<string, never>;

/** Starts moving `paths` into `target_dir` (FR-FM-004, FR-FM-005).
 *
 * Returns a session id. Per-file progress arrives through the
 * `browser:move-progress` event; use `cancelMoveFiles` to stop the batch.
 * Throws `CommandError` on invalid targets or selection.
 */
export async function startMoveFiles(
  paths: string[],
  targetDir: string,
): Promise<string> {
  const response = await invokeCommand<StartMoveFilesResponse>(
    "start_move_files",
    {
      paths,
      target_dir: targetDir,
    } satisfies StartMoveFilesRequest,
  );
  return response.session_id;
}

/** Requests cancellation of a running move batch (no-op when finished). */
export async function cancelMoveFiles(sessionId: string): Promise<void> {
  await invokeCommand<CancelMoveFilesResponse>("cancel_move_files", {
    session_id: sessionId,
  } satisfies CancelMoveFilesRequest);
}

// ── Copy files types ───────────────────────────────────────────────────

export interface StartCopyFilesRequest {
  paths: string[];
  target_dir: string;
}

export interface StartCopyFilesResponse {
  session_id: string;
}

export interface CancelCopyFilesRequest {
  session_id: string;
}

export type CancelCopyFilesResponse = Record<string, never>;

/** Starts copying `paths` into `target_dir` (FR-FM-004, FR-FM-005).
 *
 * Returns a session id. Per-file progress arrives through the
 * `browser:copy-progress` event; use `cancelCopyFiles` to stop the batch.
 * Originals are never modified. Throws `CommandError` on invalid targets or
 * selection.
 */
export async function startCopyFiles(
  paths: string[],
  targetDir: string,
): Promise<string> {
  const response = await invokeCommand<StartCopyFilesResponse>(
    "start_copy_files",
    {
      paths,
      target_dir: targetDir,
    } satisfies StartCopyFilesRequest,
  );
  return response.session_id;
}

/** Requests cancellation of a running copy batch (no-op when finished). */
export async function cancelCopyFiles(sessionId: string): Promise<void> {
  await invokeCommand<CancelCopyFilesResponse>("cancel_copy_files", {
    session_id: sessionId,
  } satisfies CancelCopyFilesRequest);
}

// ── External actions types ─────────────────────────────────────────────

export interface RevealFileRequest {
  path: string;
}

export type RevealFileResponse = Record<string, never>;

export interface OpenWithRequest {
  path: string;
}

export type OpenWithResponse = Record<string, never>;

/** Reveals `path` in the operating system file manager (FR-FM-006).
 *
 * Throws `CommandError` when the file is missing or the platform cannot
 * reveal it. The backend exposes no general process-launch capability.
 */
export async function revealFile(path: string): Promise<void> {
  await invokeCommand<RevealFileResponse>("reveal_file", {
    path,
  } satisfies RevealFileRequest);
}

/** Opens `path` with the operating system default application (FR-FM-007).
 *
 * Throws `CommandError` when the file is missing, unreadable, or no default
 * application exists. The backend exposes no general process-launch
 * capability.
 */
export async function openWith(path: string): Promise<void> {
  await invokeCommand<OpenWithResponse>("open_with", {
    path,
  } satisfies OpenWithRequest);
}

// ── Drag-out types ─────────────────────────────────────────────────────

export interface DragOutRequest {
  paths: string[];
}

export type DragOutResponse = Record<string, never>;

/** Starts a drag session for `paths` into compatible applications
 * (FR-FM-011).
 *
 * Throws `CommandError` when any target is missing or the platform has no
 * drag-out adapter. The backend exposes no general process-launch or
 * clipboard capability.
 */
export async function dragOut(paths: string[]): Promise<void> {
  await invokeCommand<DragOutResponse>("drag_out", {
    paths,
  } satisfies DragOutRequest);
}

// ── Drag-in probe types ────────────────────────────────────────────────

/** Classification of a dropped filesystem path (FR-DI-001). */
export type ProbePathKind =
  "directory" | "playable" | "unsupported" | "missing";

export interface ProbePathRequest {
  path: string;
}

export interface ProbePathResponse {
  kind: ProbePathKind;
}

/** Classifies a dropped path so the UI can decide between revealing its
 * folder and playing it (FR-DI-001).
 *
 * The backend is the source of truth for filesystem and format checks: the
 * frontend never inspects paths itself. Throws `CommandError` only when the
 * path cannot be inspected (for example a permission denial); a missing or
 * unsupported target is a normal `kind`, not an error.
 */
export async function probePath(path: string): Promise<ProbePathKind> {
  const response = await invokeCommand<ProbePathResponse>("probe_path", {
    path,
  } satisfies ProbePathRequest);
  return response.kind;
}

// ── Recent folders types ───────────────────────────────────────────────

export interface RecentFolderData {
  path: string;
  name: string;
  last_opened_ms: number;
}

export interface ListRecentFoldersResponse {
  folders: RecentFolderData[];
}

export interface RecordRecentFolderRequest {
  path: string;
}

export type RecordRecentFolderResponse = Record<string, never>;
export type ClearRecentFoldersResponse = Record<string, never>;

/** Returns the recent-folder history from most to least recent (FR-BR-011). */
export async function listRecentFolders(): Promise<RecentFolderData[]> {
  const response = await invokeCommand<ListRecentFoldersResponse>(
    "list_recent_folders",
    {},
  );
  return response.folders;
}

/** Records `path` as the most recently opened folder (best-effort history).
 *
 * Missing or non-directory paths are rejected by the backend with a safe
 * error; callers treat recording as non-critical.
 */
export async function recordRecentFolder(path: string): Promise<void> {
  await invokeCommand<RecordRecentFolderResponse>("record_recent_folder", {
    path,
  } satisfies RecordRecentFolderRequest);
}

/** Removes the entire recent-folder history. */
export async function clearRecentFolders(): Promise<void> {
  await invokeCommand<ClearRecentFoldersResponse>("clear_recent_folders", {});
}

/** Removes recalculable waveform data without touching user data. */
export async function clearWaveformCache(): Promise<void> {
  await invoke("clear_waveform_cache");
}

// ── Opened files types ────────────────────────────────────────────────

/** Returns and clears the audio files the OS asked PulseSeek to open.
 *
 * On macOS this drains the cold-start queue (files passed as launch
 * arguments or delivered by `RunEvent::Opened` before the frontend
 * subscribed). Warm opens arrive through the `browser:opened-files` event.
 */
export async function openedAudioFiles(): Promise<string[]> {
  const response = await invoke<unknown>("opened_audio_files");
  if (
    !Array.isArray(response) ||
    !response.every((path) => typeof path === "string")
  ) {
    throw new Error("Invalid opened files response.");
  }
  return response as string[];
}

export interface FolderBookmarkData {
  path: string;
  name: string;
}

export async function listFolderBookmarks(): Promise<FolderBookmarkData[]> {
  const response = await invokeCommand<{ bookmarks: FolderBookmarkData[] }>(
    "list_folder_bookmarks",
    {},
  );
  return response.bookmarks;
}

export async function addFolderBookmark(path: string): Promise<void> {
  await invokeCommand<Record<string, never>>("add_folder_bookmark", { path });
}

export async function removeFolderBookmark(path: string): Promise<void> {
  await invokeCommand<Record<string, never>>("remove_folder_bookmark", {
    path,
  });
}
