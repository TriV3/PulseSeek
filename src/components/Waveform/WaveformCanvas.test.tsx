import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { WaveformCanvas } from "./WaveformCanvas";
import { onPosition } from "../../api/playbackEvents";
import type { WaveformLevel } from "../../api/waveform";
import type { Canvas2D } from "./waveformRenderer";

vi.mock("../../api/playbackEvents", () => ({
  onPosition: vi.fn(),
}));

const LEVEL: WaveformLevel = {
  format_version: 1,
  channels: 1,
  samples_per_peak: 10,
  min: [-0.5, 0, 0.5],
  max: [-0.4, 0.1, 0.6],
};

const LEVEL_STEREO: WaveformLevel = {
  format_version: 1,
  channels: 2,
  samples_per_peak: 10,
  min: [-0.5, 0, 0.5, -0.3, 0.2, 0.4],
  max: [-0.4, 0.1, 0.6, -0.2, 0.3, 0.5],
};

function createMockContext() {
  const moves: Array<[number, number]> = [];
  const state = { strokes: 0, dashes: [] as Array<string> };
  const ctx: Canvas2D & Record<string, unknown> = {
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn((x: number, y: number) => {
      moves.push([x, y]);
    }),
    lineTo: vi.fn(),
    stroke: vi.fn(() => {
      state.strokes += 1;
    }),
    fill: vi.fn(),
    closePath: vi.fn(),
    setTransform: vi.fn(),
    setLineDash: vi.fn((segments: number[]) => {
      state.dashes.push(segments.join(","));
    }),
    createLinearGradient: vi.fn(() => ({
      addColorStop: vi.fn(),
    })),
    strokeStyle: "#000",
    fillStyle: "#000",
    lineWidth: 1,
    lineCap: "butt",
    lineJoin: "miter",
  };
  return { ctx, moves, state };
}

let mockContext: ReturnType<typeof createMockContext>;
let observerInstance: {
  trigger: (width: number, height: number) => void;
} | null;
let positionHandler:
  | ((payload: { position_ms: number; duration_ms: number | null }) => void)
  | undefined;
let rafCallbacks: Array<FrameRequestCallback> = [];

/** Runs every pending animation frame immediately, in order. */
function flushRaf() {
  const pending = rafCallbacks;
  rafCallbacks = [];
  for (const callback of pending) callback(0);
}

class MockResizeObserver {
  private readonly callback: ResizeObserverCallback;
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    observerInstance = {
      trigger: (width: number, height: number) => {
        this.fire(width, height);
      },
    };
  }
  private fire(width: number, height: number) {
    this.callback(
      [{ contentRect: { width, height } } as ResizeObserverEntry],
      this as unknown as ResizeObserver,
    );
  }
  // Real observers fire immediately on observe with the initial size.
  observe = vi.fn(() => this.fire(100, 40));
  disconnect = vi.fn();
}

beforeEach(() => {
  mockContext = createMockContext();
  observerInstance = null;
  positionHandler = undefined;
  rafCallbacks = [];

  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
    mockContext.ctx as unknown as CanvasRenderingContext2D,
  );
  window.ResizeObserver =
    MockResizeObserver as unknown as typeof ResizeObserver;
  window.requestAnimationFrame = ((callback: FrameRequestCallback) => {
    rafCallbacks.push(callback);
    return rafCallbacks.length;
  }) as typeof window.requestAnimationFrame;
  vi.mocked(onPosition).mockImplementation((handler) => {
    positionHandler = handler;
    return Promise.resolve(() => {});
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  // @ts-expect-error restoring test-only stub on jsdom
  delete window.ResizeObserver;
});

function emitPosition(positionMs: number, durationMs: number | null = 2000) {
  positionHandler?.({ position_ms: positionMs, duration_ms: durationMs });
}

describe("WaveformCanvas", () => {
  it("draws the envelope once waveform data arrives", () => {
    render(<WaveformCanvas waveform={LEVEL} durationMs={2000} />);

    expect(mockContext.ctx.clearRect).toHaveBeenCalled();
    // Grid line, min edge, and max edge.
    expect(mockContext.state.strokes).toBeGreaterThanOrEqual(3);
  });

  it("clears the canvas while no waveform is available", () => {
    render(<WaveformCanvas waveform={null} durationMs={null} />);

    expect(mockContext.ctx.clearRect).toHaveBeenCalled();
    expect(mockContext.state.strokes).toBe(0);
  });

  it("draws the playhead from position events without re-rendering", () => {
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    flushRaf();
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    const strokesBefore = mockContext.state.strokes;

    emitPosition(500);
    flushRaf();
    emitPosition(1000);
    flushRaf();
    emitPosition(1500);
    flushRaf();
    expect(mockContext.state.strokes).toBeGreaterThan(strokesBefore);
    expect(mockContext.state.dashes).toContain("4,3");

    // 1000 / 2000 ms maps to the middle of a 100px-wide canvas.
    const playheadMoves = mockContext.moves.filter(([x]) => x === 50);
    expect(playheadMoves.length).toBeGreaterThan(0);

    // High-frequency position updates never re-render React: the canvas node
    // is stable and the slider value is written imperatively instead.
    expect(container.querySelector("canvas")).toBe(canvas);
    expect(canvas.getAttribute("aria-valuenow")).toBe("1500");
  });

  it("does not draw a playhead without a known duration", () => {
    render(<WaveformCanvas waveform={LEVEL} durationMs={null} />);
    flushRaf();
    emitPosition(1000, null);
    flushRaf();

    expect(mockContext.state.dashes).not.toContain("4,3");
  });

  it("requests a coarser level after a debounced resize", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const onRequestRefetch = vi.fn();
    render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        onRequestRefetch={onRequestRefetch}
      />,
    );
    observerInstance?.trigger(300, 100);

    expect(onRequestRefetch).not.toHaveBeenCalled();
    vi.advanceTimersByTime(199);
    expect(onRequestRefetch).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onRequestRefetch).toHaveBeenCalledWith(600);
  });

  it("coalesces rapid resizes into one refetch", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const onRequestRefetch = vi.fn();
    render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        onRequestRefetch={onRequestRefetch}
      />,
    );
    observerInstance?.trigger(300, 100);
    observerInstance?.trigger(400, 100);
    observerInstance?.trigger(500, 100);
    vi.advanceTimersByTime(200);

    expect(onRequestRefetch).toHaveBeenCalledTimes(1);
    expect(onRequestRefetch).toHaveBeenCalledWith(1000);
  });

  it("redraws after a resize", () => {
    render(<WaveformCanvas waveform={LEVEL} durationMs={2000} />);
    const before = mockContext.state.strokes;
    observerInstance?.trigger(400, 100);
    expect(mockContext.state.strokes).toBeGreaterThan(before);
  });

  it("ignores non-finite observer measurements", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const onRequestRefetch = vi.fn();
    render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        onRequestRefetch={onRequestRefetch}
      />,
    );
    // The initial observer fire schedules a legitimate refetch; clear it so
    // only the non-finite measurement below is observed.
    vi.advanceTimersByTime(200);
    onRequestRefetch.mockClear();
    const before = mockContext.state.strokes;

    observerInstance?.trigger(Number.NaN, 100);
    vi.advanceTimersByTime(500);

    expect(onRequestRefetch).not.toHaveBeenCalled();
    expect(mockContext.state.strokes).toBe(before);
  });

  it("drops a previous file's playhead when the waveform changes", () => {
    const { rerender } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    flushRaf();
    emitPosition(500);
    flushRaf();
    expect(mockContext.state.dashes).toContain("4,3");

    mockContext.state.dashes.length = 0;
    rerender(<WaveformCanvas waveform={LEVEL_STEREO} durationMs={2000} />);
    flushRaf();

    // The stale position must not paint a playhead on the new waveform.
    expect(mockContext.state.dashes).not.toContain("4,3");
  });

  it("keeps the playhead when only the duration updates", () => {
    const { container, rerender } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={null} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    flushRaf();
    emitPosition(500, 2000);
    flushRaf();
    rerender(<WaveformCanvas waveform={LEVEL} durationMs={2000} />);
    flushRaf();

    // A duration change from the same file must not wipe the position.
    expect(canvas.getAttribute("aria-valuenow")).toBe("500");
  });

  it("seeks to the clicked position", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 50, pointerId: 1 });
    fireEvent.pointerUp(canvas, { pointerId: 1 });

    expect(onSeek).toHaveBeenCalledWith(1000);
  });

  it("seeks continuously while dragging", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 25, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 75, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 10, pointerId: 1 });
    fireEvent.pointerUp(canvas, { pointerId: 1 });

    expect(onSeek).toHaveBeenNthCalledWith(1, 500);
    expect(onSeek).toHaveBeenNthCalledWith(2, 1500);
    expect(onSeek).toHaveBeenNthCalledWith(3, 200);
  });

  it("keeps the drag preview stable while position events arrive", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 25, pointerId: 1 });
    expect(onSeek).toHaveBeenLastCalledWith(500);

    // A throttled position event during the drag must not move the preview.
    emitPosition(1200);
    flushRaf();
    expect(canvas.getAttribute("aria-valuenow")).toBe("500");

    // After the drag, the next confirmed position reconciles the playhead.
    fireEvent.pointerUp(canvas, { pointerId: 1 });
    emitPosition(1200);
    flushRaf();
    expect(canvas.getAttribute("aria-valuenow")).toBe("1200");
  });

  it("clamps pointer seeks to the duration bounds", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 500, pointerId: 1 });
    fireEvent.pointerUp(canvas, { pointerId: 1 });
    fireEvent.pointerDown(canvas, { clientX: -50, pointerId: 2 });
    fireEvent.pointerUp(canvas, { pointerId: 2 });

    expect(onSeek).toHaveBeenNthCalledWith(1, 2000);
    expect(onSeek).toHaveBeenNthCalledWith(2, 0);
  });

  it("does not seek without a known duration", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={null} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 50, pointerId: 1 });
    fireEvent.keyDown(canvas, { key: "ArrowRight" });

    expect(onSeek).not.toHaveBeenCalled();
    expect(canvas.getAttribute("aria-disabled")).toBe("true");
    expect(canvas.tabIndex).toBe(-1);
  });

  it("seeks from the keyboard", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    flushRaf();
    emitPosition(1000);
    flushRaf();

    fireEvent.keyDown(canvas, { key: "ArrowRight" });
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    fireEvent.keyDown(canvas, { key: "Home" });
    fireEvent.keyDown(canvas, { key: "End" });

    expect(onSeek).toHaveBeenNthCalledWith(1, 2000);
    expect(onSeek).toHaveBeenNthCalledWith(2, 0);
    expect(onSeek).toHaveBeenNthCalledWith(3, 0);
    expect(onSeek).toHaveBeenNthCalledWith(4, 2000);
  });

  it("redraws with the selected style without refetching data", () => {
    const onRequestRefetch = vi.fn();
    const { rerender } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        onRequestRefetch={onRequestRefetch}
        style="outline"
      />,
    );
    flushRaf();
    const outlineStrokes = mockContext.state.strokes;

    rerender(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        onRequestRefetch={onRequestRefetch}
        style="solid"
      />,
    );
    flushRaf();

    // The solid style fills instead of stroking the envelope.
    expect(mockContext.ctx.fill).toHaveBeenCalled();
    expect(onRequestRefetch).not.toHaveBeenCalled();
    expect(outlineStrokes).toBeGreaterThan(0);
  });
});

function stubCanvasRect(canvas: HTMLCanvasElement, width: number) {
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: width,
    bottom: 40,
    width,
    height: 40,
    toJSON: () => ({}),
  } as DOMRect);
}
