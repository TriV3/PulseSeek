import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AudioDeviceSelector } from "./AudioDeviceSelector";

const props = {
  devices: [
    { id: "a", name: "Speakers" },
    { id: "b", name: "HDMI" },
  ],
  selectedDeviceId: "a",
  isLoading: false,
  isSelecting: false,
  error: null,
  onChange: vi.fn(),
  onRetry: vi.fn(),
};
describe("AudioDeviceSelector", () => {
  it("shows and changes selected device", () => {
    render(<AudioDeviceSelector {...props} />);
    const select = screen.getByRole("combobox", { name: "Output device" });
    expect(select).toHaveValue("a");
    fireEvent.change(select, { target: { value: "b" } });
    expect(props.onChange).toHaveBeenCalledWith("b");
  });
  it("shows retry for no-device and failure states", () => {
    render(
      <AudioDeviceSelector
        {...props}
        devices={[]}
        selectedDeviceId={null}
        error="No device"
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("No device");
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("announces loading and associates errors with selector", () => {
    render(
      <AudioDeviceSelector {...props} isLoading error="Device unavailable" />,
    );
    const selector = screen.getByRole("combobox", { name: "Output device" });
    expect(selector).toBeDisabled();
    expect(selector).toHaveAttribute(
      "aria-describedby",
      "audio-output-device-status",
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Loading output devices",
    );
  });
});
