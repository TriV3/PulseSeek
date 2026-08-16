import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  onPosition,
  onSpectrumFrame,
  type SpectrumFramePayload,
} from "../../api/playbackEvents";
import { LogAnalyzerCanvas } from "./LogAnalyzerCanvas";
import { drawLogAnalyzer } from "./logAnalyzerRenderer";

vi.mock("../../api/playbackEvents", () => ({
  onPosition: vi.fn(),
  onSpectrumFrame: vi.fn(),
}));
vi.mock("../Waveform/WaveformCanvas", () => ({
  WaveformCanvas: () => null,
}));
vi.mock("./logAnalyzerRenderer", async () => {
  const actual = await vi.importActual<typeof import("./logAnalyzerRenderer")>(
    "./logAnalyzerRenderer",
  );
  return { ...actual, drawLogAnalyzer: vi.fn() };
});

let spectrumHandler: ((payload: SpectrumFramePayload) => void) | undefined;
let resize: ((width: number, height: number) => void) | undefined;
let animationCallbacks: FrameRequestCallback[];

const FIRST: SpectrumFramePayload = {
  format_version: 1,
  sequence: 1,
  position_frames: 0,
  sample_rate: 48_000,
  fft_size: 8,
  magnitudes: [0, 0.1, 0.8, 0.2, 0],
};

beforeEach(() => {
  spectrumHandler = undefined;
  resize = undefined;
  animationCallbacks = [];
  vi.mocked(onSpectrumFrame).mockImplementation(async (handler) => {
    spectrumHandler = handler;
    return () => undefined;
  });
  vi.mocked(onPosition).mockResolvedValue(() => undefined);
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
  vi.mocked(drawLogAnalyzer).mockClear();
});

describe("LogAnalyzerCanvas", () => {
  it("does not subscribe or draw while disabled", () => {
    render(<LogAnalyzerCanvas enabled={false} theme="dark" />);

    expect(
      screen.getByRole("img", {
        name: "Logarithmic frequency analyzer disabled",
      }),
    ).toBeInTheDocument();
    expect(onSpectrumFrame).not.toHaveBeenCalled();
    expect(drawLogAnalyzer).not.toHaveBeenCalled();
  });

  it("draws spectrum events without waiting for an animation frame", async () => {
    const { container } = render(<LogAnalyzerCanvas enabled theme="dark" />);
    await act(async () => undefined);
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;

    act(() => resize?.(240, 120));
    expect(canvas.width).toBe(240);
    expect(canvas.height).toBe(120);
    act(() => animationCallbacks.shift()?.(0));
    vi.mocked(drawLogAnalyzer).mockClear();

    act(() => spectrumHandler?.(FIRST));

    expect(drawLogAnalyzer).toHaveBeenCalledOnce();
    expect(animationCallbacks).toHaveLength(0);
  });

  it("repaints the current frame when the semantic theme changes", async () => {
    const { rerender } = render(<LogAnalyzerCanvas enabled theme="dark" />);
    await act(async () => undefined);
    act(() => resize?.(240, 120));
    act(() => spectrumHandler?.(FIRST));
    act(() => animationCallbacks.shift()?.(0));
    vi.mocked(drawLogAnalyzer).mockClear();

    rerender(<LogAnalyzerCanvas enabled theme="midnight" />);
    act(() => animationCallbacks.shift()?.(0));

    expect(drawLogAnalyzer).toHaveBeenCalledOnce();
    expect(vi.mocked(drawLogAnalyzer).mock.calls[0]?.[1]).toEqual(FIRST);
  });

  it("draws successive audio frames instead of freezing the first spectrum", async () => {
    render(<LogAnalyzerCanvas enabled theme="dark" />);
    await act(async () => undefined);
    act(() => resize?.(240, 120));

    act(() => spectrumHandler?.(FIRST));
    act(() =>
      spectrumHandler?.({
        ...FIRST,
        sequence: 2,
        magnitudes: [0, 0.8, 0.15, 0.6, 0],
      }),
    );
    expect(drawLogAnalyzer).toHaveBeenCalledTimes(2);
    expect(vi.mocked(drawLogAnalyzer).mock.calls[0]?.[1]).toMatchObject({
      sequence: 1,
    });
    expect(vi.mocked(drawLogAnalyzer).mock.calls[1]?.[1]).toMatchObject({
      sequence: 2,
      magnitudes: [0, 0.8, 0.15, 0.6, 0],
    });
  });
});
