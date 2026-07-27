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

/**
 * Health check command. Returns `true` when the Rust backend responds.
 */
export async function healthCheck(): Promise<boolean> {
  const response = await invokeCommand<HealthResponse>("health", {});
  return response.ready;
}
