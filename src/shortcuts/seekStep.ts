export const SEEK_STEP_PRESETS = [1, 2, 5, 10, 15, 20, 30] as const;
export type SeekStepMode = "auto" | `${(typeof SEEK_STEP_PRESETS)[number]}s`;

export function seekStepMs(
  mode: SeekStepMode,
  durationMs: number | null,
): number {
  if (mode !== "auto") return Number.parseInt(mode, 10) * 1_000;
  if (durationMs === null || !Number.isFinite(durationMs) || durationMs <= 0) {
    return 5_000;
  }
  if (durationMs < 15_000) return 1_000;
  if (durationMs < 60_000) return 2_000;
  if (durationMs < 7 * 60_000) return 5_000;
  if (durationMs < 15 * 60_000) return 10_000;
  if (durationMs < 30 * 60_000) return 15_000;
  if (durationMs <= 60 * 60_000) return 20_000;
  return 30_000;
}
