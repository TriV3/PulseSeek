import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_SHORTCUTS,
  type ShortcutBindings,
} from "../../shortcuts/keyboardShortcuts";
import { ShortcutEditor } from "./ShortcutEditor";

const baseProps = {
  open: true,
  bindings: DEFAULT_SHORTCUTS,
  platform: "linux" as const,
  onSave: vi.fn(),
  onReset: vi.fn(),
  onCancel: vi.fn(),
};

describe("ShortcutEditor", () => {
  it("captures a draft and saves only after explicit Save", () => {
    const onSave = vi.fn();
    render(<ShortcutEditor {...baseProps} onSave={onSave} />);

    const openFolder = screen.getByRole("button", {
      name: /change shortcut for open folder/i,
    });
    fireEvent.click(openFolder);
    fireEvent.keyDown(openFolder, { key: "p", ctrlKey: true });

    expect(onSave).not.toHaveBeenCalled();
    expect(openFolder).toHaveTextContent("Ctrl+P");

    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        open_folder: { key: "p", primary: true, shift: false, alt: false },
      }),
    );
  });

  it("blocks Save with live conflict and reserved-key errors", () => {
    render(<ShortcutEditor {...baseProps} />);

    const openFolder = screen.getByRole("button", {
      name: /change shortcut for open folder/i,
    });
    fireEvent.click(openFolder);
    fireEvent.keyDown(openFolder, { key: "f", ctrlKey: true });

    expect(screen.getByRole("alert")).toHaveTextContent(
      /conflicts with Search/i,
    );
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

    fireEvent.click(openFolder);
    fireEvent.keyDown(openFolder, { key: "F4", altKey: true });
    expect(screen.getByRole("alert")).toHaveTextContent(/reserved/i);
  });

  it("resets through backend immediately, cancels, and shows editable A-B rows", async () => {
    const onCancel = vi.fn();
    const onSave = vi.fn();
    const onReset = vi.fn(async () => DEFAULT_SHORTCUTS);
    render(
      <ShortcutEditor
        {...baseProps}
        bindings={{
          ...DEFAULT_SHORTCUTS,
          open_folder: { key: "p", primary: true, shift: false, alt: false },
        }}
        onCancel={onCancel}
        onSave={onSave}
        onReset={onReset}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    await waitFor(() => expect(onReset).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(
        screen.getByRole("button", {
          name: /change shortcut for open folder/i,
        }),
      ).toHaveTextContent("Ctrl+O"),
    );
    expect(
      screen.getByRole("button", {
        name: /change shortcut for set a point/i,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: /change shortcut for set b point/i,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: /change shortcut for toggle a-b repeat/i,
      }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Unavailable")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("shows saving and safe operation errors", async () => {
    let rejectSave: (error: Error) => void = () => {};
    const onSave = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectSave = reject;
        }),
    );
    render(<ShortcutEditor {...baseProps} onSave={onSave} />);

    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByRole("button", { name: "Saving…" })).toBeDisabled();
    rejectSave(new Error("Could not save keyboard shortcuts."));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Could not save keyboard shortcuts.",
      ),
    );
  });

  it("shows reset progress and reset errors without changing draft", async () => {
    let rejectReset: (error: Error) => void = () => {};
    const onReset = vi.fn(
      () =>
        new Promise<ShortcutBindings>((_resolve, reject) => {
          rejectReset = reject;
        }),
    );
    render(
      <ShortcutEditor
        {...baseProps}
        bindings={{
          ...DEFAULT_SHORTCUTS,
          open_folder: { key: "p", primary: true, shift: false, alt: false },
        }}
        onReset={onReset}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(screen.getByRole("button", { name: "Resetting…" })).toBeDisabled();
    rejectReset(new Error("Could not reset keyboard shortcuts."));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Could not reset keyboard shortcuts.",
      ),
    );
    expect(
      screen.getByRole("button", { name: /change shortcut for open folder/i }),
    ).toHaveTextContent("Ctrl+P");
  });

  it("traps focus, cancels with Escape, and restores prior focus", () => {
    const trigger = document.createElement("button");
    document.body.append(trigger);
    trigger.focus();
    const onCancel = vi.fn();
    const { rerender } = render(
      <ShortcutEditor {...baseProps} onCancel={onCancel} />,
    );

    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    const firstShortcut = screen.getByRole("button", {
      name: /change shortcut for open folder/i,
    });
    firstShortcut.focus();
    fireEvent.keyDown(window, { key: "Tab", shiftKey: true });
    expect(screen.getByRole("button", { name: "Save" })).toHaveFocus();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();

    rerender(<ShortcutEditor {...baseProps} open={false} />);
    expect(trigger).toHaveFocus();
    trigger.remove();
  });
});
