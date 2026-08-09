import type { SpectrumFramePayload } from "../../api/playbackEvents";

export const MIN_FREQUENCY_HZ = 10;
export const MIN_DECIBELS = -90;

export interface AnalyzerTokens {
  spectrum: string;
  spectrumSoft: string;
  grid: string;
  label: string;
}

export interface AnalyzerCanvas2D {
  clearRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  quadraticCurveTo(
    controlX: number,
    controlY: number,
    x: number,
    y: number,
  ): void;
  closePath(): void;
  stroke(): void;
  fill(): void;
  fillText(text: string, x: number, y: number): void;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  fillStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  font: string;
}

interface AnalyzerPoint {
  x: number;
  y: number;
}

const FREQUENCY_GUIDES = [
  10, 20, 50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 20_000,
];

export function frequencyToX(
  frequencyHz: number,
  sampleRate: number,
  widthPx: number,
): number {
  if (widthPx <= 0 || sampleRate <= MIN_FREQUENCY_HZ * 2) return 0;
  const nyquist = sampleRate / 2;
  const clamped = Math.max(MIN_FREQUENCY_HZ, Math.min(nyquist, frequencyHz));
  const normalized =
    Math.log(clamped / MIN_FREQUENCY_HZ) / Math.log(nyquist / MIN_FREQUENCY_HZ);
  return normalized * widthPx;
}

export function magnitudeToY(magnitude: number, heightPx: number): number {
  if (heightPx <= 0) return 0;
  const floorMagnitude = 10 ** (MIN_DECIBELS / 20);
  const decibels = 20 * Math.log10(Math.max(floorMagnitude, magnitude));
  const clamped = Math.max(MIN_DECIBELS, Math.min(0, decibels));
  return (1 - (clamped - MIN_DECIBELS) / -MIN_DECIBELS) * heightPx;
}

export function drawLogAnalyzer(
  context: AnalyzerCanvas2D,
  frame: SpectrumFramePayload | null,
  widthPx: number,
  heightPx: number,
  tokens: AnalyzerTokens,
): void {
  if (widthPx <= 0 || heightPx <= 0) return;
  context.clearRect(0, 0, widthPx, heightPx);
  drawGrid(context, frame?.sample_rate ?? 48_000, widthPx, heightPx, tokens);
  if (!frame) return;

  const binWidthHz = frame.sample_rate / frame.fft_size;
  const firstBin = Math.max(1, Math.ceil(MIN_FREQUENCY_HZ / binWidthHz));
  if (firstBin >= frame.magnitudes.length) return;

  const points: AnalyzerPoint[] = [];
  for (let index = firstBin; index < frame.magnitudes.length; index += 1) {
    const frequencyHz = index * binWidthHz;
    const point = {
      x: frequencyToX(frequencyHz, frame.sample_rate, widthPx),
      y: magnitudeToY(frame.magnitudes[index] ?? 0, heightPx),
    };
    const previous = points.at(-1);
    if (previous && Math.round(previous.x) === Math.round(point.x)) {
      previous.y = Math.min(previous.y, point.y);
      previous.x = point.x;
    } else {
      points.push(point);
    }
  }
  if (points.length === 0) return;

  const first = points[0];
  if (!first) return;

  context.beginPath();
  context.moveTo(0, heightPx);
  context.lineTo(0, first.y);
  traceSpectrumCurve(context, points, widthPx);
  context.lineTo(widthPx, heightPx);
  context.closePath();
  context.fillStyle = tokens.spectrumSoft;
  context.fill();

  context.beginPath();
  context.moveTo(0, first.y);
  traceSpectrumCurve(context, points, widthPx);
  context.strokeStyle = tokens.spectrum;
  context.lineWidth = 1.5;
  context.stroke();
}

function traceSpectrumCurve(
  context: AnalyzerCanvas2D,
  points: AnalyzerPoint[],
  widthPx: number,
): void {
  const first = points[0];
  if (!first) return;
  context.lineTo(first.x, first.y);
  for (let index = 0; index < points.length - 1; index += 1) {
    const current = points[index];
    const next = points[index + 1];
    if (!current || !next) continue;
    context.quadraticCurveTo(
      current.x,
      current.y,
      (current.x + next.x) / 2,
      (current.y + next.y) / 2,
    );
  }
  const last = points.at(-1);
  if (!last) return;
  context.lineTo(last.x, last.y);
  if (last.x < widthPx) context.lineTo(widthPx, last.y);
}

function drawGrid(
  context: AnalyzerCanvas2D,
  sampleRate: number,
  widthPx: number,
  heightPx: number,
  tokens: AnalyzerTokens,
): void {
  const nyquist = sampleRate / 2;
  context.strokeStyle = tokens.grid;
  context.fillStyle = tokens.label;
  context.lineWidth = 1;
  context.font = "9px system-ui, sans-serif";
  for (const frequencyHz of FREQUENCY_GUIDES) {
    if (frequencyHz > nyquist) break;
    const x = frequencyToX(frequencyHz, sampleRate, widthPx);
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, heightPx);
    context.stroke();
    if (
      frequencyHz === 10 ||
      frequencyHz === 100 ||
      frequencyHz === 1_000 ||
      frequencyHz === 10_000
    ) {
      context.fillText(formatFrequency(frequencyHz), x + 3, heightPx - 5);
    }
  }
}

function formatFrequency(frequencyHz: number): string {
  return frequencyHz >= 1_000 ? `${frequencyHz / 1_000}k` : String(frequencyHz);
}
