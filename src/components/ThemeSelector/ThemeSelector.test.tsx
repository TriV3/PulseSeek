import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ThemeSelector } from "./ThemeSelector";

describe("ThemeSelector", () => {
  it("renders every theme option", () => {
    render(<ThemeSelector theme="system" onChange={() => {}} />);

    const select = screen.getByLabelText("Theme");
    expect(select).toHaveValue("system");
    expect(screen.getByRole("option", { name: "System" })).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "PulseSeek Light" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "PulseSeek Dark" }),
    ).toBeInTheDocument();
  });

  it("reports theme changes", () => {
    const onChange = vi.fn();
    render(<ThemeSelector theme="system" onChange={onChange} />);

    fireEvent.change(screen.getByLabelText("Theme"), {
      target: { value: "dark" },
    });

    expect(onChange).toHaveBeenCalledWith("dark");
  });
});
