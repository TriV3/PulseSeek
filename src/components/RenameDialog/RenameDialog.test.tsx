import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RenameDialog } from "./RenameDialog";

const baseProps = {
  open: true,
  title: "Rename File",
  initialName: "song1.mp3",
  onConfirm: vi.fn(),
  onCancel: vi.fn(),
};

describe("RenameDialog", () => {
  it("focuses and selects the name field when opened", () => {
    render(<RenameDialog {...baseProps} />);

    const input = screen.getByLabelText("New file name") as HTMLInputElement;
    expect(input).toHaveFocus();
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe("song1.mp3".length);
  });

  it("submits the trimmed name on Enter", () => {
    const onConfirm = vi.fn();
    render(<RenameDialog {...baseProps} onConfirm={onConfirm} />);

    const input = screen.getByLabelText("New file name");
    fireEvent.change(input, { target: { value: " renamed.wav " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onConfirm).toHaveBeenCalledWith("renamed.wav");
  });

  it("submits through the confirm button", () => {
    const onConfirm = vi.fn();
    render(<RenameDialog {...baseProps} onConfirm={onConfirm} />);

    const input = screen.getByLabelText("New file name");
    fireEvent.change(input, { target: { value: "renamed.wav" } });
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));

    expect(onConfirm).toHaveBeenCalledWith("renamed.wav");
  });

  it("cancels on Escape without submitting", () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    render(
      <RenameDialog {...baseProps} onCancel={onCancel} onConfirm={onConfirm} />,
    );

    fireEvent.keyDown(screen.getByLabelText("New file name"), {
      key: "Escape",
    });

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("disables confirm while the name is empty", () => {
    render(<RenameDialog {...baseProps} />);

    fireEvent.change(screen.getByLabelText("New file name"), {
      target: { value: "   " },
    });

    expect(screen.getByRole("button", { name: "Rename" })).toBeDisabled();
  });

  it("renders a backend error with a live alert", () => {
    render(<RenameDialog {...baseProps} error="That name is already taken." />);

    expect(screen.getByRole("alert")).toHaveTextContent(
      "That name is already taken.",
    );
    expect(screen.getByLabelText("New file name")).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  });

  it("disables controls and labels confirm while busy", () => {
    render(<RenameDialog {...baseProps} busy />);

    expect(screen.getByRole("button", { name: "Renaming…" })).toBeDisabled();
    expect(screen.getByLabelText("New file name")).toBeDisabled();
  });

  it("restores focus to the trigger when closed", () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const { rerender } = render(<RenameDialog {...baseProps} open={true} />);
    rerender(<RenameDialog {...baseProps} open={false} />);

    expect(trigger).toHaveFocus();
    trigger.remove();
  });

  it("renders nothing when closed", () => {
    const { container } = render(<RenameDialog {...baseProps} open={false} />);
    expect(container).toBeEmptyDOMElement();
  });
});
