/** Pure geometry and Canvas 2D drawing for the waveform envelope. */

export interface WaveformTokens {
  wave: string;
  waveGrid: string;
  playhead: string;
}

export const DEFAULT_TOKENS: WaveformTokens = {
  wave: "#7f91a2",
  waveGrid: "#cbd3db",
  playhead: "#f29c38",
};

export interface Point {
  x: number;
  y: number;
}

export interface EnvelopeChannel {
  rowCenter: number;
  rowHeight: number;
  /** Lower edge of the envelope, one point per bucket. */
  minPoints: Point[];
  /** Upper edge of the envelope, one point per bucket. */
  maxPoints: Point[];
}

export interface EnvelopeGeometry {
  channels: EnvelopeChannel[];
  /** Progress playhead x coordinate, or null when unknown. */
  playheadX: number | null;
}

export interface WaveformSource {
  channels: number;
  min: number[];
  max: number[];
}

const AMPLITUDE_MAX = 1;

/**
 * Maps interleaved waveform peaks to envelope polylines per channel.
 *
 * Peaks are interleaved by bucket then channel (`[ch0, ch1, ch0, ch1, ...]`),
 * matching the backend payload. Each channel gets its own horizontal row.
 */
export function buildEnvelope(
  source: WaveformSource,
  widthPx: number,
  heightPx: number,
  positionMs: number | null,
  durationMs: number | null,
): EnvelopeGeometry {
  if (widthPx <= 0 || heightPx <= 0 || source.channels <= 0) {
    return { channels: [], playheadX: null };
  }

  const channels = source.channels;
  const peaks = source.min.length;
  const buckets = Math.floor(peaks / channels);
  const rowHeight = heightPx / channels;

  const channelRows: EnvelopeChannel[] = [];
  for (let channel = 0; channel < channels; channel += 1) {
    const rowCenter = channel * rowHeight + rowHeight / 2;
    const minPoints: Point[] = [];
    const maxPoints: Point[] = [];
    for (let bucket = 0; bucket < buckets; bucket += 1) {
      const index = bucket * channels + channel;
      if (index >= peaks) break;
      const x = ((bucket + 0.5) * widthPx) / buckets;
      minPoints.push({
        x,
        y: envelopeY(rowCenter, rowHeight, source.min[index], channel),
      });
      maxPoints.push({
        x,
        y: envelopeY(rowCenter, rowHeight, source.max[index], channel),
      });
    }
    channelRows.push({ rowCenter, rowHeight, minPoints, maxPoints });
  }

  return {
    channels: channelRows,
    playheadX: playheadX(widthPx, positionMs, durationMs),
  };
}

function envelopeY(
  rowCenter: number,
  rowHeight: number,
  amplitude: number,
  channel: number,
): number {
  const clamped = clampAmplitude(amplitude);
  const scale = rowHeight / 2;
  const y = rowCenter - clamped * scale;
  return clamp(y, channel * rowHeight, (channel + 1) * rowHeight);
}

function clampAmplitude(value: number): number {
  return Math.max(-AMPLITUDE_MAX, Math.min(AMPLITUDE_MAX, value));
}

function playheadX(
  widthPx: number,
  positionMs: number | null,
  durationMs: number | null,
): number | null {
  if (positionMs === null || durationMs === null || durationMs <= 0)
    return null;
  return clamp((positionMs / durationMs) * widthPx, 0, widthPx);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}

/** Minimal structural subset of the Canvas 2D context used by the renderer. */
export interface Canvas2D {
  clearRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  stroke(): void;
  setTransform(
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
    f: number,
  ): void;
  setLineDash(segments: number[]): void;
  strokeStyle: unknown;
  lineWidth: number;
  lineCap: string;
  lineJoin: string;
}

/**
 * Draws a resolved envelope geometry onto a Canvas 2D context.
 *
 * All colors come from `tokens`, which the caller resolves from semantic
 * design tokens; the renderer never hard-codes a theme color.
 */
export function drawEnvelope(
  ctx: Canvas2D,
  geometry: EnvelopeGeometry,
  tokens: WaveformTokens,
  widthPx: number,
  heightPx: number,
): void {
  ctx.clearRect(0, 0, widthPx, heightPx);
  ctx.setLineDash([]);

  for (const channel of geometry.channels) {
    ctx.strokeStyle = tokens.waveGrid;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, channel.rowCenter);
    ctx.lineTo(widthPx, channel.rowCenter);
    ctx.stroke();
  }

  for (const channel of geometry.channels) {
    ctx.strokeStyle = tokens.wave;
    ctx.lineWidth = 1.5;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    tracePolyline(ctx, channel.minPoints);
    ctx.stroke();
    ctx.beginPath();
    tracePolyline(ctx, channel.maxPoints);
    ctx.stroke();
  }

  if (geometry.playheadX !== null) {
    ctx.strokeStyle = tokens.playhead;
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 3]);
    ctx.beginPath();
    ctx.moveTo(geometry.playheadX, 0);
    ctx.lineTo(geometry.playheadX, heightPx);
    ctx.stroke();
    ctx.setLineDash([]);
  }
}

function tracePolyline(ctx: Canvas2D, points: Point[]): void {
  if (points.length === 0) return;
  ctx.moveTo(points[0].x, points[0].y);
  for (let i = 1; i < points.length; i += 1) {
    ctx.lineTo(points[i].x, points[i].y);
  }
}

/**
 * Resolves waveform colors from semantic design tokens on `scope`.
 *
 * Falls back to neutral defaults when the tokens are unavailable so the
 * renderer still works outside a fully themed document.
 */
export function resolveTokens(
  scope: Element | null | undefined,
): WaveformTokens {
  if (!scope) return DEFAULT_TOKENS;
  const style = window.getComputedStyle(scope);
  const read = (name: string, fallback: string) =>
    style.getPropertyValue(name).trim() || fallback;
  return {
    wave: read("--wave", DEFAULT_TOKENS.wave),
    waveGrid: read("--wave-grid", DEFAULT_TOKENS.waveGrid),
    playhead: read("--wave-playhead", DEFAULT_TOKENS.playhead),
  };
}

/** Maps a canvas width to a requested bucket target (2 buckets per pixel). */
export function defaultTargetPeaksForWidth(widthPx: number): number {
  if (!Number.isFinite(widthPx) || widthPx <= 0) return 1;
  return Math.max(1, Math.ceil(widthPx * 2));
}
