import { describe, expect, it, vi, afterEach } from "vitest";
import {
  buildEnvelope,
  drawEnvelope,
  positionMsForX,
  resolveTokens,
  defaultTargetPeaksForWidth,
  type Canvas2D,
  type EnvelopeGeometry,
} from "./waveformRenderer";
function geometryFor(
  channels: number,
  min: number[],
  max: number[],
  width = 100,
  height = 40,
): EnvelopeGeometry {
  return buildEnvelope({ channels, min, max }, width, height, null, null);
}

function createMockContext(): {
  ctx: Canvas2D;
  calls: string[];
  strokeStyles: unknown[];
  fillStyles: unknown[];
  gradients: Array<{ stops: Array<[number, string]> }>;
} {
  const calls: string[] = [];
  const strokeStyles: unknown[] = [];
  const fillStyles: unknown[] = [];
  const gradients: Array<{
    stops: Array<[number, string]>;
  }> = [];
  const ctx: Canvas2D = {
    clearRect: () => calls.push("clearRect"),
    beginPath: () => calls.push("beginPath"),
    moveTo: () => calls.push("moveTo"),
    lineTo: () => calls.push("lineTo"),
    stroke: () => calls.push("stroke"),
    fill: () => calls.push("fill"),
    closePath: () => calls.push("closePath"),
    setTransform: () => calls.push("setTransform"),
    setLineDash: (segments) => calls.push(`setLineDash:${segments.join(",")}`),
    createLinearGradient: () => {
      const gradient = {
        addColorStop: (offset: number, color: string) => {
          gradients.push({ stops: [[offset, color]] });
        },
      };
      return gradient;
    },
    strokeStyle: "#000",
    fillStyle: "#000",
    lineWidth: 1,
    lineCap: "butt",
    lineJoin: "miter",
  };
  Object.defineProperty(ctx, "strokeStyle", {
    set(value: unknown) {
      strokeStyles.push(value);
    },
  });
  Object.defineProperty(ctx, "fillStyle", {
    set(value: unknown) {
      fillStyles.push(value);
    },
  });
  return { ctx, calls, strokeStyles, fillStyles, gradients };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("buildEnvelope", () => {
  it("maps a mono waveform to one centered row", () => {
    const geometry = geometryFor(1, [-1, 0], [1, 0.5]);

    expect(geometry.channels).toHaveLength(1);
    const channel = geometry.channels[0];
    expect(channel.rowCenter).toBe(20);
    expect(channel.rowHeight).toBe(40);
    // Buckets: x at 25 and 75. max edge: amp 1 -> y 0, amp 0.5 -> y 10.
    expect(channel.maxPoints).toEqual([
      { x: 25, y: 0 },
      { x: 75, y: 10 },
    ]);
    // min edge: amp -1 -> y 40, amp 0 -> y 20.
    expect(channel.minPoints).toEqual([
      { x: 25, y: 40 },
      { x: 75, y: 20 },
    ]);
  });

  it("stacks stereo channels into separate rows", () => {
    const geometry = geometryFor(2, [-0.5, -0.25], [0.5, 0.75], 100, 80);

    expect(geometry.channels).toHaveLength(2);
    expect(geometry.channels[0].rowCenter).toBe(20);
    expect(geometry.channels[1].rowCenter).toBe(60);

    const second = geometry.channels[1];
    // Second channel peak is min[1] = -0.25 -> y = 60 + 0.25*20 = 65.
    expect(second.minPoints[0]).toEqual({ x: 50, y: 65 });
    expect(second.maxPoints[0]).toEqual({ x: 50, y: 45 });
  });

  it("clamps amplitudes and row bounds", () => {
    const geometry = geometryFor(1, [-5, 0], [5, 0], 100, 40);
    const channel = geometry.channels[0];
    // Amplitude 5 clamps to 1 -> y 0; -5 clamps to -1 -> y 40.
    expect(channel.maxPoints[0].y).toBe(0);
    expect(channel.minPoints[0].y).toBe(40);
  });

  it("returns no channels for degenerate sizes", () => {
    expect(geometryFor(1, [-1], [1], 0, 40).channels).toHaveLength(0);
    expect(geometryFor(1, [-1], [1], 100, 0).channels).toHaveLength(0);
  });
});

describe("playhead mapping", () => {
  it("maps position over duration to an x coordinate", () => {
    const geometry = buildEnvelope(
      { channels: 1, min: [-1], max: [1] },
      100,
      40,
      500,
      2000,
    );
    expect(geometry.playheadX).toBe(25);
  });

  it("clamps the playhead to the rendered span", () => {
    const start = buildEnvelope(
      { channels: 1, min: [-1], max: [1] },
      100,
      40,
      0,
      2000,
    );
    const end = buildEnvelope(
      { channels: 1, min: [-1], max: [1] },
      100,
      40,
      3000,
      2000,
    );
    expect(start.playheadX).toBe(0);
    expect(end.playheadX).toBe(100);
  });

  it("returns no playhead when position or duration is unknown", () => {
    const none = buildEnvelope(
      { channels: 1, min: [-1], max: [1] },
      100,
      40,
      500,
      null,
    );
    const noPosition = buildEnvelope(
      { channels: 1, min: [-1], max: [1] },
      100,
      40,
      null,
      2000,
    );
    expect(none.playheadX).toBeNull();
    expect(noPosition.playheadX).toBeNull();
  });
});

describe("drawEnvelope", () => {
  const geometry = geometryFor(1, [-1, 0], [1, 0.5], 100, 40);

  it("clears, draws grid, envelopes, and playhead", () => {
    const { ctx, calls } = createMockContext();
    const withPlayhead = { ...geometry, playheadX: 25 };

    drawEnvelope(
      ctx,
      withPlayhead,
      {
        wave: "#abc",
        waveGrid: "#def",
        waveSoft: "#123",
        playhead: "#f00",
      },
      100,
      40,
    );

    expect(calls[0]).toBe("clearRect");
    expect(
      calls.filter((call) => call === "stroke").length,
    ).toBeGreaterThanOrEqual(4);
    expect(calls.some((call) => call.startsWith("setLineDash:4,3"))).toBe(
      false,
    );
  });

  it("consumes semantic theme tokens for every color", () => {
    const { ctx, strokeStyles } = createMockContext();
    drawEnvelope(
      ctx,
      geometry,
      {
        wave: "#111111",
        waveGrid: "#222222",
        waveSoft: "#444444",
        playhead: "#333333",
      },
      100,
      40,
    );

    expect(strokeStyles).toContain("#111111");
    expect(strokeStyles).toContain("#222222");
    expect(strokeStyles).not.toContain("#333333"); // no playhead -> no playhead stroke
  });

  it("draws the playhead with the playhead token", () => {
    const { ctx, strokeStyles } = createMockContext();
    drawEnvelope(
      ctx,
      { ...geometry, playheadX: 50 },
      {
        wave: "#111111",
        waveGrid: "#222222",
        waveSoft: "#444444",
        playhead: "#333333",
      },
      100,
      40,
    );

    expect(strokeStyles).toContain("#333333");
  });

  it("gives the outline style a soft body fill so the center reads against the background", () => {
    const { ctx, calls, strokeStyles, fillStyles } = createMockContext();
    drawEnvelope(
      ctx,
      geometry,
      {
        wave: "#111111",
        waveGrid: "#222222",
        waveSoft: "#444444",
        playhead: "#333333",
      },
      100,
      40,
      "outline",
    );

    // The envelope body is filled with the soft token (distinct from the
    // exterior), and the min/max edges are still stroked with the wave token.
    expect(calls).toContain("fill");
    expect(fillStyles).toContain("#444444");
    expect(strokeStyles).toContain("#111111");
  });

  it("fills the envelope for the solid style instead of stroking it", () => {
    const { ctx, calls, strokeStyles, fillStyles } = createMockContext();
    drawEnvelope(
      ctx,
      geometry,
      {
        wave: "#111111",
        waveGrid: "#222222",
        waveSoft: "#444444",
        playhead: "#333333",
      },
      100,
      40,
      "solid",
    );

    expect(calls).toContain("fill");
    expect(calls).toContain("closePath");
    expect(fillStyles).toContain("#111111");
    // The envelope is filled; the two envelope polylines are not stroked.
    expect(strokeStyles.filter((style) => style === "#111111")).toHaveLength(0);
  });

  it("fills the envelope with a token gradient for the gradient style", () => {
    const { ctx, calls, gradients } = createMockContext();
    drawEnvelope(
      ctx,
      geometry,
      {
        wave: "#111111",
        waveGrid: "#222222",
        waveSoft: "#444444",
        playhead: "#333333",
      },
      100,
      40,
      "gradient",
    );

    expect(calls).toContain("fill");
    expect(gradients.length).toBeGreaterThan(0);
    const stopColors = gradients.flatMap((gradient) =>
      gradient.stops.map(([, color]) => color),
    );
    expect(stopColors).toContain("#111111");
    expect(stopColors).toContain("#444444");
  });
});

describe("resolveTokens", () => {
  it("reads semantic waveform tokens from the computed style", () => {
    const getPropertyValue = vi.fn((name: string) => {
      if (name === "--wave") return "#0a0a0a";
      if (name === "--wave-grid") return "#1b1b1b";
      if (name === "--wave-soft") return "#2c2c2c";
      if (name === "--wave-seek-current") return "#3d3d3d";
      return "";
    });
    vi.spyOn(window, "getComputedStyle").mockReturnValue({
      getPropertyValue,
    } as unknown as CSSStyleDeclaration);

    expect(resolveTokens(document.createElement("canvas"))).toEqual({
      wave: "#0a0a0a",
      waveGrid: "#1b1b1b",
      waveSoft: "#2c2c2c",
      playhead: "#3d3d3d",
    });
  });

  it("falls back to defaults when tokens are missing", () => {
    const getPropertyValue = vi.fn(() => "");
    vi.spyOn(window, "getComputedStyle").mockReturnValue({
      getPropertyValue,
    } as unknown as CSSStyleDeclaration);

    expect(resolveTokens(document.createElement("canvas")).wave).not.toBe("");
  });

  it("returns defaults without a scope", () => {
    expect(resolveTokens(null)).toEqual({
      wave: "#7f91a2",
      waveGrid: "#cbd3db",
      waveSoft: "#a7b6c4",
      playhead: "#f29c38",
    });
  });
});

describe("defaultTargetPeaksForWidth", () => {
  it("asks for two buckets per pixel with a positive floor", () => {
    expect(defaultTargetPeaksForWidth(100)).toBe(200);
    expect(defaultTargetPeaksForWidth(0)).toBe(1);
  });

  it("never produces a non-finite target", () => {
    expect(defaultTargetPeaksForWidth(Number.NaN)).toBe(1);
    expect(defaultTargetPeaksForWidth(Number.POSITIVE_INFINITY)).toBe(1);
    expect(defaultTargetPeaksForWidth(-5)).toBe(1);
  });
});

describe("positionMsForX", () => {
  it("maps the left edge to zero and the right edge to the duration", () => {
    expect(positionMsForX(0, 100, 2000)).toBe(0);
    expect(positionMsForX(100, 100, 2000)).toBe(2000);
  });

  it("maps the center proportionally", () => {
    expect(positionMsForX(50, 100, 2000)).toBe(1000);
  });

  it("returns an integer millisecond accepted by the Rust seek command", () => {
    expect(positionMsForX(1, 3, 2000)).toBe(667);
    expect(Number.isInteger(positionMsForX(1, 3, 2000))).toBe(true);
  });

  it("clamps coordinates outside the canvas", () => {
    expect(positionMsForX(-20, 100, 2000)).toBe(0);
    expect(positionMsForX(500, 100, 2000)).toBe(2000);
  });

  it("returns null without a known duration or width", () => {
    expect(positionMsForX(50, 100, null)).toBeNull();
    expect(positionMsForX(50, 0, 2000)).toBeNull();
  });
});
