import { describe, expect, it, vi } from "vitest";
import {
  drawMusicalSpectrum,
  noteLabel,
  type MusicalSpectrumCanvas2D,
} from "./musicalSpectrumRenderer";

const frame = {
  format_version: 1 as const,
  sequence: 1,
  position_frames: 0,
  sample_rate: 48_000,
  tuning_reference_hz: 440,
  bands: [
    {
      note_number: 68,
      lower_frequency_hz: 403.48,
      center_frequency_hz: 415.3,
      upper_frequency_hz: 427.47,
      magnitude: 0.1,
    },
    {
      note_number: 69,
      lower_frequency_hz: 427.47,
      center_frequency_hz: 440,
      upper_frequency_hz: 452.89,
      magnitude: 1,
    },
  ],
};

describe("musical spectrum renderer", () => {
  it("labels equal-tempered note numbers with scientific pitch notation", () => {
    expect(noteLabel(60)).toBe("C4");
    expect(noteLabel(61)).toBe("C♯4");
    expect(noteLabel(69)).toBe("A4");
    expect(noteLabel(72)).toBe("C5");
  });

  it("draws one energy bar per musical band with semantic tokens", () => {
    const context = canvasContext();
    const tokens = {
      spectrum: "var(--analyzer-spectrum)",
      spectrumSoft: "var(--analyzer-spectrum-soft)",
      grid: "var(--analyzer-grid)",
      label: "var(--analyzer-label)",
    };

    drawMusicalSpectrum(context, frame, 100, 50, tokens);

    expect(context.fillRect).toHaveBeenCalledTimes(2);
    expect(context.fillRect).toHaveBeenLastCalledWith(
      expect.any(Number),
      0,
      expect.any(Number),
      50,
    );
    expect(context.fillStyle).toBe(tokens.spectrum);

    vi.clearAllMocks();
    drawMusicalSpectrum(context, frame, 0, 50, tokens);
    expect(context.clearRect).not.toHaveBeenCalled();
  });
});

function canvasContext(): MusicalSpectrumCanvas2D {
  return {
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
    fillText: vi.fn(),
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 0,
    font: "",
  };
}
