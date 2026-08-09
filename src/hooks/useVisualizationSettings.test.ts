import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  loadVisualizationSettings,
  saveVisualizationSettings,
} from "../api/commandEnvelope";
import { useVisualizationSettings } from "./useVisualizationSettings";

vi.mock("../api/commandEnvelope", () => ({
  loadVisualizationSettings: vi.fn(),
  saveVisualizationSettings: vi.fn(),
}));

describe("useVisualizationSettings", () => {
  beforeEach(() => {
    vi.mocked(loadVisualizationSettings).mockReset();
    vi.mocked(saveVisualizationSettings).mockReset();
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn().mockReturnValue({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }),
    });
  });

  it("loads, persists selection, and exposes the effective visualization", async () => {
    vi.mocked(loadVisualizationSettings).mockResolvedValue({
      enabled: true,
      mode: "linear",
      quality: "balanced",
    });
    vi.mocked(saveVisualizationSettings).mockImplementation(
      async (settings) => settings,
    );

    const { result } = renderHook(() => useVisualizationSettings());
    await waitFor(() => expect(result.current.isLoaded).toBe(true));
    expect(result.current.effectiveMode).toBe("linear");

    act(() => result.current.update({ quality: "high" }));
    await waitFor(() =>
      expect(saveVisualizationSettings).toHaveBeenCalledWith(
        { enabled: true, mode: "linear", quality: "high" },
        false,
      ),
    );
  });

  it("falls back to waveform when reduced motion is requested", async () => {
    vi.mocked(window.matchMedia).mockReturnValue({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    } as unknown as MediaQueryList);
    vi.mocked(loadVisualizationSettings).mockResolvedValue({
      enabled: true,
      mode: "musical",
      quality: "high",
    });

    const { result } = renderHook(() => useVisualizationSettings());
    await waitFor(() => expect(result.current.isLoaded).toBe(true));

    expect(loadVisualizationSettings).toHaveBeenCalledWith(true);
    expect(result.current.effectiveMode).toBe("waveform");
    expect(result.current.reducedMotion).toBe(true);
  });

  it("does not overwrite an immediate user choice with a late initial load", async () => {
    let resolveLoad:
      | ((settings: {
          enabled: boolean;
          mode: "waveform";
          quality: "balanced";
        }) => void)
      | null = null;
    vi.mocked(loadVisualizationSettings).mockReturnValue(
      new Promise((resolve) => {
        resolveLoad = resolve;
      }),
    );
    vi.mocked(saveVisualizationSettings).mockImplementation(
      async (settings) => settings,
    );

    const { result } = renderHook(() => useVisualizationSettings());
    act(() => result.current.update({ mode: "linear" }));
    await waitFor(() => expect(result.current.effectiveMode).toBe("linear"));

    act(() => {
      if (!resolveLoad) throw new Error("load resolver unavailable");
      resolveLoad({ enabled: true, mode: "waveform", quality: "balanced" });
    });
    await waitFor(() => expect(result.current.isLoaded).toBe(true));

    expect(result.current.settings.mode).toBe("linear");
    expect(result.current.effectiveMode).toBe("linear");
  });
});
