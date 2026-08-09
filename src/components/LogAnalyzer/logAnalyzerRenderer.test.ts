import { describe, expect, it, vi } from "vitest";
import {
  drawLogAnalyzer,
  frequencyToX,
  magnitudeToY,
  type AnalyzerCanvas2D,
} from "./logAnalyzerRenderer";

describe("logarithmic analyzer renderer", () => {
  it("maps frequencies logarithmically from 10 Hz to Nyquist", () => {
    expect(frequencyToX(10, 48_000, 100)).toBeCloseTo(0);
    expect(frequencyToX(Math.sqrt(10 * 24_000), 48_000, 100)).toBeCloseTo(50);
    expect(frequencyToX(24_000, 48_000, 100)).toBeCloseTo(100);
  });

  it("maps magnitudes to a bounded decibel axis", () => {
    expect(magnitudeToY(1, 80)).toBeCloseTo(0);
    expect(magnitudeToY(0, 80)).toBeCloseTo(80);
    expect(magnitudeToY(10 ** (-45 / 20), 80)).toBeCloseTo(40);
  });

  it("draws with semantic tokens and ignores zero-sized surfaces", () => {
    const quadraticCurveTo = vi.fn();
    const context = {
      ...canvasContext(),
      quadraticCurveTo,
    } as AnalyzerCanvas2D;
    const tokens = {
      spectrum: "rgb(1, 2, 3)",
      spectrumSoft: "rgba(1, 2, 3, 0.2)",
      grid: "rgb(4, 5, 6)",
      label: "rgb(7, 8, 9)",
    };
    const frame = {
      format_version: 1 as const,
      sequence: 1,
      position_frames: 0,
      sample_rate: 48_000,
      fft_size: 8,
      magnitudes: [0, 0.1, 0.8, 0.2, 0],
    };

    drawLogAnalyzer(context, frame, 100, 50, tokens);

    expect(context.strokeStyle).toBe(tokens.spectrum);
    expect(context.fillStyle).toBe(tokens.spectrumSoft);
    expect(context.stroke).toHaveBeenCalled();
    expect(context.fill).toHaveBeenCalled();
    expect(quadraticCurveTo).toHaveBeenCalled();

    const fillOrder = vi.mocked(context.fill).mock.invocationCallOrder[0];
    const strokeOrder = vi
      .mocked(context.stroke)
      .mock.invocationCallOrder.at(-1);
    expect(fillOrder).toBeDefined();
    expect(strokeOrder).toBeDefined();
    expect(
      vi
        .mocked(context.beginPath)
        .mock.invocationCallOrder.some(
          (order) => order > (fillOrder ?? 0) && order < (strokeOrder ?? 0),
        ),
    ).toBe(true);

    vi.clearAllMocks();
    drawLogAnalyzer(context, frame, 0, 50, tokens);
    expect(context.clearRect).not.toHaveBeenCalled();
  });
});

function canvasContext(): AnalyzerCanvas2D {
  return {
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    quadraticCurveTo: vi.fn(),
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
