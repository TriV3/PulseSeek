export type ShortcutPlatform = "macos" | "windows" | "linux";

export interface ShortcutChord {
  key: string;
  primary: boolean;
  shift: boolean;
  alt: boolean;
}

export type ShortcutBinding = ShortcutChord;

export const SHORTCUT_ACTIONS = [
  { id: "open_folder", label: "Open folder", available: true },
  { id: "toggle_play_pause", label: "Play / pause", available: true },
  { id: "play_selection", label: "Play selection", available: true },
  { id: "previous_track", label: "Previous track", available: true },
  { id: "next_track", label: "Next track", available: true },
  { id: "seek_backward", label: "Seek backward", available: true },
  { id: "seek_forward", label: "Seek forward", available: true },
  { id: "toggle_loop", label: "Toggle loop", available: true },
  { id: "move_to_trash", label: "Move to Trash", available: true },
  { id: "refresh", label: "Refresh", available: true },
  { id: "focus_search", label: "Search", available: true },
  { id: "set_playback_mode_one_shot", label: "One-shot mode", available: true },
  {
    id: "set_playback_mode_loop_current",
    label: "Loop-current mode",
    available: true,
  },
  {
    id: "set_playback_mode_sequential",
    label: "Sequential mode",
    available: true,
  },
  { id: "set_playback_mode_random", label: "Random mode", available: true },
  { id: "mark_keep", label: "Mark Keep", available: true },
  { id: "mark_maybe", label: "Mark Maybe", available: true },
  { id: "mark_reject", label: "Mark Reject", available: true },
  { id: "mark_favorite", label: "Mark Favorite", available: true },
  { id: "mark_clear", label: "Clear mark", available: true },
  { id: "set_ab_start", label: "Set A point", available: true },
  { id: "set_ab_end", label: "Set B point", available: true },
  { id: "toggle_ab_repeat", label: "Toggle A-B repeat", available: true },
] as const;

export type ShortcutActionId = (typeof SHORTCUT_ACTIONS)[number]["id"];
export interface ShortcutMapping {
  action: ShortcutActionId;
  chord: ShortcutChord;
}
export type ShortcutBindings = Record<ShortcutActionId, ShortcutBinding | null>;
export type ShortcutValidationErrors = Partial<
  Record<ShortcutActionId, string>
>;

export const DEFAULT_SHORTCUTS: ShortcutBindings = {
  open_folder: chord("o", true),
  toggle_play_pause: chord("space"),
  play_selection: chord("enter"),
  previous_track: chord("arrowup"),
  next_track: chord("arrowdown"),
  seek_backward: chord("arrowleft"),
  seek_forward: chord("arrowright"),
  toggle_loop: chord("l"),
  move_to_trash: chord("delete"),
  refresh: chord("r", true),
  focus_search: chord("f", true),
  set_playback_mode_one_shot: chord("1", true, false, true),
  set_playback_mode_loop_current: chord("2", true, false, true),
  set_playback_mode_sequential: chord("3", true, false, true),
  set_playback_mode_random: chord("4", true, false, true),
  mark_keep: chord("k", true, true),
  mark_maybe: chord("m", true, true),
  mark_reject: chord("r", true, true),
  mark_favorite: chord("f", true, true),
  mark_clear: chord("u", true, true),
  // A-B region selection: bracket keys place A/B at the playhead, "a"
  // toggles A-B repeat. Unmodified so they are reachable during playback.
  set_ab_start: chord("["),
  set_ab_end: chord("]"),
  toggle_ab_repeat: chord("a"),
};

function chord(
  key: string,
  primary = false,
  shift = false,
  alt = false,
): ShortcutBinding {
  return { key, primary, shift, alt };
}

export function getShortcutPlatform(): ShortcutPlatform {
  return /Mac/i.test(navigator.platform) || /Mac/i.test(navigator.userAgent)
    ? "macos"
    : /Win/i.test(navigator.platform) || /Win/i.test(navigator.userAgent)
      ? "windows"
      : "linux";
}

function normalizeKey(key: string): string {
  if (key === " ") return "space";
  const normalized = key.trim().toLocaleLowerCase();
  if (normalized === "spacebar") return "space";
  if (normalized === "esc") return "escape";
  return normalized;
}

export function shortcutFromEvent(
  event: KeyboardEvent,
  platform: ShortcutPlatform,
): ShortcutBinding | null {
  if (["Alt", "Control", "Meta", "Shift"].includes(event.key)) return null;
  if (
    (platform === "macos" && event.ctrlKey) ||
    (platform !== "macos" && event.metaKey)
  )
    return null;
  return {
    key: normalizeKey(event.key),
    primary: platform === "macos" ? event.metaKey : event.ctrlKey,
    shift: event.shiftKey,
    alt: event.altKey,
  };
}

export function matchShortcut(
  event: KeyboardEvent,
  binding: ShortcutBinding,
  platform: ShortcutPlatform,
): boolean {
  if (normalizeKey(event.key) !== normalizeKey(binding.key)) return false;
  return (
    (platform === "macos" ? event.metaKey : event.ctrlKey) ===
      binding.primary &&
    event.shiftKey === binding.shift &&
    event.altKey === binding.alt &&
    (platform === "macos" ? !event.ctrlKey : !event.metaKey)
  );
}

function shortcutSignature(binding: ShortcutBinding): string {
  return `${binding.primary}:${binding.shift}:${binding.alt}:${normalizeKey(binding.key)}`;
}

function keyLabel(key: string): string {
  const specialKeys: Record<string, string> = {
    space: "Space",
    arrowleft: "←",
    arrowright: "→",
    enter: "Enter",
    delete: "Delete",
    backspace: "Backspace",
    escape: "Escape",
    tab: "Tab",
  };
  if (specialKeys[key]) return specialKeys[key];
  if (/^f\d{1,2}$/.test(key)) return key.toLocaleUpperCase();
  return key.length === 1 ? key.toLocaleUpperCase() : key;
}

export function formatShortcut(
  binding: ShortcutBinding,
  platform: ShortcutPlatform,
): string {
  const labels: string[] = [];
  if (binding.primary) labels.push(platform === "macos" ? "⌘" : "Ctrl");
  if (binding.alt) labels.push(platform === "macos" ? "⌥" : "Alt");
  if (binding.shift) labels.push(platform === "macos" ? "⇧" : "Shift");
  const separator = platform === "macos" ? " " : "+";
  return [...labels, keyLabel(binding.key)].join(separator);
}

function isReserved(
  actionId: ShortcutActionId,
  binding: ShortcutBinding,
  platform: ShortcutPlatform,
): boolean {
  const key = normalizeKey(binding.key);
  if (
    key === "tab" ||
    key === "escape" ||
    (key === "enter" && actionId !== "play_selection") ||
    (binding.primary &&
      !binding.shift &&
      !binding.alt &&
      ["q", "w"].includes(key))
  ) {
    return true;
  }
  if (platform === "macos") {
    return (
      binding.primary &&
      !binding.shift &&
      !binding.alt &&
      ["h", "m", "space"].includes(key)
    );
  }
  return !binding.primary && !binding.shift && binding.alt && key === "f4";
}

export function validateShortcutBindings(
  bindings: ShortcutBindings,
  platform: ShortcutPlatform,
): ShortcutValidationErrors {
  const errors: ShortcutValidationErrors = {};
  const actionsById = new Map(
    SHORTCUT_ACTIONS.map((action) => [action.id, action]),
  );
  const assigned = new Map<string, ShortcutActionId[]>();

  for (const action of SHORTCUT_ACTIONS) {
    if (!action.available) continue;
    const binding = bindings[action.id];
    if (!binding) continue;
    if (
      ["alt", "altgraph", "control", "ctrl", "meta", "shift", "super"].includes(
        normalizeKey(binding.key),
      )
    ) {
      errors[action.id] = "Choose a non-modifier key.";
    }
    if (isReserved(action.id, binding, platform))
      errors[action.id] = "Reserved by system or interface.";
    const signature = shortcutSignature(binding);
    assigned.set(signature, [...(assigned.get(signature) ?? []), action.id]);
  }

  for (const ids of assigned.values()) {
    if (ids.length < 2) continue;
    for (const id of ids) {
      const other = ids.find((candidate) => candidate !== id);
      if (other)
        errors[id] = `Conflicts with ${actionsById.get(other)?.label}.`;
    }
  }
  return errors;
}
