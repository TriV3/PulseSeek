import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { useFileDrop } from "./useFileDrop";

const mockOnDragDropEvent = vi.hoisted(() => vi.fn());
const mockUnlisten = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: mockOnDragDropEvent }),
}));

function captureHandler(): (event: {
  payload: { type: string; paths?: string[] };
}) => void {
  expect(mockOnDragDropEvent).toHaveBeenCalled();
  return mockOnDragDropEvent.mock.calls[0][0];
}

beforeEach(() => {
  mockOnDragDropEvent.mockReset();
  mockUnlisten.mockReset();
  mockOnDragDropEvent.mockResolvedValue(mockUnlisten);
});

describe("useFileDrop", () => {
  it("subscribes to drag-drop events on mount", async () => {
    renderHook(() => useFileDrop(() => {}));
    await waitFor(() => expect(mockOnDragDropEvent).toHaveBeenCalled());
  });

  it("activates on enter and keeps active on over", async () => {
    const { result } = renderHook(() => useFileDrop(() => {}));
    const handler = captureHandler();

    act(() => handler({ payload: { type: "enter", paths: ["/music/a.wav"] } }));
    expect(result.current.active).toBe(true);

    act(() => handler({ payload: { type: "over" } }));
    expect(result.current.active).toBe(true);
  });

  it("reports dropped paths and deactivates", async () => {
    const onDrop = vi.fn();
    const { result } = renderHook(() => useFileDrop(onDrop));
    const handler = captureHandler();

    act(() => handler({ payload: { type: "enter", paths: ["/music/a.wav"] } }));
    act(() =>
      handler({
        payload: { type: "drop", paths: ["/music/a.wav", "/music/b.wav"] },
      }),
    );

    expect(onDrop).toHaveBeenCalledWith(["/music/a.wav", "/music/b.wav"]);
    expect(result.current.active).toBe(false);
  });

  it("deactivates on leave and cancel", async () => {
    const { result } = renderHook(() => useFileDrop(() => {}));
    const handler = captureHandler();

    act(() => handler({ payload: { type: "enter", paths: ["/music/a.wav"] } }));
    expect(result.current.active).toBe(true);

    act(() => handler({ payload: { type: "leave" } }));
    expect(result.current.active).toBe(false);

    act(() => handler({ payload: { type: "enter", paths: ["/music/a.wav"] } }));
    act(() => handler({ payload: { type: "cancel" } }));
    expect(result.current.active).toBe(false);
  });

  it("suppresses the default dragover and drop so the webview never navigates", () => {
    renderHook(() => useFileDrop(() => {}));

    const preventDefault = vi.fn();
    const fireWindowEvent = (type: "dragover" | "drop") => {
      const event = new Event(type) as Event & { preventDefault: () => void };
      event.preventDefault = preventDefault;
      window.dispatchEvent(event);
    };
    fireWindowEvent("dragover");
    fireWindowEvent("drop");
    expect(preventDefault).toHaveBeenCalledTimes(2);
  });

  it("unsubscribes and removes window listeners on unmount", async () => {
    const { unmount } = renderHook(() => useFileDrop(() => {}));
    await waitFor(() => expect(mockOnDragDropEvent).toHaveBeenCalled());

    unmount();
    await waitFor(() => expect(mockUnlisten).toHaveBeenCalled());

    const preventDefault = vi.fn();
    const event = new Event("drop") as Event & { preventDefault: () => void };
    event.preventDefault = preventDefault;
    window.dispatchEvent(event);
    expect(preventDefault).not.toHaveBeenCalled();
  });

  it("never passes a stale drop callback", async () => {
    const first = vi.fn();
    const second = vi.fn();
    const { result, rerender } = renderHook(
      ({ onDrop }) => useFileDrop(onDrop),
      { initialProps: { onDrop: first } },
    );
    const handler = captureHandler();
    rerender({ onDrop: second });

    act(() => handler({ payload: { type: "drop", paths: ["/music/a.wav"] } }));
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith(["/music/a.wav"]);
    expect(result.current.active).toBe(false);
  });
});
