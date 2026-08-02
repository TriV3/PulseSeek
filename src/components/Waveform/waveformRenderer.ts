/** Pure geometry and Canvas 2D drawing for the waveform envelope. */

/** Renderer style for the waveform envelope. */
export type WaveformStyle = "solid" | "gradient" | "outline";

export interface WaveformTokens {
  wave: string;
  waveGrid: string;
  waveSoft: string;
  playhead: string;
}

export const DEFAULT_TOKENS: WaveformTokens = {
  wave: "#7f91a2",
  waveGrid: "#cbd3db",
  waveSoft: "#a7b6c4",
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
  fill(): void;
  closePath(): void;
  setTransform(
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
    f: number,
  ): void;
  setLineDash(segments: number[]): void;
  createLinearGradient(
    x0: number,
    y0: number,
    x1: number,
    y1: number,
  ): CanvasGradient2D;
  strokeStyle: unknown;
  fillStyle: unknown;
  lineWidth: number;
  lineCap: string;
  lineJoin: string;
}

/** Minimal gradient object used by the renderer. */
export interface CanvasGradient2D {
  addColorStop(offset: number, color: string): void;
}

/**
 * Draws a resolved envelope geometry onto a Canvas 2D context.
 *
 * All colors come from `tokens`, which the caller resolves from semantic
 * design tokens; the renderer never hard-codes a theme color.
 *
 * `style` selects how each channel's envelope is painted:
 * - `outline` fills the envelope body with a soft token and strokes the
 *   min/max edges, so the waveform reads against the panel background;
 * - `solid` fills the closed area between the min and max edges;
 * - `gradient` fills the same area with a vertical token gradient that is
 *   strongest at the channel center and fades toward the row edges.
 */
export function drawEnvelope(
  ctx: Canvas2D,
  geometry: EnvelopeGeometry,
  tokens: WaveformTokens,
  widthPx: number,
  heightPx: number,
  style: WaveformStyle = "outline",
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
    if (style === "outline") {
      fillEnvelopeArea(ctx, channel, tokens, "outline");
      strokeEnvelopeEdges(ctx, channel, tokens.wave);
    } else {
      fillEnvelopeArea(ctx, channel, tokens, style);
    }
  }

  if (geometry.playheadX !== null) {
    ctx.strokeStyle = tokens.playhead;
    ctx.lineWidth = 1;
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(geometry.playheadX, 0);
    ctx.lineTo(geometry.playheadX, heightPx);
    ctx.stroke();
    ctx.setLineDash([]);
  }
}

/** Strokes the min and max envelope polylines of one channel. */
function strokeEnvelopeEdges(
  ctx: Canvas2D,
  channel: EnvelopeChannel,
  color: string,
): void {
  ctx.strokeStyle = color;
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

/** Fills the closed area between the min and max edges of one channel. */
function fillEnvelopeArea(
  ctx: Canvas2D,
  channel: EnvelopeChannel,
  tokens: WaveformTokens,
  style: "solid" | "gradient" | "outline",
): void {
  if (channel.minPoints.length === 0 || channel.maxPoints.length === 0) return;

  if (style === "gradient") {
    const gradient = ctx.createLinearGradient(
      0,
      channel.rowCenter - channel.rowHeight / 2,
      0,
      channel.rowCenter + channel.rowHeight / 2,
    );
    gradient.addColorStop(0, tokens.waveSoft);
    gradient.addColorStop(0.5, tokens.wave);
    gradient.addColorStop(1, tokens.waveSoft);
    ctx.fillStyle = gradient;
  } else {
    ctx.fillStyle = style === "outline" ? tokens.waveSoft : tokens.wave;
  }

  ctx.beginPath();
  tracePolyline(ctx, channel.maxPoints);
  for (let i = channel.minPoints.length - 1; i >= 0; i -= 1) {
    ctx.lineTo(channel.minPoints[i].x, channel.minPoints[i].y);
  }
  ctx.closePath();
  ctx.fill();
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
    waveSoft: read("--wave-soft", DEFAULT_TOKENS.waveSoft),
    playhead: read("--wave-seek-current", DEFAULT_TOKENS.playhead),
  };
}

/** Maps a canvas width to a requested bucket target (2 buckets per pixel). */
export function defaultTargetPeaksForWidth(widthPx: number): number {
  if (!Number.isFinite(widthPx) || widthPx <= 0) return 1;
  return Math.max(1, Math.ceil(widthPx * 2));
}

/**
 * Inverse of `playheadX`: maps a pointer x coordinate to a seek position in
 * milliseconds. Clamps into `[0, durationMs]` and returns `null` when the
 * duration or canvas width is unavailable.
 */
export function positionMsForX(
  xPx: number,
  widthPx: number,
  durationMs: number | null,
): number | null {
  if (durationMs === null || durationMs <= 0) return null;
  if (!Number.isFinite(xPx) || !Number.isFinite(widthPx) || widthPx <= 0) {
    return null;
  }
  return Math.round(clamp((xPx / widthPx) * durationMs, 0, durationMs));
}
