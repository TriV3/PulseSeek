// Mock for @tauri-apps/api/event — used during Playwright E2E tests.
// Delegates to window.__TAURI_BACKEND__ set up by the backend fixture.

export type UnlistenFn = () => void;

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

export async function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  const backend = getBackend();
  return backend.listen(
    event,
    handler as (event: { payload: unknown }) => void,
  );
}
