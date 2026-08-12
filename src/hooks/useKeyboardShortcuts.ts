import { useEffect } from "react";
import {
  DEFAULT_SHORTCUTS,
  getShortcutPlatform,
  matchShortcut,
  SHORTCUT_ACTIONS,
  type ShortcutActionId,
  type ShortcutBindings,
} from "../shortcuts/keyboardShortcuts";

export interface KeyboardShortcutActions {
  onOpenFolder?: () => void | Promise<void>;
  onTogglePlayPause?: () => void | Promise<void>;
  onPreviousTrack?: () => void | Promise<void>;
  onNextTrack?: () => void | Promise<void>;
  onSeekBackward?: () => void | Promise<void>;
  onSeekForward?: () => void | Promise<void>;
  onToggleLoop?: () => void | Promise<void>;
  onMoveToTrash?: () => void | Promise<void>;
  onMarkKeep?: () => void | Promise<void>;
  onMarkMaybe?: () => void | Promise<void>;
  onMarkReject?: () => void | Promise<void>;
  onMarkFavorite?: () => void | Promise<void>;
  onMarkClear?: () => void | Promise<void>;
  onPlaySelection?: () => void | Promise<void>;
  onRefresh?: () => void | Promise<void>;
  onFocusSearch?: () => void | Promise<void>;
  onSetAbStart?: () => void | Promise<void>;
  onSetAbEnd?: () => void | Promise<void>;
  onToggleAbRepeat?: () => void | Promise<void>;
  onSetPlaybackModeOneShot?: () => void | Promise<void>;
  onSetPlaybackModeLoopCurrent?: () => void | Promise<void>;
  onSetPlaybackModeSequential?: () => void | Promise<void>;
  onSetPlaybackModeRandom?: () => void | Promise<void>;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return (
    target.matches("input, textarea, [contenteditable]") ||
    target.closest("input, textarea, [contenteditable]") !== null
  );
}

function isNativeActivationTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest(
      "button, a, select, [role='button'], [role='checkbox'], [role='combobox'], [role='link'], [role='listbox'], [role='menu'], [role='menuitem'], [role='radio'], [role='slider'], [role='spinbutton'], [role='switch'], [role='tab']",
    ) !== null
  );
}

function isNavigationTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest(
      "[role='grid'], [role='tree'], [role='slider'], [role='separator'], [role='listbox'], [role='menu']",
    ) !== null
  );
}

function isModalTarget(target: EventTarget | null): boolean {
  const modalSelector =
    "[aria-modal='true'], [role='dialog'], [role='alertdialog']";
  return (
    (target instanceof Element && target.closest(modalSelector) !== null) ||
    document.querySelector(modalSelector) !== null
  );
}

const ACTION_CALLBACKS: Record<
  ShortcutActionId,
  keyof KeyboardShortcutActions | null
> = {
  open_folder: "onOpenFolder",
  toggle_play_pause: "onTogglePlayPause",
  play_selection: "onPlaySelection",
  previous_track: "onPreviousTrack",
  next_track: "onNextTrack",
  seek_backward: "onSeekBackward",
  seek_forward: "onSeekForward",
  toggle_loop: "onToggleLoop",
  move_to_trash: "onMoveToTrash",
  refresh: "onRefresh",
  focus_search: "onFocusSearch",
  set_playback_mode_one_shot: "onSetPlaybackModeOneShot",
  set_playback_mode_loop_current: "onSetPlaybackModeLoopCurrent",
  set_playback_mode_sequential: "onSetPlaybackModeSequential",
  set_playback_mode_random: "onSetPlaybackModeRandom",
  mark_keep: "onMarkKeep",
  mark_maybe: "onMarkMaybe",
  mark_reject: "onMarkReject",
  mark_favorite: "onMarkFavorite",
  mark_clear: "onMarkClear",
  set_ab_start: "onSetAbStart",
  set_ab_end: "onSetAbEnd",
  toggle_ab_repeat: "onToggleAbRepeat",
};

export function useKeyboardShortcuts(
  actions: KeyboardShortcutActions,
  bindings: ShortcutBindings = DEFAULT_SHORTCUTS,
): void {
  useEffect(() => {
    const platform = getShortcutPlatform();

    function onKeyDown(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.isComposing ||
        isModalTarget(event.target)
      )
        return;
      const editable = isEditableTarget(event.target);
      if (
        isNavigationTarget(event.target) &&
        [
          "ArrowDown",
          "ArrowUp",
          "ArrowLeft",
          "ArrowRight",
          "Home",
          "End",
          "PageDown",
          "PageUp",
        ].includes(event.key)
      ) {
        return;
      }
      if (
        !editable &&
        isNativeActivationTarget(event.target) &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.shiftKey &&
        !event.altKey &&
        [" ", "Enter"].includes(event.key)
      )
        return;

      for (const action of SHORTCUT_ACTIONS) {
        if (editable && action.id !== "focus_search") continue;
        const callbackName = ACTION_CALLBACKS[action.id];
        const binding = bindings[action.id];
        if (!callbackName || !binding) continue;
        const callback = actions[callbackName];
        if (!callback || !matchShortcut(event, binding, platform)) continue;
        event.preventDefault();
        void callback();
        return;
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [actions, bindings]);
}
