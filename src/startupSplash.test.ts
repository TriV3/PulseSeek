import { afterEach, describe, expect, it, vi } from "vitest";
import { dismissStartupSplash } from "./startupSplash";

describe("dismissStartupSplash", () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("keeps the startup splash visible before fading it from the document", () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="startup-splash"></div>';

    dismissStartupSplash();

    vi.advanceTimersByTime(1_499);

    expect(document.querySelector("#startup-splash")).not.toHaveClass(
      "startup-splash--leaving",
    );

    vi.advanceTimersByTime(1);

    expect(document.querySelector("#startup-splash")).toHaveClass(
      "startup-splash--leaving",
    );

    vi.advanceTimersByTime(220);

    expect(document.querySelector("#startup-splash")).toBeNull();
  });

  it("does nothing when the splash has already been removed", () => {
    expect(() => dismissStartupSplash()).not.toThrow();
  });
});
