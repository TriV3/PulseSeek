import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { VisualizationSelector } from "./VisualizationSelector";

describe("VisualizationSelector", () => {
  it("selects one exclusive visualization", () => {
    const onChange = vi.fn();
    render(<VisualizationSelector value="waveform" onChange={onChange} />);

    const selector = screen.getByLabelText("Visualization");
    expect(selector).toHaveValue("waveform");

    fireEvent.change(selector, { target: { value: "logarithmic" } });

    expect(onChange).toHaveBeenCalledWith("logarithmic");

    fireEvent.change(selector, { target: { value: "linear" } });

    expect(onChange).toHaveBeenCalledWith("linear");
    expect(screen.getByRole("option", { name: "Linear analyzer" })).toHaveValue(
      "linear",
    );

    fireEvent.change(selector, { target: { value: "musical" } });

    expect(onChange).toHaveBeenCalledWith("musical");
    expect(
      screen.getByRole("option", { name: "Musical spectrum" }),
    ).toHaveValue("musical");
  });
});
