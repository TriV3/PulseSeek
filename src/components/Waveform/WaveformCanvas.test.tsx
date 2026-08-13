import { createRef } from "react";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { WaveformCanvas, type WaveformCanvasHandle } from "./WaveformCanvas";
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
  it("exposes zoom controls and zooms around pointer", () => {
    const { container, getByRole } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2_000} showZoomControls />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    expect(
      getByRole("button", { name: "Zoom in waveform" }),
    ).toBeInTheDocument();
    fireEvent.wheel(canvas, { clientX: 25, deltaY: -100 });

    expect(
      getByRole("button", { name: "Reset waveform zoom" }),
    ).not.toBeDisabled();
    expect(canvas).toHaveAttribute("aria-label", "Waveform seek");
  });

  it("pans zoomed waveform by dragging without seeking", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2_000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    fireEvent.wheel(canvas, { clientX: 50, deltaY: -100 });
    fireEvent.pointerDown(canvas, { clientX: 50, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 60, pointerId: 1 });
    fireEvent.pointerUp(canvas, { clientX: 60, pointerId: 1 });

    expect(onSeek).not.toHaveBeenCalled();
  });

  it("maps A/B marker drag preview in zoomed coordinates", () => {
    const onSetAbPoint = vi.fn();
    const { container, getByRole } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2_000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        onSetAbPoint={onSetAbPoint}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    fireEvent.wheel(canvas, { clientX: 50, deltaY: -100 });
    const marker = getByRole("slider", { name: "B point" });
    fireEvent.pointerDown(marker, { clientX: 75, pointerId: 2 });
    fireEvent.pointerMove(marker, { clientX: 100, pointerId: 2 });

    expect(marker).toHaveStyle("--ab-x: 100");
  });

  it("uses damped pinch zoom", () => {
    const onViewportChange = vi.fn();
    const { container } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2_000}
        onViewportChange={onViewportChange}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    fireEvent.pointerDown(canvas, { clientX: 25, pointerId: 1 });
    fireEvent.pointerDown(canvas, { clientX: 75, pointerId: 2 });
    fireEvent.pointerMove(canvas, { clientX: 0, pointerId: 1 });

    const viewport = onViewportChange.mock.lastCall?.[0];
    expect(viewport.endMs - viewport.startMs).toBeGreaterThan(1_000);
  });

  it("keeps visible part of A-B highlight after zoom", () => {
    const { container, getByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2_000}
        abPoints={{ startMs: 900, endMs: 1_100 }}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    fireEvent.wheel(canvas, { clientX: 50, deltaY: -100 });

    expect(getByTestId("waveform-ab-band")).toHaveStyle("--ab-width: 12.5");
  });

  it("draws the envelope once waveform data arrives", () => {
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );

    expect(mockContext.ctx.clearRect).toHaveBeenCalled();
    // Grid line, min edge, and max edge.
    expect(mockContext.state.strokes).toBeGreaterThanOrEqual(3);
    expect(container.querySelector("canvas")).toHaveClass(
      "waveform-canvas-surface--revealing",
    );
  });

  it("clears the canvas while no waveform is available", () => {
    const { container } = render(
      <WaveformCanvas waveform={null} durationMs={null} />,
    );

    expect(mockContext.ctx.clearRect).toHaveBeenCalled();
    expect(mockContext.state.strokes).toBe(0);
    expect(container.querySelector("canvas")).not.toHaveClass(
      "waveform-canvas-surface--revealing",
    );
  });

  it("draws the playhead from position events without re-rendering", () => {
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    flushRaf();
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    const strokesBefore = mockContext.state.strokes;

    emitPosition(500);
    emitPosition(1000);
    emitPosition(1500);
    expect(mockContext.state.strokes).toBe(strokesBefore);
    expect(
      container.querySelector("[data-testid='waveform-current-marker']"),
    ).toHaveStyle("--seek-x: 75px");

    // The progress marker is composited above the static canvas; 1500 / 2000
    // ms maps to 75px of the 100px waveform.
    expect(
      container.querySelector("[data-testid='waveform-current-marker']"),
    ).toHaveStyle("--seek-x: 75px");

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
    const { container, rerender } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    flushRaf();
    emitPosition(500);
    flushRaf();
    expect(
      container.querySelector("[data-testid='waveform-current-marker']"),
    ).toHaveStyle("--seek-x: 25px");

    mockContext.state.dashes.length = 0;
    rerender(<WaveformCanvas waveform={LEVEL_STEREO} durationMs={2000} />);
    flushRaf();

    // The stale position must not paint a playhead on the new waveform.
    expect(
      container.querySelector("[data-testid='waveform-current-marker']"),
    ).not.toBeVisible();
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

  it("shows the current time next to the playhead", () => {
    const { getByTestId } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );

    emitPosition(1250);
    flushRaf();

    expect(getByTestId("waveform-current-time")).toHaveTextContent("0:01");
    expect(getByTestId("waveform-current-time")).toHaveStyle(
      "--seek-x: 62.5px",
    );
  });

  it("shows the hovered time next to the pointer bar", () => {
    const { container, getByTestId } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerMove(canvas, { clientX: 75, pointerId: 1 });
    flushRaf();

    expect(getByTestId("waveform-hover-time")).toHaveTextContent("0:01");
    expect(getByTestId("waveform-hover-time")).toHaveStyle("--seek-x: 75px");
    expect(getByTestId("waveform-hover-marker")).toHaveStyle("--seek-x: 75px");

    fireEvent.pointerLeave(canvas);
    expect(getByTestId("waveform-hover-time")).not.toBeVisible();
  });

  it("renders pointer updates without waiting for an animation frame", () => {
    const { container, getByTestId } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    flushRaf();

    fireEvent.pointerMove(canvas, { clientX: 100, pointerId: 1 });

    expect(rafCallbacks).toHaveLength(0);
    expect(getByTestId("waveform-hover-marker")).toHaveStyle("--seek-x: 100px");
  });

  it("coalesces fast pointer events to the latest position without using animation frames", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const { container, getByTestId } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    flushRaf();

    fireEvent.pointerMove(canvas, { clientX: 10, pointerId: 1 });
    for (let clientX = 11; clientX <= 100; clientX += 1) {
      fireEvent.pointerMove(canvas, { clientX, pointerId: 1 });
    }

    expect(rafCallbacks).toHaveLength(0);
    expect(getByTestId("waveform-hover-marker")).toHaveStyle("--seek-x: 10px");

    vi.advanceTimersByTime(17);
    expect(getByTestId("waveform-hover-marker")).toHaveStyle("--seek-x: 100px");
  });

  it("keeps time labels fully inside the waveform at both edges", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const { container, getByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        restoredPositionMs={0}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    flushRaf();

    expect(getByTestId("waveform-current-time")).toHaveStyle("--seek-x: 24px");

    fireEvent.pointerMove(canvas, { clientX: 0, pointerId: 1 });
    flushRaf();
    expect(getByTestId("waveform-hover-marker")).toHaveStyle("--seek-x: 0px");
    expect(getByTestId("waveform-hover-time")).toHaveStyle("--seek-x: 24px");

    fireEvent.pointerMove(canvas, { clientX: 100, pointerId: 1 });
    vi.advanceTimersByTime(17);
    expect(getByTestId("waveform-hover-marker")).toHaveStyle("--seek-x: 100px");
    expect(getByTestId("waveform-hover-time")).toHaveStyle("--seek-x: 76px");
  });

  it("shows a restored position before native playback events arrive", () => {
    const { container, getByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        restoredPositionMs={1500}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    flushRaf();

    expect(canvas).toHaveAttribute("aria-valuenow", "1500");
    expect(getByTestId("waveform-current-time")).toHaveTextContent("0:01");
  });

  it("returns the visible playhead and time to zero after Stop", () => {
    const { getByTestId, rerender } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} resetRevision={0} />,
    );
    emitPosition(1500);
    flushRaf();

    rerender(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} resetRevision={1} />,
    );
    flushRaf();

    expect(getByTestId("waveform-current-marker")).toHaveStyle("--seek-x: 0px");
    expect(getByTestId("waveform-current-time")).toHaveTextContent("0:00");
  });

  it("can seek as soon as a position event supplies the duration", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={null} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    emitPosition(250, 2000);
    flushRaf();
    fireEvent.pointerDown(canvas, { clientX: 50, pointerId: 1 });
    fireEvent.pointerUp(canvas, { pointerId: 1 });

    expect(onSeek).toHaveBeenCalledWith(1000);
    expect(canvas.getAttribute("aria-valuemax")).toBe("2000");
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

  it("previews continuously while dragging and seeks once on release", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 25, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 75, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 10, pointerId: 1 });
    vi.advanceTimersByTime(17);

    expect(onSeek).not.toHaveBeenCalled();
    expect(canvas.getAttribute("aria-valuenow")).toBe("200");

    fireEvent.pointerUp(canvas, { pointerId: 1 });

    expect(onSeek).toHaveBeenCalledTimes(1);
    expect(onSeek).toHaveBeenCalledWith(200);
  });

  it("keeps the drag preview stable while position events arrive", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} onSeek={onSeek} />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);

    fireEvent.pointerDown(canvas, { clientX: 25, pointerId: 1 });
    flushRaf();
    expect(onSeek).not.toHaveBeenCalled();
    expect(canvas.getAttribute("aria-valuenow")).toBe("500");

    // A throttled position event during the drag must not move the preview.
    emitPosition(1200);
    flushRaf();
    expect(canvas.getAttribute("aria-valuenow")).toBe("500");

    // After the drag, the next confirmed position reconciles the playhead.
    fireEvent.pointerUp(canvas, { pointerId: 1 });
    expect(onSeek).toHaveBeenCalledTimes(1);
    expect(onSeek).toHaveBeenCalledWith(500);
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

  it("does not render A-B markers without points", () => {
    const { queryByTestId } = render(
      <WaveformCanvas waveform={LEVEL} durationMs={2000} />,
    );

    expect(queryByTestId("waveform-ab-start")).not.toBeInTheDocument();
    expect(queryByTestId("waveform-ab-end")).not.toBeInTheDocument();
    expect(queryByTestId("waveform-ab-band")).not.toBeInTheDocument();
  });

  it("renders a single pending A marker before the region is confirmed", () => {
    const { getByTestId, queryByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: null }}
      />,
    );

    expect(getByTestId("waveform-ab-start")).toHaveClass(
      "waveform-ab-marker--pending",
    );
    expect(getByTestId("waveform-ab-start")).toHaveStyle("--ab-x: 50");
    expect(queryByTestId("waveform-ab-end")).not.toBeInTheDocument();
    expect(queryByTestId("waveform-ab-band")).not.toBeInTheDocument();
  });

  it("renders pending markers while the second point is pending", () => {
    const { getByTestId, queryByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
      />,
    );

    expect(getByTestId("waveform-ab-start")).toHaveClass(
      "waveform-ab-marker--pending",
    );
    expect(getByTestId("waveform-ab-end")).toHaveClass(
      "waveform-ab-marker--pending",
    );
    expect(getByTestId("waveform-ab-end")).toHaveStyle("--ab-x: 75");
    expect(queryByTestId("waveform-ab-band")).toBeInTheDocument();
  });

  it("renders solid markers and a band only for the confirmed region", () => {
    const { getByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
      />,
    );

    expect(getByTestId("waveform-ab-start")).not.toHaveClass(
      "waveform-ab-marker--pending",
    );
    expect(getByTestId("waveform-ab-end")).not.toHaveClass(
      "waveform-ab-marker--pending",
    );
    const band = getByTestId("waveform-ab-band");
    expect(band).toHaveStyle("--ab-x: 50");
    expect(band).toHaveStyle("--ab-width: 25");
  });

  it("never renders the loop band for a lone unconfirmed point", () => {
    const { queryByTestId: querySolo } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: null }}
        loopRegion={null}
      />,
    );
    expect(querySolo("waveform-ab-band")).not.toBeInTheDocument();
  });

  it("stays pending when points no longer match the confirmed region", () => {
    const { getByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 200, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
      />,
    );

    expect(getByTestId("waveform-ab-start")).toHaveClass(
      "waveform-ab-marker--pending",
    );
    expect(getByTestId("waveform-ab-end")).not.toHaveClass(
      "waveform-ab-marker--pending",
    );
  });

  it("omits markers when the duration is unknown", () => {
    const { queryByTestId } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={null}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
      />,
    );

    expect(queryByTestId("waveform-ab-start")).not.toBeInTheDocument();
    expect(queryByTestId("waveform-ab-band")).not.toBeInTheDocument();
  });

  it("exposes the visual playhead through the imperative handle", () => {
    const ref = createRef<WaveformCanvasHandle>();
    render(
      <WaveformCanvas
        ref={ref}
        waveform={LEVEL}
        durationMs={2000}
        restoredPositionMs={1500}
      />,
    );
    flushRaf();

    expect(ref.current?.getPlayheadPosition()).toBe(1500);

    emitPosition(500);
    flushRaf();
    expect(ref.current?.getPlayheadPosition()).toBe(500);
  });

  it("repositions a marker by dragging it on the waveform", () => {
    const onSetAbPoint = vi.fn();
    const { container } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
        onSetAbPoint={onSetAbPoint}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    const endMarker = container.querySelector(
      "[data-testid='waveform-ab-end']",
    ) as HTMLElement;

    fireEvent.pointerDown(endMarker, { clientX: 75, pointerId: 1 });
    fireEvent.pointerMove(endMarker, { clientX: 90, pointerId: 1 });
    fireEvent.pointerUp(endMarker, { pointerId: 1 });

    expect(onSetAbPoint).toHaveBeenCalledWith("b", 1_800);
  });

  it("clamps a dragged A marker so it never passes B", () => {
    const onSetAbPoint = vi.fn();
    const { container } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
        onSetAbPoint={onSetAbPoint}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    const startMarker = container.querySelector(
      "[data-testid='waveform-ab-start']",
    ) as HTMLElement;

    fireEvent.pointerDown(startMarker, { clientX: 50, pointerId: 1 });
    fireEvent.pointerMove(startMarker, { clientX: 90, pointerId: 1 });
    fireEvent.pointerUp(startMarker, { pointerId: 1 });

    expect(onSetAbPoint).toHaveBeenCalledWith("a", 1_499);
  });

  it("exposes keyboard-adjustable A-B marker sliders", () => {
    const onSetAbPoint = vi.fn();
    const { getByRole } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
        onSetAbPoint={onSetAbPoint}
      />,
    );

    const start = getByRole("slider", { name: "A point" });
    const end = getByRole("slider", { name: "B point" });
    expect(start).toHaveAttribute("aria-valuenow", "1000");
    expect(start).toHaveAttribute("aria-valuemax", "1499");
    expect(end).toHaveAttribute("aria-valuemin", "1001");

    fireEvent.keyDown(start, { key: "ArrowRight" });
    fireEvent.keyDown(end, { key: "ArrowLeft", shiftKey: true });

    expect(onSetAbPoint).toHaveBeenNthCalledWith(1, "a", 1_001);
    expect(onSetAbPoint).toHaveBeenNthCalledWith(2, "b", 1_400);
  });

  it("reverts a cancelled marker drag to the committed position", () => {
    const onSetAbPoint = vi.fn();
    const { container } = render(
      <WaveformCanvas
        waveform={LEVEL}
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
        onSetAbPoint={onSetAbPoint}
      />,
    );
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    stubCanvasRect(canvas, 100);
    const startMarker = container.querySelector(
      "[data-testid='waveform-ab-start']",
    ) as HTMLElement;

    fireEvent.pointerDown(startMarker, { clientX: 50, pointerId: 1 });
    fireEvent.pointerMove(startMarker, { clientX: 90, pointerId: 1 });
    fireEvent.pointerCancel(startMarker, { pointerId: 1 });

    expect(onSetAbPoint).not.toHaveBeenCalled();
    expect(startMarker).toHaveStyle("--ab-x: 50");
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
