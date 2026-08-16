import type { SpectrumFramePayload } from "../../api/playbackEvents";

export const MIN_DECIBELS = -90;

export interface LinearAnalyzerTokens {
  spectrum: string;
  spectrumSoft: string;
  grid: string;
  label: string;
}

export interface LinearAnalyzerCanvas2D {
  clearRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
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

const FREQUENCY_GUIDES_HZ = [0, 5_000, 10_000, 15_000, 20_000];

export function frequencyToLinearX(
  frequencyHz: number,
  sampleRate: number,
  widthPx: number,
): number {
  if (widthPx <= 0 || sampleRate <= 0) return 0;
  const nyquist = sampleRate / 2;
  const clamped = Math.max(0, Math.min(nyquist, frequencyHz));
  return (clamped / nyquist) * widthPx;
}

export function magnitudeToY(magnitude: number, heightPx: number): number {
  if (heightPx <= 0) return 0;
  const floorMagnitude = 10 ** (MIN_DECIBELS / 20);
  const decibels = 20 * Math.log10(Math.max(floorMagnitude, magnitude));
  const clamped = Math.max(MIN_DECIBELS, Math.min(0, decibels));
  return (1 - (clamped - MIN_DECIBELS) / -MIN_DECIBELS) * heightPx;
}

export function drawLinearAnalyzer(
  context: LinearAnalyzerCanvas2D,
  frame: SpectrumFramePayload | null,
  widthPx: number,
  heightPx: number,
  tokens: LinearAnalyzerTokens,
): void {
  if (widthPx <= 0 || heightPx <= 0) return;
  context.clearRect(0, 0, widthPx, heightPx);
  drawGrid(context, frame?.sample_rate ?? 48_000, widthPx, heightPx, tokens);
  if (!frame || frame.magnitudes.length === 0) return;

  const binWidthHz = frame.sample_rate / frame.fft_size;
  const points: AnalyzerPoint[] = [];
  for (let index = 0; index < frame.magnitudes.length; index += 1) {
    const point = {
      x: frequencyToLinearX(index * binWidthHz, frame.sample_rate, widthPx),
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
  const first = points[0];
  if (!first) return;

  context.beginPath();
  context.moveTo(0, heightPx);
  context.lineTo(0, first.y);
  tracePoints(context, points);
  context.lineTo(widthPx, heightPx);
  context.closePath();
  context.fillStyle = tokens.spectrumSoft;
  context.fill();

  context.beginPath();
  context.moveTo(0, first.y);
  tracePoints(context, points);
  context.strokeStyle = tokens.spectrum;
  context.lineWidth = 1.5;
  context.stroke();
}

function tracePoints(
  context: LinearAnalyzerCanvas2D,
  points: AnalyzerPoint[],
): void {
  for (let index = 1; index < points.length; index += 1) {
    const point = points[index];
    if (point) context.lineTo(point.x, point.y);
  }
}

function drawGrid(
  context: LinearAnalyzerCanvas2D,
  sampleRate: number,
  widthPx: number,
  heightPx: number,
  tokens: LinearAnalyzerTokens,
): void {
  const nyquist = sampleRate / 2;
  context.strokeStyle = tokens.grid;
  context.fillStyle = tokens.label;
  context.lineWidth = 1;
  context.font = "9px system-ui, sans-serif";

  for (const frequencyHz of FREQUENCY_GUIDES_HZ) {
    if (frequencyHz > nyquist) break;
    const x = frequencyToLinearX(frequencyHz, sampleRate, widthPx);
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, heightPx);
    context.stroke();
    context.fillText(formatFrequency(frequencyHz), x + 3, heightPx - 5);
  }
}

function formatFrequency(frequencyHz: number): string {
  return frequencyHz >= 1_000 ? `${frequencyHz / 1_000}k` : String(frequencyHz);
}
