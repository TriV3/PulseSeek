import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  onMusicalSpectrumFrame,
  type MusicalSpectrumFramePayload,
} from "../../api/playbackEvents";
import { MusicalSpectrumCanvas } from "./MusicalSpectrumCanvas";
import { drawMusicalSpectrum } from "./musicalSpectrumRenderer";

vi.mock("../../api/playbackEvents", () => ({
  onMusicalSpectrumFrame: vi.fn(),
}));
vi.mock("../Waveform/WaveformCanvas", () => ({
  WaveformCanvas: () => <div data-testid="musical-seek-overlay" />,
}));
vi.mock("./musicalSpectrumRenderer", async () => {
  const actual = await vi.importActual<
    typeof import("./musicalSpectrumRenderer")
  >("./musicalSpectrumRenderer");
  return { ...actual, drawMusicalSpectrum: vi.fn() };
});

let spectrumHandler:
  ((payload: MusicalSpectrumFramePayload) => void) | undefined;
let resize: ((width: number, height: number) => void) | undefined;
let animationCallbacks: FrameRequestCallback[];

const FIRST: MusicalSpectrumFramePayload = {
  format_version: 1,
  sequence: 1,
  position_frames: 0,
  sample_rate: 48_000,
  tuning_reference_hz: 440,
  bands: [
    {
      note_number: 69,
      lower_frequency_hz: 427.47,
      center_frequency_hz: 440,
      upper_frequency_hz: 452.89,
      magnitude: 0.8,
    },
  ],
};

beforeEach(() => {
  spectrumHandler = undefined;
  resize = undefined;
  animationCallbacks = [];
  vi.mocked(onMusicalSpectrumFrame).mockImplementation(async (handler) => {
    spectrumHandler = handler;
    return () => undefined;
  });
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
    animationCallbacks.push(callback);
    return animationCallbacks.length;
  });
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    clearRect: vi.fn(),
  } as unknown as CanvasRenderingContext2D);
  class Observer {
    constructor(private readonly callback: ResizeObserverCallback) {}
    observe() {
      resize = (width, height) =>
        this.callback(
          [{ contentRect: { width, height } } as ResizeObserverEntry],
          this as unknown as ResizeObserver,
        );
    }
    disconnect() {}
    unobserve() {}
  }
  window.ResizeObserver = Observer as unknown as typeof ResizeObserver;
  vi.mocked(drawMusicalSpectrum).mockClear();
});

describe("MusicalSpectrumCanvas", () => {
  it("does not subscribe, draw, or expose seek while disabled", () => {
    render(<MusicalSpectrumCanvas enabled={false} theme="dark" />);

    expect(
      screen.getByRole("img", { name: "Musical spectrum disabled" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("musical-seek-overlay"),
    ).not.toBeInTheDocument();
    expect(onMusicalSpectrumFrame).not.toHaveBeenCalled();
    expect(drawMusicalSpectrum).not.toHaveBeenCalled();
  });

  it("draws accepted musical frames immediately and exposes seek", async () => {
    const { container } = render(
      <MusicalSpectrumCanvas enabled theme="dark" durationMs={2_000} />,
    );
    await act(async () => undefined);
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;

    expect(screen.getByTestId("musical-seek-overlay")).toBeInTheDocument();
    act(() => resize?.(240, 120));
    expect(canvas.width).toBe(240);
    expect(canvas.height).toBe(120);
    act(() => animationCallbacks.shift()?.(0));
    vi.mocked(drawMusicalSpectrum).mockClear();

    act(() => spectrumHandler?.(FIRST));

    expect(drawMusicalSpectrum).toHaveBeenCalledOnce();
    expect(animationCallbacks).toHaveLength(0);
  });

  it("repaints the current frame when the semantic theme changes", async () => {
    const { rerender } = render(<MusicalSpectrumCanvas enabled theme="dark" />);
    await act(async () => undefined);
    act(() => resize?.(240, 120));
    act(() => spectrumHandler?.(FIRST));
    vi.mocked(drawMusicalSpectrum).mockClear();

    rerender(<MusicalSpectrumCanvas enabled theme="midnight" />);
    act(() => animationCallbacks.shift()?.(0));

    expect(drawMusicalSpectrum).toHaveBeenCalledOnce();
    expect(vi.mocked(drawMusicalSpectrum).mock.calls[0]?.[1]).toEqual(FIRST);
  });
});
