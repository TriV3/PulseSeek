import { renderHook, act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAudioDevices } from "./useAudioDevices";

const listMock = vi.hoisted(() => vi.fn());
const currentMock = vi.hoisted(() => vi.fn());
const selectMock = vi.hoisted(() => vi.fn());
const lostMock = vi.hoisted(() => vi.fn());
vi.mock("../api/commandEnvelope", () => ({
  listDevices: listMock,
  currentDevice: currentMock,
  selectDevice: selectMock,
}));
vi.mock("../api/playbackEvents", () => ({ onDeviceLost: lostMock }));

beforeEach(() => {
  vi.resetAllMocks();
  listMock.mockResolvedValue([{ id: "default", name: "Default" }]);
  currentMock.mockResolvedValue({ id: "default", name: "Default" });
  lostMock.mockResolvedValue(() => undefined);
});

describe("useAudioDevices", () => {
  it("loads default device", async () => {
    const { result } = renderHook(() => useAudioDevices());
    await vi.waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.selectedDeviceId).toBe("default");
  });

  it("rolls back failed selection", async () => {
    selectMock.mockRejectedValue(new Error("device missing"));
    const { result } = renderHook(() => useAudioDevices());
    await vi.waitFor(() => expect(result.current.isLoading).toBe(false));
    let confirmed: string | null = "unexpected";
    await act(async () => {
      confirmed = await result.current.choose("missing");
    });
    expect(result.current.selectedDeviceId).toBe("default");
    expect(result.current.error).toBe("device missing");
    expect(confirmed).toBeNull();
  });

  it("reflects native fallback device after selection", async () => {
    currentMock
      .mockResolvedValueOnce({ id: "default", name: "Default" })
      .mockResolvedValue({ id: "default", name: "Default" });
    selectMock.mockResolvedValue(undefined);
    const { result } = renderHook(() => useAudioDevices());
    await vi.waitFor(() => expect(result.current.isLoading).toBe(false));

    await act(async () => result.current.choose("missing"));

    expect(selectMock).toHaveBeenCalledWith("missing");
    expect(result.current.selectedDeviceId).toBe("default");
  });
});
