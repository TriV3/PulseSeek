import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useKeyboardShortcuts } from "./useKeyboardShortcuts";

function press(
  key: string,
  options: KeyboardEventInit = {},
  target: EventTarget = window,
) {
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    key,
    ...options,
  });
  Object.defineProperty(event, "target", { configurable: true, value: target });
  target.dispatchEvent(event);
  return event;
}

function createActions() {
  return {
    onOpenFolder: vi.fn(),
    onTogglePlayPause: vi.fn(),
    onPreviousTrack: vi.fn(),
    onNextTrack: vi.fn(),
    onSeekBackward: vi.fn(),
    onSeekForward: vi.fn(),
    onToggleLoop: vi.fn(),
    onMoveToTrash: vi.fn(),
    onFocusSearch: vi.fn(),
    onMarkKeep: vi.fn(),
  };
}

describe("useKeyboardShortcuts", () => {
  let actions: ReturnType<typeof createActions>;

  beforeEach(() => {
    actions = createActions();
  });

  afterEach(() => {
    document.body.replaceChildren();
    Object.defineProperty(navigator, "platform", {
      configurable: true,
      value: "Linux",
    });
  });

  it("dispatches primary shortcuts", () => {
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press("o", { ctrlKey: true });
      press(" ");
      press("ArrowLeft");
      press("ArrowRight");
      press("ArrowLeft", { ctrlKey: true });
      press("ArrowRight", { ctrlKey: true });
      press("l");
      press("Delete");
    });

    expect(actions.onOpenFolder).toHaveBeenCalledOnce();
    expect(actions.onTogglePlayPause).toHaveBeenCalledOnce();
    expect(actions.onSeekBackward).toHaveBeenCalledOnce();
    expect(actions.onSeekForward).toHaveBeenCalledOnce();
    expect(actions.onPreviousTrack).toHaveBeenCalledOnce();
    expect(actions.onNextTrack).toHaveBeenCalledOnce();
    expect(actions.onToggleLoop).toHaveBeenCalledOnce();
    expect(actions.onMoveToTrash).toHaveBeenCalledOnce();
  });

  it("uses Meta as the platform modifier on macOS", () => {
    Object.defineProperty(navigator, "platform", {
      configurable: true,
      value: "MacIntel",
    });
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press("o", { metaKey: true });
      press("ArrowLeft", { metaKey: true });
      press("ArrowRight", { ctrlKey: true });
    });

    expect(actions.onOpenFolder).toHaveBeenCalledOnce();
    expect(actions.onPreviousTrack).toHaveBeenCalledOnce();
    expect(actions.onNextTrack).not.toHaveBeenCalled();
  });

  it("ignores editable controls", () => {
    const input = document.createElement("input");
    const textarea = document.createElement("textarea");
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    document.body.append(input, textarea, editable);
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press(" ", {}, input);
      press("l", {}, textarea);
      press("o", { ctrlKey: true }, input);
      press("Delete", {}, editable);
    });

    expect(actions.onOpenFolder).not.toHaveBeenCalled();
    expect(actions.onTogglePlayPause).not.toHaveBeenCalled();
    expect(actions.onToggleLoop).not.toHaveBeenCalled();
    expect(actions.onMoveToTrash).not.toHaveBeenCalled();
  });

  it("leaves file-grid navigation to the grid", () => {
    const grid = document.createElement("div");
    grid.setAttribute("role", "grid");
    const row = document.createElement("div");
    grid.append(row);
    document.body.append(grid);
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press("ArrowLeft", {}, row);
      press("ArrowRight", {}, row);
    });

    expect(actions.onSeekBackward).not.toHaveBeenCalled();
    expect(actions.onSeekForward).not.toHaveBeenCalled();
  });

  it("allows modified mark shortcuts while grid has focus", () => {
    const grid = document.createElement("div");
    grid.setAttribute("role", "grid");
    const row = document.createElement("div");
    grid.append(row);
    document.body.append(grid);
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press("k", { ctrlKey: true, shiftKey: true }, row);
    });

    expect(actions.onMarkKeep).toHaveBeenCalledOnce();
  });

  it("ignores composing and already-handled events", () => {
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press(" ", { isComposing: true });
      const event = new KeyboardEvent("keydown", {
        bubbles: true,
        cancelable: true,
        key: "l",
      });
      event.preventDefault();
      window.dispatchEvent(event);
    });

    expect(actions.onTogglePlayPause).not.toHaveBeenCalled();
    expect(actions.onToggleLoop).not.toHaveBeenCalled();
  });

  it("suppresses widgets and modals", () => {
    const button = document.createElement("button");
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    const child = document.createElement("div");
    dialog.append(child);
    document.body.append(button, dialog);
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press(" ", {}, button);
      press(" ", {}, child);
      press("f", { ctrlKey: true }, child);
      press("l");
    });

    expect(actions.onTogglePlayPause).not.toHaveBeenCalled();
    expect(actions.onTogglePlayPause).not.toHaveBeenCalled();
    expect(actions.onFocusSearch).not.toHaveBeenCalled();
  });

  it("allows only focus-search in editable fields", () => {
    const input = document.createElement("input");
    document.body.append(input);
    renderHook(() => useKeyboardShortcuts(actions));

    act(() => {
      press("f", { ctrlKey: true }, input);
      press("l", {}, input);
    });

    expect(actions.onFocusSearch).toHaveBeenCalledOnce();
    expect(actions.onToggleLoop).not.toHaveBeenCalled();
  });
});
