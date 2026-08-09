import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  VisualizationSelector,
  VisualizationSettingsControls,
} from "./VisualizationSelector";

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

  it("exposes enabled and quality controls with accessible labels", () => {
    const onEnabledChange = vi.fn();
    const onQualityChange = vi.fn();
    render(
      <VisualizationSettingsControls
        enabled
        quality="balanced"
        onEnabledChange={onEnabledChange}
        onQualityChange={onQualityChange}
      />,
    );

    fireEvent.click(
      screen.getByRole("checkbox", { name: "Real-time visualizations" }),
    );
    expect(onEnabledChange).toHaveBeenCalledWith(false);

    fireEvent.change(screen.getByLabelText("Visualization quality"), {
      target: { value: "high" },
    });
    expect(onQualityChange).toHaveBeenCalledWith("high");
    expect(screen.getByText("Real-time visualizations")).toBeVisible();
  });
});
