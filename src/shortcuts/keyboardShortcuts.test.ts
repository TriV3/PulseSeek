import { describe, expect, it } from "vitest";
import {
  DEFAULT_SHORTCUTS,
  formatShortcut,
  matchShortcut,
  validateShortcutBindings,
  type ShortcutBindings,
  SHORTCUT_ACTIONS,
} from "./keyboardShortcuts";

function keyEvent(key: string, options: KeyboardEventInit = {}) {
  return new KeyboardEvent("keydown", { key, ...options });
}

describe("keyboard shortcuts", () => {
  it("uses the stable Rust action identifiers", () => {
    const ids = SHORTCUT_ACTIONS.map((action) => action.id);

    expect(ids).toContain("mark_clear");
    expect(ids).toContain("toggle_ab_repeat");
    expect(ids).not.toContain("clear_mark");
    expect(ids).not.toContain("clear_ab_repeat");
  });

  it("enables A-B selection with bracket and toggle defaults", () => {
    for (const id of ["set_ab_start", "set_ab_end", "toggle_ab_repeat"]) {
      const entry = SHORTCUT_ACTIONS.find((action) => action.id === id);
      expect(entry?.available, `${id} must be available`).toBe(true);
    }
    expect(DEFAULT_SHORTCUTS.set_ab_start).toEqual({
      key: "[",
      primary: false,
      shift: false,
      alt: false,
    });
    expect(DEFAULT_SHORTCUTS.set_ab_end).toEqual({
      key: "]",
      primary: false,
      shift: false,
      alt: false,
    });
    expect(DEFAULT_SHORTCUTS.toggle_ab_repeat).toEqual({
      key: "a",
      primary: false,
      shift: false,
      alt: false,
    });
    expect(validateShortcutBindings(DEFAULT_SHORTCUTS, "macos")).toEqual({});
    expect(validateShortcutBindings(DEFAULT_SHORTCUTS, "linux")).toEqual({});
  });

  it("matches logical keys and every modifier exactly", () => {
    const binding = { key: "o", primary: true, shift: false, alt: false };

    expect(
      matchShortcut(keyEvent("O", { ctrlKey: true }), binding, "linux"),
    ).toBe(true);
    expect(
      matchShortcut(
        keyEvent("o", { ctrlKey: true, shiftKey: true }),
        binding,
        "linux",
      ),
    ).toBe(false);
    expect(
      matchShortcut(keyEvent("o", { metaKey: true }), binding, "linux"),
    ).toBe(false);
  });

  it("maps primary to Meta on macOS and Ctrl elsewhere", () => {
    const binding = { key: "f", primary: true, shift: false, alt: false };

    expect(
      matchShortcut(keyEvent("f", { metaKey: true }), binding, "macos"),
    ).toBe(true);
    expect(
      matchShortcut(keyEvent("f", { ctrlKey: true }), binding, "macos"),
    ).toBe(false);
    expect(formatShortcut(binding, "macos")).toBe("⌘ F");
    expect(formatShortcut(binding, "linux")).toBe("Ctrl+F");
  });

  it("reports conflicts and reserved operating-system shortcuts", () => {
    const bindings: ShortcutBindings = {
      ...DEFAULT_SHORTCUTS,
      open_folder: { key: "f", primary: true, shift: false, alt: false },
      focus_search: { key: "f", primary: true, shift: false, alt: false },
      refresh: { key: "q", primary: true, shift: false, alt: false },
    };

    const errors = validateShortcutBindings(bindings, "macos");

    expect(errors.open_folder).toMatch(/conflicts with Search/i);
    expect(errors.focus_search).toMatch(/conflicts with Open folder/i);
    expect(errors.refresh).toMatch(/reserved/i);
  });

  it("reserves interface keys except native Enter play selection", () => {
    for (const key of ["tab", "escape", "enter"]) {
      const errors = validateShortcutBindings(
        {
          ...DEFAULT_SHORTCUTS,
          play_selection: {
            key: "p",
            primary: false,
            shift: false,
            alt: false,
          },
          refresh: { key, primary: false, shift: false, alt: false },
        },
        "linux",
      );
      expect(errors.refresh).toMatch(/reserved/i);
    }

    expect(validateShortcutBindings(DEFAULT_SHORTCUTS, "linux")).toEqual({});
  });

  it("reserves primary window commands on Windows and Linux", () => {
    for (const platform of ["windows", "linux"] as const) {
      const errors = validateShortcutBindings(
        {
          ...DEFAULT_SHORTCUTS,
          refresh: { key: "q", primary: true, shift: false, alt: false },
        },
        platform,
      );
      expect(errors.refresh).toMatch(/reserved/i);
    }
  });
});
