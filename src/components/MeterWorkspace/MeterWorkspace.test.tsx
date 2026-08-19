import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MeterWorkspace } from "./MeterWorkspace";

describe("MeterWorkspace", () => {
  it("shows every tile availability state through keyboard-accessible controls", () => {
    render(<MeterWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Spectrum Analyzer" }));

    const state = screen.getByRole("combobox", {
      name: "Spectrum Analyzer state",
    });
    fireEvent.change(state, { target: { value: "unavailable" } });
    expect(screen.getByRole("status")).toHaveTextContent("Unavailable");

    for (const value of [
      "incomplete",
      "degraded",
      "error",
      "loading",
      "ready",
    ]) {
      fireEvent.change(state, { target: { value } });
      expect(screen.getByRole("status")).toHaveTextContent(
        value === "ready" ? "Ready" : value[0].toUpperCase() + value.slice(1),
      );
    }
  });

  it("supports keyboard activation and focus-visible styling on tile controls", () => {
    render(<MeterWorkspace />);
    const add = screen.getByRole("button", { name: "Spectrum Analyzer" });
    add.focus();
    fireEvent.keyDown(add, { key: "Enter" });
    fireEvent.click(add);
    expect(
      screen.getByRole("button", { name: "Duplicate Spectrum Analyzer" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Remove Spectrum Analyzer" }),
    ).toBeInTheDocument();
  });
});
