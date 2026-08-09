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
  });
});
