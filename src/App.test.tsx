import { render, screen } from "@testing-library/react";
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

    // The tree is rendered with an "Open Folder" button.
    expect(
      screen.getByRole("button", { name: "Open Folder" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "File list" }),
    ).toBeInTheDocument();
  });
});
