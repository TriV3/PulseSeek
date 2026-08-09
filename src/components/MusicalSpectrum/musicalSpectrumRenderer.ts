import type { MusicalSpectrumFramePayload } from "../../api/playbackEvents";

const MIN_DECIBELS = -90;
const NOTE_NAMES = [
  "C",
  "C♯",
  "D",
  "D♯",
  "E",
  "F",
  "F♯",
  "G",
  "G♯",
  "A",
  "A♯",
  "B",
];

export interface MusicalSpectrumTokens {
  spectrum: string;
  spectrumSoft: string;
  grid: string;
  label: string;
}

export interface MusicalSpectrumCanvas2D {
  clearRect(x: number, y: number, width: number, height: number): void;
  fillRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  stroke(): void;
  fillText(text: string, x: number, y: number): void;
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  font: string;
}

export function noteLabel(noteNumber: number): string {
  const pitchClass = ((noteNumber % 12) + 12) % 12;
  const octave = Math.floor(noteNumber / 12) - 1;
  return `${NOTE_NAMES[pitchClass] ?? "?"}${octave}`;
}

export function drawMusicalSpectrum(
  context: MusicalSpectrumCanvas2D,
  frame: MusicalSpectrumFramePayload | null,
  widthPx: number,
  heightPx: number,
  tokens: MusicalSpectrumTokens,
): void {
  if (widthPx <= 0 || heightPx <= 0) return;
  context.clearRect(0, 0, widthPx, heightPx);
  if (!frame || frame.bands.length === 0) return;

  const bandWidth = widthPx / frame.bands.length;
  drawOctaveGuides(context, frame, bandWidth, heightPx, tokens);
  const gap = Math.min(1, bandWidth * 0.16);
  context.fillStyle = tokens.spectrum;
  for (const [index, band] of frame.bands.entries()) {
    const y = magnitudeToY(band.magnitude, heightPx);
    context.fillRect(
      index * bandWidth + gap / 2,
      y,
      Math.max(0.5, bandWidth - gap),
      heightPx - y,
    );
  }
}

function magnitudeToY(magnitude: number, heightPx: number): number {
  const floorMagnitude = 10 ** (MIN_DECIBELS / 20);
  const decibels = 20 * Math.log10(Math.max(floorMagnitude, magnitude));
  const clamped = Math.max(MIN_DECIBELS, Math.min(0, decibels));
  return (1 - (clamped - MIN_DECIBELS) / -MIN_DECIBELS) * heightPx;
}

function drawOctaveGuides(
  context: MusicalSpectrumCanvas2D,
  frame: MusicalSpectrumFramePayload,
  bandWidth: number,
  heightPx: number,
  tokens: MusicalSpectrumTokens,
): void {
  context.strokeStyle = tokens.grid;
  context.fillStyle = tokens.label;
  context.lineWidth = 1;
  context.font = "9px system-ui, sans-serif";
  for (const [index, band] of frame.bands.entries()) {
    if (band.note_number % 12 !== 0) continue;
    const x = index * bandWidth;
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, heightPx);
    context.stroke();
    if (bandWidth >= 4)
      context.fillText(noteLabel(band.note_number), x + 3, heightPx - 5);
  }
}
