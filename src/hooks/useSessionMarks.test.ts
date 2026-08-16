import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useSessionMarks } from "./useSessionMarks";

describe("useSessionMarks", () => {
  it("starts with no marks", () => {
    const { result } = renderHook(() => useSessionMarks());
    expect(result.current.marks).toEqual({});
  });

  it("applies a mark to every requested id", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3", "b.wav"], "keep"));
    expect(result.current.marks).toEqual({
      "a.mp3": "keep",
      "b.wav": "keep",
    });
  });

  it("replaces an existing mark with the new one", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3"], "keep"));
    act(() => result.current.setMark(["a.mp3"], "reject"));
    expect(result.current.marks).toEqual({ "a.mp3": "reject" });
  });

  it("unmarks only the requested ids", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3", "b.wav"], "keep"));
    act(() => result.current.unmark(["a.mp3"]));
    expect(result.current.marks).toEqual({ "b.wav": "keep" });
  });

  it("clears every mark", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3", "b.wav"], "keep"));
    act(() => result.current.clear());
    expect(result.current.marks).toEqual({});
  });

  it("moves a mark to a renamed id", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3"], "favorite"));
    act(() => result.current.reconcile("a.mp3", "a-renamed.mp3"));
    expect(result.current.marks).toEqual({ "a-renamed.mp3": "favorite" });
  });

  it("leaves marks untouched when the old id is unknown", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3"], "keep"));
    act(() => result.current.reconcile("b.wav", "b-renamed.wav"));
    expect(result.current.marks).toEqual({ "a.mp3": "keep" });
  });

  it("replaces the whole mark set after external reconciliation", () => {
    const { result } = renderHook(() => useSessionMarks());
    act(() => result.current.setMark(["a.mp3"], "keep"));
    act(() => result.current.replace({ "b.mp3": "keep" }));
    expect(result.current.marks).toEqual({ "b.mp3": "keep" });
  });
});
