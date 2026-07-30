import { useEffect } from "react";

export interface KeyboardShortcutActions {
  onOpenFolder?: () => void | Promise<void>;
  onTogglePlayPause?: () => void | Promise<void>;
  onPreviousTrack?: () => void | Promise<void>;
  onNextTrack?: () => void | Promise<void>;
  onSeekBackward?: () => void | Promise<void>;
  onSeekForward?: () => void | Promise<void>;
  onToggleLoop?: () => void | Promise<void>;
  onMoveToTrash?: () => void | Promise<void>;
}

function isMacPlatform(): boolean {
  return /Mac/i.test(navigator.platform) || /Mac/i.test(navigator.userAgent);
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return (
    target.matches("input, textarea, select, [contenteditable]") ||
    target.closest("input, textarea, select, [contenteditable]") !== null
  );
}

function isInsideFileGrid(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest("[role='grid']") !== null;
}

export function useKeyboardShortcuts(actions: KeyboardShortcutActions): void {
  useEffect(() => {
    const modifierKey = isMacPlatform() ? "metaKey" : "ctrlKey";

    function onKeyDown(event: KeyboardEvent) {
      const editable = isEditableTarget(event.target);
      const inFileGrid = isInsideFileGrid(event.target);
      const hasPlatformModifier = event[modifierKey];

      if (editable) return;

      if (
        hasPlatformModifier &&
        event.key.toLowerCase() === "o" &&
        actions.onOpenFolder
      ) {
        event.preventDefault();
        void actions.onOpenFolder();
        return;
      }

      if (
        hasPlatformModifier &&
        event.key === "ArrowLeft" &&
        actions.onPreviousTrack
      ) {
        event.preventDefault();
        void actions.onPreviousTrack();
        return;
      }

      if (
        hasPlatformModifier &&
        event.key === "ArrowRight" &&
        actions.onNextTrack
      ) {
        event.preventDefault();
        void actions.onNextTrack();
        return;
      }

      if (
        !hasPlatformModifier &&
        event.key === " " &&
        actions.onTogglePlayPause
      ) {
        if (
          event.target instanceof HTMLElement &&
          event.target.matches("button, a")
        ) {
          return;
        }
        event.preventDefault();
        void actions.onTogglePlayPause();
        return;
      }

      if (
        !hasPlatformModifier &&
        event.key === "ArrowLeft" &&
        !inFileGrid &&
        actions.onSeekBackward
      ) {
        event.preventDefault();
        void actions.onSeekBackward();
        return;
      }

      if (
        !hasPlatformModifier &&
        event.key === "ArrowRight" &&
        !inFileGrid &&
        actions.onSeekForward
      ) {
        event.preventDefault();
        void actions.onSeekForward();
        return;
      }

      if (
        !hasPlatformModifier &&
        event.key.toLowerCase() === "l" &&
        actions.onToggleLoop
      ) {
        event.preventDefault();
        void actions.onToggleLoop();
        return;
      }

      if (
        !hasPlatformModifier &&
        (event.key === "Delete" || event.key === "Backspace") &&
        !inFileGrid &&
        actions.onMoveToTrash
      ) {
        event.preventDefault();
        void actions.onMoveToTrash();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [actions]);
}
