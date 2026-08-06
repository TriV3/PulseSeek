import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MoveDialog } from "./MoveDialog";

const baseProps = {
  open: true,
  title: "Move Files",
  fileNameCount: 2,
  targetDir: null,
  onPickTarget: vi.fn(),
  onConfirm: vi.fn(),
  onCancel: vi.fn(),
};

describe("MoveDialog", () => {
  it("prompts for a target folder before confirming", () => {
    render(<MoveDialog {...baseProps} />);

    expect(
      screen.getByRole("button", { name: "Choose folder…" }),
    ).toBeInTheDocument();
    expect(screen.getByText("No folder selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Move" })).toBeDisabled();
  });

  it("reports the selected target and enables confirm", () => {
    render(<MoveDialog {...baseProps} targetDir="/library" />);

    expect(screen.getByText("/library")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Move" })).not.toBeDisabled();
  });

  it("shows the file count for multi-selection", () => {
    render(<MoveDialog {...baseProps} fileNameCount={5} />);
    expect(screen.getByText("Move 5 files into a folder.")).toBeInTheDocument();
  });

  it("picks a folder through the pick action", () => {
    const onPickTarget = vi.fn();
    render(<MoveDialog {...baseProps} onPickTarget={onPickTarget} />);

    fireEvent.click(screen.getByRole("button", { name: "Choose folder…" }));
    expect(onPickTarget).toHaveBeenCalledTimes(1);
  });

  it("confirms the move", () => {
    const onConfirm = vi.fn();
    render(
      <MoveDialog {...baseProps} targetDir="/library" onConfirm={onConfirm} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Move" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("shows progress while busy and disables actions", () => {
    const onCancel = vi.fn();
    render(
      <MoveDialog
        {...baseProps}
        targetDir="/library"
        busy
        progress={{ completed: 1, total: 3 }}
        onCancel={onCancel}
      />,
    );

    expect(screen.getByText("Moving file 1 of 3…")).toBeInTheDocument();
    expect(screen.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "1",
    );
    expect(screen.getByRole("button", { name: "Moving…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).not.toBeDisabled();
  });

  it("cancels a running batch", () => {
    const onCancel = vi.fn();
    render(
      <MoveDialog
        {...baseProps}
        targetDir="/library"
        busy
        progress={{ completed: 1, total: 3 }}
        onCancel={onCancel}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("reports successful and failed targets separately", () => {
    render(
      <MoveDialog
        {...baseProps}
        targetDir="/library"
        summary={{
          okCount: 1,
          failed: [
            {
              path: "/music/b.wav",
              ok: false,
              category: "Conflict",
              message: "PulseSeek could not apply that change.",
              diagnostic_code: "file.operation",
            },
          ],
        }}
      />,
    );

    expect(screen.getByText("1 file moved.")).toBeInTheDocument();
    expect(screen.getByText("1 file could not be moved:")).toBeInTheDocument();
    expect(
      screen.getByText("PulseSeek could not apply that change."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Move" })).toBeDisabled();
  });

  it("renders a backend error with a live alert", () => {
    render(<MoveDialog {...baseProps} error="That folder is unavailable." />);

    expect(screen.getByRole("alert")).toHaveTextContent(
      "That folder is unavailable.",
    );
  });

  it("cancels on Escape", () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    render(
      <MoveDialog
        {...baseProps}
        targetDir="/library"
        onCancel={onCancel}
        onConfirm={onConfirm}
      />,
    );

    fireEvent.keyDown(screen.getByRole("alertdialog"), { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("restores focus to the trigger when closed", () => {
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const { rerender } = render(<MoveDialog {...baseProps} open />);
    rerender(<MoveDialog {...baseProps} open={false} />);

    expect(trigger).toHaveFocus();
    trigger.remove();
  });

  it("renders nothing when closed", () => {
    const { container } = render(<MoveDialog {...baseProps} open={false} />);
    expect(container).toBeEmptyDOMElement();
  });
});
