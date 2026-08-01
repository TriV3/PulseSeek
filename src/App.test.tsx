import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
});

describe("application shell", () => {
  it("renders the folder tree with an accessible heading", () => {
    render(<App />);

    // The heading is visually hidden but present for screen readers.
    expect(
      screen.getByRole("heading", { level: 1, name: "PulseSeek" }),
    ).toBeInTheDocument();

    expect(
      screen.queryByRole("button", { name: "Open Folder" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "File list" }),
    ).toBeInTheDocument();
  });

  it("presents the Resonic-inspired workspace regions", () => {
    render(<App />);

    expect(
      screen.getByRole("region", { name: "Waveform overview" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("toolbar", { name: "Playback controls" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Browser" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "File list" })).toBeInTheDocument();
  });

  it("lets keyboard users resize both workspace splits", () => {
    render(<App />);

    const waveformSeparator = screen.getByRole("separator", {
      name: "Resize waveform",
    });
    const browserSeparator = screen.getByRole("separator", {
      name: "Resize browser",
    });

    expect(waveformSeparator).toHaveAttribute("aria-valuenow", "38");
    fireEvent.keyDown(waveformSeparator, { key: "ArrowDown" });
    expect(waveformSeparator).toHaveAttribute("aria-valuenow", "40");

    expect(browserSeparator).toHaveAttribute("aria-valuenow", "24");
    fireEvent.keyDown(browserSeparator, { key: "ArrowRight" });
    expect(browserSeparator).toHaveAttribute("aria-valuenow", "26");
  });
});
