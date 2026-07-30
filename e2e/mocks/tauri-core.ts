// Mock for @tauri-apps/api/core — used during Playwright E2E tests.
// Delegates to window.__TAURI_BACKEND__ set up by the backend fixture.

export interface EnvelopePayload {
  version: number;
  command: string;
  payload: unknown;
}

export interface CommandResponse<T = unknown> {
  version: number;
  ok: boolean;
  data?: T;
  error?: { category: string; message: string; diagnostic_code: string };
}

function getBackend(): {
  invoke: (cmd: string, args?: unknown) => unknown;
} {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const w = window as any;
  if (!w.__TAURI_BACKEND__) {
    throw new Error("__TAURI_BACKEND__ not found. Did the addInitScript run?");
  }
  return w.__TAURI_BACKEND__;
}

export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const backend = getBackend();

  // Enveloped command path: invoke("invoke_command", { envelope })
  if (command === "invoke_command" && args?.envelope) {
    const envelope = args.envelope as EnvelopePayload;
    const data = backend.invoke(envelope.command, envelope.payload);
    return { version: 1, ok: true, data } as T;
  }

  // Direct command path: invoke("pick_folder_dialog", ...)
  const result = backend.invoke(command, args);
  return (result ?? {}) as T;
}
