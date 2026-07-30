import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PlaybackModeSelector } from "./PlaybackModeSelector";

describe("PlaybackModeSelector", () => {
  it("shows active mode and all choices", () => {
    render(<PlaybackModeSelector mode="loop-current" onChange={vi.fn()} />);

    expect(screen.getByRole("combobox", { name: "Playback mode" })).toHaveValue(
      "loop-current",
    );
    expect(
      screen.getByRole("option", { name: "One shot" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Random" })).toBeInTheDocument();
  });

  it("dispatches keyboard/native select changes", () => {
    const onChange = vi.fn();
    render(<PlaybackModeSelector mode="one-shot" onChange={onChange} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Playback mode" }), {
      target: { value: "random" },
    });

    expect(onChange).toHaveBeenCalledWith("random");
  });

  it("shows command error and disabled state", () => {
    render(
      <PlaybackModeSelector
        mode="one-shot"
        disabled
        error="Mode unavailable."
        onChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("combobox")).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Mode unavailable.");
  });
});
