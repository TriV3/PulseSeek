import { describe, expect, it } from "vitest";
import { seekStepMs } from "./seekStep";

describe("seekStepMs", () => {
  it("uses selected preset", () => {
    expect(seekStepMs("10s", 60_000)).toBe(10_000);
  });

  it("uses duration-based auto step plateaus", () => {
    expect(seekStepMs("auto", 14_999)).toBe(1_000);
    expect(seekStepMs("auto", 15_000)).toBe(2_000);
    expect(seekStepMs("auto", 59_999)).toBe(2_000);
    expect(seekStepMs("auto", 60_000)).toBe(5_000);
    expect(seekStepMs("auto", 7 * 60_000 - 1)).toBe(5_000);
    expect(seekStepMs("auto", 7 * 60_000)).toBe(10_000);
    expect(seekStepMs("auto", 15 * 60_000)).toBe(15_000);
    expect(seekStepMs("auto", 30 * 60_000)).toBe(20_000);
    expect(seekStepMs("auto", 60 * 60_000)).toBe(20_000);
    expect(seekStepMs("auto", 60 * 60_000 + 1)).toBe(30_000);
  });

  it("never goes below one second", () => {
    expect(seekStepMs("auto", 5_000)).toBe(1_000);
  });

  it("falls back to five seconds without duration", () => {
    expect(seekStepMs("auto", null)).toBe(5_000);
  });
});
