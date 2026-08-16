import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  onPosition,
  onSpectrumFrame,
  type SpectrumFramePayload,
} from "../../api/playbackEvents";
import { LinearAnalyzerCanvas } from "./LinearAnalyzerCanvas";
import { drawLinearAnalyzer } from "./linearAnalyzerRenderer";

vi.mock("../../api/playbackEvents", () => ({
  onPosition: vi.fn(),
  onSpectrumFrame: vi.fn(),
}));
vi.mock("../Waveform/WaveformCanvas", () => ({
  WaveformCanvas: () => <div data-testid="linear-seek-overlay" />,
}));
vi.mock("./linearAnalyzerRenderer", async () => {
  const actual = await vi.importActual<
    typeof import("./linearAnalyzerRenderer")
  >("./linearAnalyzerRenderer");
  return { ...actual, drawLinearAnalyzer: vi.fn() };
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
  vi.mocked(drawLinearAnalyzer).mockClear();
});

describe("LinearAnalyzerCanvas", () => {
  it("does not subscribe, draw, or expose seek while disabled", () => {
    render(<LinearAnalyzerCanvas enabled={false} theme="dark" />);

    expect(
      screen.getByRole("img", { name: "Linear frequency analyzer disabled" }),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("linear-seek-overlay")).not.toBeInTheDocument();
    expect(onSpectrumFrame).not.toHaveBeenCalled();
    expect(drawLinearAnalyzer).not.toHaveBeenCalled();
  });

  it("draws spectrum events without waiting for an animation frame", async () => {
    const { container } = render(
      <LinearAnalyzerCanvas enabled theme="dark" durationMs={2_000} />,
    );
    await act(async () => undefined);
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;

    expect(screen.getByTestId("linear-seek-overlay")).toBeInTheDocument();
    act(() => resize?.(240, 120));
    expect(canvas.width).toBe(240);
    expect(canvas.height).toBe(120);
    act(() => animationCallbacks.shift()?.(0));
    vi.mocked(drawLinearAnalyzer).mockClear();

    act(() => spectrumHandler?.(FIRST));

    expect(drawLinearAnalyzer).toHaveBeenCalledOnce();
    expect(animationCallbacks).toHaveLength(0);
  });

  it("repaints the current frame when the semantic theme changes", async () => {
    const { rerender } = render(<LinearAnalyzerCanvas enabled theme="dark" />);
    await act(async () => undefined);
    act(() => resize?.(240, 120));
    act(() => spectrumHandler?.(FIRST));
    vi.mocked(drawLinearAnalyzer).mockClear();

    rerender(<LinearAnalyzerCanvas enabled theme="midnight" />);
    act(() => animationCallbacks.shift()?.(0));

    expect(drawLinearAnalyzer).toHaveBeenCalledOnce();
    expect(vi.mocked(drawLinearAnalyzer).mock.calls[0]?.[1]).toEqual(FIRST);
  });
});
