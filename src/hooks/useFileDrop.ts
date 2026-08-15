import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";

/** State exposed by [`useFileDrop`]: whether an external drag is hovering. */
export interface FileDropState {
  /** True while the user drags files over the window (FR-DI-001). */
  active: boolean;
}

/**
 * Tracks external file drag-and-drop events delivered by Tauri and reports
 * the hover state. On drop, the absolute dropped `paths` are handed to
 * `onDrop` exactly once; the webview default that would navigate to the
 * dropped file is suppressed on `dragover`/`drop`.
 *
 * The `onDrop` callback is read through a ref so a stable subscription never
 * captures a stale closure.
 */
export function useFileDrop(onDrop: (paths: string[]) => void): FileDropState {
  const [active, setActive] = useState(false);
  const onDropRef = useRef(onDrop);

  useEffect(() => {
    onDropRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let disposed = false;

    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const type = event.payload.type;
        if (type === "enter" || type === "over") {
          setActive(true);
        } else if (type === "drop") {
          setActive(false);
          onDropRef.current(event.payload.paths);
        } else {
          setActive(false);
        }
      })
      .then((unlistenFn) => {
        if (disposed) unlistenFn();
        else unlisten = unlistenFn;
      })
      .catch(() => {
        // Listening is best-effort; browsing and playback still work.
      });

    // The webview would otherwise navigate to the dropped file. Preventing
    // the HTML5 default keeps the window on the application.
    const preventDefault = (event: DragEvent) => event.preventDefault();
    window.addEventListener("dragover", preventDefault);
    window.addEventListener("drop", preventDefault);

    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("dragover", preventDefault);
      window.removeEventListener("drop", preventDefault);
    };
  }, []);

  return { active };
}
