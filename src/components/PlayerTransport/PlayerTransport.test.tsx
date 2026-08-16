import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PlayerTransport } from "./PlayerTransport";

const props = {
  status: "playing" as const,
  hasSelection: true,
  positionMs: 12_000,
  durationMs: 60_000,
  volume: 1,
  muted: false,
  canPrevious: true,
  canNext: true,
  error: null,
  onTogglePlayPause: vi.fn(),
  onStop: vi.fn(),
  onPrevious: vi.fn(),
  onNext: vi.fn(),
  onVolume: vi.fn(),
  onToggleMute: vi.fn(),
};

describe("PlayerTransport", () => {
  it("renders accessible transport controls and time", () => {
    render(<PlayerTransport {...props} />);

    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Previous" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next" })).toBeInTheDocument();
    expect(
      screen.queryByRole("slider", { name: "Playback position" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Volume" })).toHaveValue("100");
    expect(screen.getByText("0:12 / 1:00")).toBeInTheDocument();
  });

  it("dispatches button and volume interactions", () => {
    render(<PlayerTransport {...props} />);

    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    fireEvent.change(screen.getByRole("slider", { name: "Volume" }), {
      target: { value: "50" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Mute" }));

    expect(props.onTogglePlayPause).toHaveBeenCalledOnce();
    expect(props.onStop).toHaveBeenCalledOnce();
    expect(props.onVolume).toHaveBeenCalledWith(0.5);
    expect(props.onToggleMute).toHaveBeenCalledOnce();
  });

  it("disables unavailable controls and shows errors", () => {
    render(
      <PlayerTransport
        {...props}
        status="idle"
        hasSelection={false}
        durationMs={null}
        canPrevious={false}
        canNext={false}
        error="Playback failed."
      />,
    );

    expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Play" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Playback failed.");
  });
});
