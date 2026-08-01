import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useTheme } from "./useTheme";

interface FakeMediaQueryList {
  matches: boolean;
  media: string;
  listeners: Array<() => void>;
  addEventListener: (type: string, listener: () => void) => void;
  removeEventListener: (type: string, listener: () => void) => void;
}

function installMatchMedia(matches: boolean): FakeMediaQueryList {
  const media: FakeMediaQueryList = {
    matches,
    media: "(prefers-color-scheme: dark)",
    listeners: [],
    addEventListener(_type, listener) {
      media.listeners.push(listener);
    },
    removeEventListener(_type, listener) {
      media.listeners = media.listeners.filter((entry) => entry !== listener);
    },
  };
  window.matchMedia = (() => media) as unknown as typeof window.matchMedia;
  return media;
}

afterEach(() => {
  delete document.documentElement.dataset.theme;
});

describe("useTheme", () => {
  it("applies light for the light preference", () => {
    renderHook(() => useTheme("light"));
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("applies dark for the dark preference", () => {
    renderHook(() => useTheme("dark"));
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("applies midnight for the midnight preference", () => {
    renderHook(() => useTheme("midnight"));
    expect(document.documentElement.dataset.theme).toBe("midnight");
  });

  it("resolves the system preference from the OS color scheme", () => {
    installMatchMedia(true);
    renderHook(() => useTheme("system"));
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("updates live when the OS color scheme changes", () => {
    const media = installMatchMedia(false);
    renderHook(() => useTheme("system"));
    expect(document.documentElement.dataset.theme).toBe("light");

    act(() => {
      media.matches = true;
      for (const listener of media.listeners) listener();
    });

    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("does not listen to the OS when a concrete theme is chosen", () => {
    const media = installMatchMedia(false);
    renderHook(() => useTheme("light"));
    expect(media.listeners).toHaveLength(0);
  });

  it("switches without restart when the preference changes", () => {
    installMatchMedia(false);
    type Preference = "system" | "light" | "dark";
    const { rerender } = renderHook(
      ({ preference }: { preference: Preference }) => useTheme(preference),
      { initialProps: { preference: "system" as Preference } },
    );
    expect(document.documentElement.dataset.theme).toBe("light");

    rerender({ preference: "dark" });
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
