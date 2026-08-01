import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// jsdom does not implement window.matchMedia. The theme system and its tests
// depend on it, so provide a minimal inert stub that tests can replace.
if (typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as typeof window.matchMedia;
}

// jsdom does not implement ResizeObserver. Components that measure a canvas
// guard against it, and tests replace this inert stub with a triggerable fake.
if (typeof window.ResizeObserver !== "function") {
  class InertResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  window.ResizeObserver =
    InertResizeObserver as unknown as typeof ResizeObserver;
}

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.theme;
});
