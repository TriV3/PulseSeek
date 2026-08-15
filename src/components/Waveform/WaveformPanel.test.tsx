import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { WaveformPanel } from "./WaveformPanel";
import { getWaveform } from "../../api/waveform";
import {
  onMusicalSpectrumFrame,
  onPosition,
  onSpectrumFrame,
  onWaveformReady,
} from "../../api/playbackEvents";
import type { WaveformLevel } from "../../api/waveform";

vi.mock("../../api/waveform", () => ({ getWaveform: vi.fn() }));
vi.mock("../../api/playbackEvents", () => ({
  onMusicalSpectrumFrame: vi.fn(),
  onPosition: vi.fn(),
  onSpectrumFrame: vi.fn(),
  onWaveformReady: vi.fn(),
}));

const LEVEL: WaveformLevel = {
  format_version: 1,
  channels: 1,
  samples_per_peak: 10,
  min: [-0.5, 0, 0.5],
  max: [-0.4, 0.1, 0.6],
};

let observerInstances: Array<{
  trigger: (width: number, height: number) => void;
}>;

class MockResizeObserver {
  private readonly callback: ResizeObserverCallback;
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    observerInstances.push({
      trigger: (width: number, height: number) => {
        this.callback(
          [{ contentRect: { width, height } } as ResizeObserverEntry],
          this as unknown as ResizeObserver,
        );
      },
    });
  }
  observe = vi.fn();
  disconnect = vi.fn();
}

beforeEach(() => {
  observerInstances = [];
  vi.mocked(getWaveform).mockReset().mockResolvedValue(LEVEL);
  vi.mocked(onPosition).mockResolvedValue(() => {});
  vi.mocked(onMusicalSpectrumFrame).mockResolvedValue(() => {});
  vi.mocked(onSpectrumFrame).mockResolvedValue(() => {});
  vi.mocked(onWaveformReady).mockResolvedValue(() => {});
  window.ResizeObserver =
    MockResizeObserver as unknown as typeof ResizeObserver;
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  // @ts-expect-error restoring test-only stub on jsdom
  delete window.ResizeObserver;
});

describe("WaveformPanel", () => {
  it("shows the selected file name in the header", () => {
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
      />,
    );

    expect(screen.getByText("A.wav")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Audio visualization" }),
    ).toBeInTheDocument();
  });

  it("keeps A/B controls in their own row before waveform workspace", () => {
    const { container } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
      />,
    );
    const panel = container.querySelector(".waveform-panel")!;

    expect([...panel.children].map((child) => child.className)).toEqual([
      "now-playing",
      "audio-summary",
      "ab-controls",
      "visualization-workspace",
    ]);
  });

  it("hides the A/B controls row in compact mode", () => {
    const { container } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        compact
      />,
    );
    const panel = container.querySelector(".waveform-panel")!;

    expect(
      screen.queryByLabelText("A-B repeat region"),
    ).not.toBeInTheDocument();
    expect([...panel.children].map((child) => child.className)).toEqual([
      "now-playing",
      "audio-summary",
      "visualization-workspace",
    ]);
  });

  it("renders the compact toggle in the header and hides the brand in compact mode", () => {
    const onToggle = vi.fn();
    const { rerender } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        onToggleCompact={onToggle}
      />,
    );

    const toggle = screen.getByRole("button", { name: "Toggle compact mode" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(toggle);
    expect(onToggle).toHaveBeenCalled();

    rerender(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        compact
        onToggleCompact={onToggle}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Toggle compact mode" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByLabelText("PulseSeek")).not.toBeInTheDocument();
  });

  it("shows only the waveform by default", () => {
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        theme="midnight"
      />,
    );

    expect(
      screen.getByRole("slider", { name: "Waveform seek" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).not.toBeInTheDocument();
  });

  it("shows the logarithmic analyzer instead of the waveform when selected", () => {
    const onSeek = vi.fn();
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        theme="midnight"
        visualization="logarithmic"
        onSeek={onSeek}
      />,
    );

    expect(
      screen.getByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("slider", { name: "Waveform seek" }),
    ).not.toBeInTheDocument();
    const analyzerSeek = screen.getByRole("slider", {
      name: "Log analyzer seek",
    });
    vi.spyOn(analyzerSeek, "getBoundingClientRect").mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: 100,
      bottom: 40,
      width: 100,
      height: 40,
      toJSON: () => ({}),
    } as DOMRect);
    fireEvent.pointerDown(analyzerSeek, { clientX: 75, pointerId: 1 });
    fireEvent.pointerUp(analyzerSeek, { pointerId: 1 });
    expect(onSeek).toHaveBeenCalledWith(1500);
    expect(getWaveform).not.toHaveBeenCalled();
  });

  it("shows the linear analyzer with the same seek overlay instead of the waveform", async () => {
    const onSeek = vi.fn();
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        theme="high-contrast"
        visualization="linear"
        onSeek={onSeek}
      />,
    );

    expect(
      screen.getByRole("img", { name: "Linear frequency analyzer" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Logarithmic frequency analyzer" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("slider", { name: "Waveform seek" }),
    ).not.toBeInTheDocument();
    const analyzerSeek = screen.getByRole("slider", {
      name: "Linear analyzer seek",
    });
    vi.spyOn(analyzerSeek, "getBoundingClientRect").mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: 100,
      bottom: 40,
      width: 100,
      height: 40,
      toJSON: () => ({}),
    } as DOMRect);
    fireEvent.pointerMove(analyzerSeek, { clientX: 25, pointerId: 1 });
    fireEvent.pointerDown(analyzerSeek, { clientX: 75, pointerId: 1 });
    fireEvent.pointerUp(analyzerSeek, { pointerId: 1 });
    await waitFor(() =>
      expect(screen.getByTestId("waveform-hover-marker")).toHaveStyle(
        "--seek-x: 25px",
      ),
    );
    expect(onSeek).toHaveBeenCalledWith(1500);
    expect(getWaveform).not.toHaveBeenCalled();
  });

  it("shows the musical spectrum exclusively with the shared seek overlay", () => {
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2_000}
        theme="dark"
        visualization="musical"
      />,
    );

    expect(
      screen.getByRole("img", { name: "Musical spectrum" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("slider", { name: "Musical spectrum seek" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("img", { name: "Linear frequency analyzer" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("slider", { name: "Waveform seek" }),
    ).not.toBeInTheDocument();
    expect(getWaveform).not.toHaveBeenCalled();
  });

  it("shows the selected file's actual audio metadata", () => {
    render(
      <WaveformPanel
        entryPath="/music/a.mp3"
        entryName="A.mp3"
        durationMs={2000}
        metadata={{
          duration_ms: 2000,
          size_bytes: 1234,
          modified_at_ms: 5678,
          channels: 1,
          sample_rate: 48_000,
          bit_depth: null,
          codec: "MP3",
        }}
      />,
    );

    expect(screen.getByText("48 kHz, mono · MP3")).toBeInTheDocument();
    expect(screen.queryByText(/lossless/i)).not.toBeInTheDocument();
  });

  it("does not invent audio metadata when it is unavailable", () => {
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        metadata={null}
      />,
    );

    expect(screen.getByText("Audio details unavailable")).toBeInTheDocument();
    expect(screen.queryByText(/44\.1 kHz/i)).not.toBeInTheDocument();
  });

  it("requests a waveform level for the selected file", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const { container } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
      />,
    );

    await waitFor(() =>
      expect(getWaveform).toHaveBeenCalledWith(
        "/music/a.wav",
        expect.any(Number),
      ),
    );
    expect(container.querySelector("canvas")).not.toBeNull();
  });

  it("keeps the canvas empty until a file is selected", () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    render(
      <WaveformPanel
        entryPath={null}
        entryName="No file selected"
        durationMs={null}
      />,
    );

    expect(screen.getByText("No file selected")).toBeInTheDocument();
    expect(getWaveform).not.toHaveBeenCalled();
  });

  it("renders an accessible error when the waveform cannot be loaded", async () => {
    vi.mocked(getWaveform).mockRejectedValue(new Error("corrupt file"));
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("corrupt file");
  });

  it("re-requests a coarser level after the canvas width changes", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
      />,
    );
    await waitFor(() => expect(getWaveform).toHaveBeenCalled());

    for (const observer of observerInstances) observer.trigger(300, 100);
    await waitFor(
      () => expect(getWaveform).toHaveBeenCalledWith("/music/a.wav", 600),
      { timeout: 2000 },
    );
  });

  it("forwards canvas seeks to the panel seek handler", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const onSeek = vi.fn();
    const { container } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        onSeek={onSeek}
      />,
    );
    await waitFor(() => expect(getWaveform).toHaveBeenCalled());

    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: 100,
      bottom: 40,
      width: 100,
      height: 40,
      toJSON: () => ({}),
    } as DOMRect);

    fireEvent.pointerDown(canvas, { clientX: 50, pointerId: 1 });
    fireEvent.pointerUp(canvas, { pointerId: 1 });

    expect(onSeek).toHaveBeenCalledWith(1000);
  });

  it("does not refetch waveform data when only the style changes", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    const { rerender } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        style="outline"
      />,
    );
    await waitFor(() => expect(getWaveform).toHaveBeenCalledTimes(1));

    rerender(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        style="solid"
      />,
    );

    expect(getWaveform).toHaveBeenCalledTimes(1);
  });

  it("disables A-B placement while the duration is unknown", () => {
    render(
      <WaveformPanel
        entryPath={null}
        entryName="No file selected"
        durationMs={null}
      />,
    );

    expect(screen.getByRole("button", { name: /Set A/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Set B/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Clear A-B/i })).toBeDisabled();
  });

  it("enables placement once an entry and duration exist", () => {
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
      />,
    );

    expect(screen.getByRole("button", { name: /Set A/i })).toBeEnabled();
    expect(screen.getByRole("button", { name: /Set B/i })).toBeEnabled();
    expect(screen.getByRole("button", { name: /Clear A-B/i })).toBeDisabled();
  });

  it("places A and B points at the current playhead", () => {
    const onSetAbPoint = vi.fn();
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        restoredPositionMs={500}
        playheadPositionMs={500}
        onSetAbPoint={onSetAbPoint}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Set A/i }));
    fireEvent.click(screen.getByRole("button", { name: /Set B/i }));

    expect(onSetAbPoint).toHaveBeenNthCalledWith(1, "a", 500);
    expect(onSetAbPoint).toHaveBeenNthCalledWith(2, "b", 500);
  });

  it("shows placed points, a confirmed region, and placement errors", async () => {
    vi.mocked(getWaveform).mockResolvedValue(LEVEL);
    render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
        abError="B point must be after the A point."
      />,
    );

    expect(screen.getByText("A 0:01")).toBeInTheDocument();
    expect(screen.getByText("B 0:01")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "B point must be after the A point.",
    );
    expect(screen.getByRole("button", { name: /Clear A-B/i })).toBeEnabled();

    await waitFor(() => expect(getWaveform).toHaveBeenCalled());
    expect(screen.getByTestId("waveform-ab-band")).toBeInTheDocument();
  });

  it("forwards clearing from the cluster and disables Clear without points", () => {
    const onClearAB = vi.fn();
    const { rerender } = render(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        abPoints={{ startMs: 1_000, endMs: 1_500 }}
        loopRegion={{ startMs: 1_000, endMs: 1_500 }}
        onClearAB={onClearAB}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Clear A-B/i }));
    expect(onClearAB).toHaveBeenCalledOnce();

    rerender(
      <WaveformPanel
        entryPath="/music/a.wav"
        entryName="A.wav"
        durationMs={2000}
        onClearAB={onClearAB}
      />,
    );
    expect(screen.getByRole("button", { name: /Clear A-B/i })).toBeDisabled();
  });
});
