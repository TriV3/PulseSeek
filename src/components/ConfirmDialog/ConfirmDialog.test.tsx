import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

describe("ConfirmDialog", () => {
  it("renders title and message when open", () => {
    render(
      <ConfirmDialog
        open={true}
        title="Move to Trash?"
        message="Are you sure?"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByText("Move to Trash?")).toBeInTheDocument();
    expect(screen.getByText("Are you sure?")).toBeInTheDocument();
  });

  it("calls onConfirm when confirm button is clicked", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog
        open={true}
        title="Confirm"
        message="Proceed?"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when cancel button is clicked", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        open={true}
        title="Confirm"
        message="Proceed?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );

    fireEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when Escape is pressed", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmDialog
        open={true}
        title="Confirm"
        message="Proceed?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("has accessible alertdialog role", () => {
    render(
      <ConfirmDialog
        open={true}
        title="Move to Trash?"
        message="Are you sure?"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  });

  it("uses custom button labels", () => {
    render(
      <ConfirmDialog
        open={true}
        title="Delete?"
        message="Really?"
        confirmLabel="Yes, delete"
        cancelLabel="Keep it"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByText("Yes, delete")).toBeInTheDocument();
    expect(screen.getByText("Keep it")).toBeInTheDocument();
  });
});
