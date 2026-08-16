import { useEffect, useRef } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

export const COMPACT_MIN_WINDOW_SIZE = { width: 440, height: 600 };
export const COMPACT_WINDOW_SIZE = COMPACT_MIN_WINDOW_SIZE;
export const DEFAULT_WINDOW_SIZE = { width: 1200, height: 800 };

export interface CompactWindowSize {
  width: number;
  height: number;
}

/** Never shrink the compact window below the documented minimum. */
const clampToMinimum = (size: CompactWindowSize): CompactWindowSize => ({
  width: Math.max(size.width, COMPACT_MIN_WINDOW_SIZE.width),
  height: Math.max(size.height, COMPACT_MIN_WINDOW_SIZE.height),
});

interface UseCompactWindowOptions {
  /** Last known non-compact window size, persisted across restarts. */
  savedSize: CompactWindowSize | null;
  /** Last known compact window size, persisted across restarts. */
  savedCompactSize: CompactWindowSize | null;
  /** Records the non-compact size when entering compact mode. */
  onRememberSize: (size: CompactWindowSize) => void;
  /** Records the compact size when the user resizes in compact mode. */
  onRememberCompactSize: (size: CompactWindowSize) => void;
  /** Preferences are still loading; ignore the initial false→true flip. */
  loaded: boolean;
}

/** Debounce for persisting resize events while in compact mode. */
const RESIZE_PERSIST_DELAY_MS = 400;

export function useCompactWindow(
  compact: boolean,
  {
    savedSize,
    savedCompactSize,
    onRememberSize,
    onRememberCompactSize,
    loaded,
  }: UseCompactWindowOptions,
) {
  const previousCompact = useRef(compact);
  const firstLoadedRun = useRef(true);
  const shouldRestoreOnExit = useRef(false);
  const compactActive = useRef(compact);
  // App passes fresh object/arrow refs on every render (it re-renders on
  // playback position ticks), so hold them in a ref and let the effect
  // depend only on the primitives that actually change behavior.
  const optionsRef = useRef({
    savedSize,
    savedCompactSize,
    onRememberSize,
    onRememberCompactSize,
  });
  useEffect(() => {
    optionsRef.current = {
      savedSize,
      savedCompactSize,
      onRememberSize,
      onRememberCompactSize,
    };
  });

  useEffect(() => {
    if (!loaded) {
      previousCompact.current = compact;
      return;
    }
    compactActive.current = compact;
    const {
      savedSize,
      savedCompactSize,
      onRememberSize,
      onRememberCompactSize,
    } = optionsRef.current;
    const enteringCompact = firstLoadedRun.current
      ? false
      : compact && !previousCompact.current;
    const startupInCompact = firstLoadedRun.current && compact;
    firstLoadedRun.current = false;
    previousCompact.current = compact;
    let active = true;
    const win = getCurrentWindow();
    void win.isMaximized().then(async (maximized) => {
      if (!active) return;
      if (enteringCompact) {
        // A maximized window is never shrunk: the compact layout still
        // applies, and leaving compact mode leaves the window untouched.
        if (maximized) {
          shouldRestoreOnExit.current = false;
          return;
        }
        const [size, scaleFactor] = await Promise.all([
          win.innerSize(),
          win.scaleFactor(),
        ]);
        if (!active) return;
        const logical = {
          width: Math.round(size.width / scaleFactor),
          height: Math.round(size.height / scaleFactor),
        };
        shouldRestoreOnExit.current = true;
        onRememberSize(logical);
        const compactSize = clampToMinimum(
          savedCompactSize ?? COMPACT_WINDOW_SIZE,
        );
        // The minimum is best-effort: a rejection must never abort the
        // resize (e.g. missing window permission or an unsupported window
        // manager) — the compact layout still needs its size.
        await win
          .setMinSize(
            new LogicalSize(
              COMPACT_MIN_WINDOW_SIZE.width,
              COMPACT_MIN_WINDOW_SIZE.height,
            ),
          )
          .catch(() => undefined);
        await win.setSize(
          new LogicalSize(compactSize.width, compactSize.height),
        );
      } else if (startupInCompact) {
        // Launching already in compact mode: never shrink, but remember the
        // window must be resized when the user leaves compact mode. When a
        // compact size was persisted, restore it so the previous session's
        // compact layout returns unchanged.
        shouldRestoreOnExit.current = !maximized;
        if (!maximized) {
          await win
            .setMinSize(
              new LogicalSize(
                COMPACT_MIN_WINDOW_SIZE.width,
                COMPACT_MIN_WINDOW_SIZE.height,
              ),
            )
            .catch(() => undefined);
          if (savedCompactSize) {
            const compactSize = clampToMinimum(savedCompactSize);
            await win.setSize(
              new LogicalSize(compactSize.width, compactSize.height),
            );
          }
        }
      } else if (!compact && shouldRestoreOnExit.current) {
        // Restore the size remembered on entry, or the documented default
        // when the app was launched already in compact mode and no size was
        // remembered this session. The persisted savedSize covers the
        // previous session's entry, so restarting while compact still
        // restores the pre-compact window size. The compact minimum is
        // lifted before the restore so a normal window smaller than the
        // compact minimum is not clamped.
        const { width, height } = savedSize ?? DEFAULT_WINDOW_SIZE;
        shouldRestoreOnExit.current = false;
        await win.setMinSize(null).catch(() => undefined);
        await win.setSize(new LogicalSize(width, height));
      }
    });

    // While compact, persist the window size (debounced) so the user's
    // compact layout survives a restart. Resizes caused by leaving compact
    // mode are ignored because compactActive flips before the restore. The
    // listener is only registered while compact, so non-compact users never
    // touch the Tauri window.
    if (!compact) return;
    let unlistenResize: (() => void) | undefined;
    let resizeTimer: number | undefined;
    const handleResize = () => {
      if (!compactActive.current) return;
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        void (async () => {
          if (!active || !compactActive.current) return;
          const [maximized, size, scaleFactor] = await Promise.all([
            win.isMaximized(),
            win.innerSize(),
            win.scaleFactor(),
          ]);
          if (!active || !compactActive.current || maximized) return;
          onRememberCompactSize(
            clampToMinimum({
              width: Math.round(size.width / scaleFactor),
              height: Math.round(size.height / scaleFactor),
            }),
          );
        })();
      }, RESIZE_PERSIST_DELAY_MS);
    };
    void win.onResized(handleResize).then((unlisten) => {
      // The cleanup may have already run while the registration promise was
      // pending; release the listener immediately instead of losing it in a
      // dead closure.
      if (!active) unlisten();
      else unlistenResize = unlisten;
    });

    return () => {
      active = false;
      compactActive.current = false;
      window.clearTimeout(resizeTimer);
      unlistenResize?.();
    };
  }, [compact, loaded]);
}
