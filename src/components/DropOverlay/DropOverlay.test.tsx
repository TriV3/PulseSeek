import { render, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { DropOverlay } from "./DropOverlay";
import "./DropOverlay.css";

describe("DropOverlay", () => {
  it("renders the drop message while active", () => {
    render(<DropOverlay active />);
    expect(
      screen.getByText("Drop files to play or reveal"),
    ).toBeInTheDocument();
  });

  it("renders nothing when inactive", () => {
    const { container } = render(<DropOverlay active={false} />);
    expect(container.querySelector(".drop-overlay")).toBeNull();
  });

  it("never blocks pointer interaction", () => {
    render(<DropOverlay active />);
    const overlay = screen.getByText("Drop files to play or reveal")
      .parentElement?.parentElement;
    expect(overlay).not.toBeNull();
    expect(overlay).toHaveClass("drop-overlay");
    // jsdom does not apply stylesheets; assert the authored rule directly.
    const cssPath = resolve(
      process.cwd(),
      "src/components/DropOverlay/DropOverlay.css",
    );
    expect(readFileSync(cssPath, "utf8")).toContain("pointer-events: none;");
  });

  it("is hidden from assistive technology", () => {
    render(<DropOverlay active />);
    const badge = screen.getByText("Drop files to play or reveal");
    expect(badge.parentElement?.parentElement).toHaveAttribute(
      "aria-hidden",
      "true",
    );
  });
});
