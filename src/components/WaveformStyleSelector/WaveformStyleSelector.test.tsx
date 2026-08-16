import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WaveformStyleSelector } from "./WaveformStyleSelector";

describe("WaveformStyleSelector", () => {
  it("renders every waveform style option", () => {
    render(<WaveformStyleSelector style="outline" onChange={() => {}} />);

    const select = screen.getByLabelText("Waveform style");
    expect(select).toHaveValue("outline");
    expect(screen.getByRole("option", { name: "Solid" })).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Gradient" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Outline" })).toBeInTheDocument();
  });

  it("reports style changes", () => {
    const onChange = vi.fn();
    render(<WaveformStyleSelector style="outline" onChange={onChange} />);

    fireEvent.change(screen.getByLabelText("Waveform style"), {
      target: { value: "solid" },
    });

    expect(onChange).toHaveBeenCalledWith("solid");
  });

  it("reports the gradient style change", () => {
    const onChange = vi.fn();
    render(<WaveformStyleSelector style="outline" onChange={onChange} />);

    fireEvent.change(screen.getByLabelText("Waveform style"), {
      target: { value: "gradient" },
    });

    expect(onChange).toHaveBeenCalledWith("gradient");
  });
});
