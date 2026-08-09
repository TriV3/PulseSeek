import { describe, expect, it, vi } from "vitest";
import {
  drawLinearAnalyzer,
  frequencyToLinearX,
  magnitudeToY,
  type LinearAnalyzerCanvas2D,
} from "./linearAnalyzerRenderer";

describe("linear analyzer renderer", () => {
  it("maps FFT frequencies linearly from DC to Nyquist", () => {
    expect(frequencyToLinearX(0, 48_000, 100)).toBeCloseTo(0);
    expect(frequencyToLinearX(6_000, 48_000, 100)).toBeCloseTo(25);
    expect(frequencyToLinearX(12_000, 48_000, 100)).toBeCloseTo(50);
    expect(frequencyToLinearX(24_000, 48_000, 100)).toBeCloseTo(100);
  });

  it("maps magnitudes to a bounded decibel axis", () => {
    expect(magnitudeToY(1, 80)).toBeCloseTo(0);
    expect(magnitudeToY(0, 80)).toBeCloseTo(80);
    expect(magnitudeToY(10 ** (-45 / 20), 80)).toBeCloseTo(40);
  });

  it("draws every FFT bin at its linear position with semantic tokens", () => {
    const context = canvasContext();
    const tokens = {
      spectrum: "var(--analyzer-spectrum)",
      spectrumSoft: "var(--analyzer-spectrum-soft)",
      grid: "var(--analyzer-grid)",
      label: "var(--analyzer-label)",
    };
    const frame = {
      format_version: 1 as const,
      sequence: 1,
      position_frames: 0,
      sample_rate: 48_000,
      fft_size: 8,
      magnitudes: [0, 0.1, 0.8, 0.2, 0],
    };

    drawLinearAnalyzer(context, frame, 100, 50, tokens);

    expect(context.lineTo).toHaveBeenCalledWith(25, expect.any(Number));
    expect(context.lineTo).toHaveBeenCalledWith(50, expect.any(Number));
    expect(context.lineTo).toHaveBeenCalledWith(75, expect.any(Number));
    expect(context.lineTo).toHaveBeenCalledWith(100, expect.any(Number));
    expect(context.strokeStyle).toBe(tokens.spectrum);
    expect(context.fillStyle).toBe(tokens.spectrumSoft);

    vi.clearAllMocks();
    drawLinearAnalyzer(context, frame, 0, 50, tokens);
    expect(context.clearRect).not.toHaveBeenCalled();
  });
});

function canvasContext(): LinearAnalyzerCanvas2D {
  return {
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    stroke: vi.fn(),
    fill: vi.fn(),
    fillText: vi.fn(),
    strokeStyle: "",
    fillStyle: "",
    lineWidth: 0,
    font: "",
  };
}
