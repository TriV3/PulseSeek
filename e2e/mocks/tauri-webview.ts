// Mock for @tauri-apps/api/webview — used during Playwright E2E tests.
// The real Tauri webview delivers external drag-and-drop events with absolute
// file paths; the mock bridges those events through __TAURI_BACKEND__ under
// the same "tauri://drag-drop" channel so tests can emit them with emitEvent.

export type UnlistenFn = () => void;

export interface DragDropEvent {
  type: "enter" | "over" | "drop" | "leave" | "cancel";
  paths?: string[];
  position?: { x: number; y: number };
}

function getBackend(): {
  listen: (
    event: string,
    handler: (event: { payload: unknown }) => void,
  ) => UnlistenFn;
} {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const w = window as any;
  if (!w.__TAURI_BACKEND__) {
    throw new Error("__TAURI_BACKEND__ not found. Did the addInitScript run?");
  }
  return w.__TAURI_BACKEND__;
}

export function getCurrentWebview(): {
  onDragDropEvent: (
    handler: (event: { payload: DragDropEvent }) => void,
  ) => Promise<UnlistenFn>;
} {
  return {
    onDragDropEvent: (
      handler: (event: { payload: DragDropEvent }) => void,
    ): Promise<UnlistenFn> => {
      const backend = getBackend();
      const unlisten = backend.listen("tauri://drag-drop", (event) => {
        handler({ payload: event.payload as DragDropEvent });
      });
      return Promise.resolve(unlisten);
    },
  };
}
