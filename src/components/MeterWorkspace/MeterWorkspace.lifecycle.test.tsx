import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MeterWorkspace } from "./MeterWorkspace";

afterEach(() => vi.restoreAllMocks());

describe("MeterWorkspace grid lifecycle", () => {
  it("supports eight tiles, keyboard reorder, resize, and maximize restore", () => {
    render(<MeterWorkspace />);
    for (const name of [
      "Spectrum Analyzer",
      "Band Energy",
      "Colored Waveform",
      "Spectrogram",
      "Loudness",
      "True Peak",
      "Stereo",
      "Diagnostics",
    ])
      fireEvent.click(screen.getByRole("button", { name }));

    const tile = screen.getByRole("article", { name: /Spectrum Analyzer/ });
    fireEvent.click(
      screen.getByRole("button", { name: "Maximize Spectrum Analyzer" }),
    );
    expect(tile).toHaveAttribute("data-maximized", "true");
    fireEvent.click(
      screen.getByRole("button", { name: "Restore Spectrum Analyzer" }),
    );
    expect(tile).toHaveAttribute("data-maximized", "false");
    fireEvent.click(
      screen.getByRole("button", { name: "Increase Spectrum Analyzer width" }),
    );
    expect(tile).toHaveAttribute("data-width", "260");
    const move = screen.getByRole("button", {
      name: "Reorder Spectrum Analyzer",
    });
    move.click();
    expect(screen.getAllByRole("article")[1]).toHaveAccessibleName(
      "Band Energy",
    );
  });

  it("navigates tile focus and observes container resize", () => {
    const observe = vi.fn();
    const disconnect = vi.fn();
    const observers: Array<{ trigger: () => void }> = [];
    class Observer {
      observe = observe;
      disconnect = disconnect;
      constructor(private readonly callback: ResizeObserverCallback) {
        observers.push(this);
      }
      trigger() {
        this.callback(
          [
            {
              target: document.createElement("div"),
            } as unknown as ResizeObserverEntry,
          ],
          this as unknown as ResizeObserver,
        );
      }
    }
    window.ResizeObserver = Observer as unknown as typeof ResizeObserver;
    render(<MeterWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));
    fireEvent.click(screen.getByRole("button", { name: "Band Energy" }));
    const tiles = screen.getAllByRole("article");
    tiles[0].focus();
    fireEvent.keyDown(tiles[0], { key: "ArrowRight" });
    expect(document.activeElement).toBe(tiles[1]);
    expect(tiles[0]).toHaveAttribute("tabindex", "-1");
    expect(tiles[1]).toHaveAttribute("tabindex", "0");
    fireEvent.keyDown(tiles[1], { key: "ArrowLeft", shiftKey: true });
    expect(screen.getAllByRole("article")[0]).toHaveAccessibleName(
      "Band Energy",
    );
    expect(observe).toHaveBeenCalled();
    const grid = screen.getByRole("region", { name: "Meter tiles" });
    vi.spyOn(grid, "getBoundingClientRect").mockReturnValue({
      width: 240,
      height: 180,
    } as DOMRect);
    observers[0]?.trigger();
    expect(tiles[0]).toHaveAttribute("data-width", "220");
    expect(tiles[0]).toHaveStyle({ minHeight: "160px" });
    expect(observe).toHaveBeenCalled();
    expect(disconnect).toHaveBeenCalled();
  });

  it("uses ResizeObserver independently to clamp changed container bounds", () => {
    const observers: Array<{ trigger: () => void }> = [];
    class Observer {
      constructor(private readonly callback: ResizeObserverCallback) {
        observers.push(this);
      }
      observe() {}
      disconnect() {}
      trigger() {
        this.callback([], this as unknown as ResizeObserver);
      }
    }
    window.ResizeObserver = Observer as unknown as typeof ResizeObserver;
    render(<MeterWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));
    const grid = screen.getByRole("region", { name: "Meter tiles" });
    const tile = screen.getByRole("article", { name: "Spectrum Analyzer" });
    vi.spyOn(grid, "getBoundingClientRect").mockReturnValue({
      width: 280,
      height: 200,
    } as DOMRect);
    observers.at(-1)?.trigger();
    expect(tile).toHaveAttribute("data-width", "220");
    expect(tile).toHaveStyle({ minHeight: "160px" });
  });

  it("keeps rapid additions unique and transfers focus after removal", () => {
    render(<MeterWorkspace />);
    const add = screen.getByRole("button", { name: "Spectrum Analyzer" });
    fireEvent.click(add);
    fireEvent.click(add);
    const tiles = screen.getAllByRole("article");
    expect(tiles).toHaveLength(2);
    expect(tiles[0]).toHaveTextContent("meter-tile-1");
    expect(tiles[1]).toHaveTextContent("meter-tile-2");
    tiles[0].focus();
    fireEvent.click(
      screen.getAllByRole("button", { name: "Remove Spectrum Analyzer" })[0],
    );
    expect(screen.getByRole("article")).toHaveAttribute("tabindex", "0");
  });

  it("preserves native arrow handling in tile controls", () => {
    render(<MeterWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));
    const state = screen.getByRole("combobox", {
      name: "Spectrum Analyzer state",
    });
    state.focus();
    fireEvent.keyDown(state, { key: "ArrowDown" });
    expect(document.activeElement).toBe(state);
  });

  it("keeps frame data outside workspace state", () => {
    const { container } = render(<MeterWorkspace />);
    expect(container.textContent).not.toContain("frame");
    expect(container.querySelector("canvas")).toBeNull();
  });

  it("retains shared subscription until final tile removal and cleans up on unmount", () => {
    const cleanups = [vi.fn()];
    const subscribe = vi.fn(() => cleanups[0]);
    const { unmount } = render(<MeterWorkspace onSubscribe={subscribe} />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Duplicate Spectrum Analyzer" }),
    );
    expect(subscribe).toHaveBeenCalledTimes(1);
    fireEvent.click(
      screen.getAllByRole("button", { name: "Remove Spectrum Analyzer" })[0],
    );
    expect(cleanups[0]).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "Remove Spectrum Analyzer" }),
    );
    expect(cleanups[0]).toHaveBeenCalledTimes(1);
    unmount();
    expect(cleanups[0]).toHaveBeenCalledTimes(1);
  });

  it("cleans retained subscription on workspace unmount", () => {
    const cleanup = vi.fn();
    const { unmount } = render(<MeterWorkspace onSubscribe={() => cleanup} />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));
    unmount();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it("mounts and releases subscriptions through lifecycle callbacks", () => {
    const subscribe = vi.fn(() => vi.fn());
    const { unmount } = render(<MeterWorkspace onSubscribe={subscribe} />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));
    expect(subscribe).toHaveBeenCalledWith("spectrum");
    fireEvent.click(
      screen.getByRole("button", { name: "Remove Spectrum Analyzer" }),
    );
    expect(subscribe.mock.results[0]?.value).toHaveBeenCalled();
    unmount();
  });
});
