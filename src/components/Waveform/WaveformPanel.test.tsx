import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { WaveformPanel } from "./WaveformPanel";
import { getWaveform } from "../../api/waveform";
import { onPosition, onWaveformReady } from "../../api/playbackEvents";
import type { WaveformLevel } from "../../api/waveform";

vi.mock("../../api/waveform", () => ({ getWaveform: vi.fn() }));
vi.mock("../../api/playbackEvents", () => ({
  onPosition: vi.fn(),
  onWaveformReady: vi.fn(),
}));

const LEVEL: WaveformLevel = {
  format_version: 1,
  channels: 1,
  samples_per_peak: 10,
  min: [-0.5, 0, 0.5],
  max: [-0.4, 0.1, 0.6],
};

let observerInstance: {
  trigger: (width: number, height: number) => void;
} | null;

class MockResizeObserver {
  private readonly callback: ResizeObserverCallback;
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    observerInstance = {
      trigger: (width: number, height: number) => {
        this.callback(
          [{ contentRect: { width, height } } as ResizeObserverEntry],
          this as unknown as ResizeObserver,
        );
      },
    };
  }
  observe = vi.fn();
  disconnect = vi.fn();
}

beforeEach(() => {
  observerInstance = null;
  vi.mocked(getWaveform).mockReset().mockResolvedValue(LEVEL);
  vi.mocked(onPosition).mockResolvedValue(() => {});
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
      screen.getByRole("region", { name: "Waveform overview" }),
    ).toBeInTheDocument();
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

    observerInstance?.trigger(300, 100);
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
});
