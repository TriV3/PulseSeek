import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_PLAYER_PREFERENCES,
  usePlayerPreferences,
} from "./usePlayerPreferences";

const api = vi.hoisted(() => ({
  load: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../api/commandEnvelope", () => ({
  loadPlayerPreferences: api.load,
  savePlayerPreferences: api.save,
}));

beforeEach(() => {
  vi.resetAllMocks();
  api.load.mockResolvedValue(DEFAULT_PLAYER_PREFERENCES);
  api.save.mockImplementation(async (preferences) => preferences);
});

describe("usePlayerPreferences", () => {
  it("loads the saved track position without restoring transport state", async () => {
    api.load.mockResolvedValueOnce({
      ...DEFAULT_PLAYER_PREFERENCES,
      playback_mode: "sequential",
      volume: 0.42,
      selected_folder_path: "/music/album",
      last_played_file_path: "/music/album/track.wav",
      last_played_position_ms: 42_500,
      last_played_duration_ms: 180_000,
    });
    const { result } = renderHook(() => usePlayerPreferences());

    await waitFor(() => expect(result.current.isLoaded).toBe(true));

    expect(result.current.preferences).toMatchObject({
      playback_mode: "sequential",
      volume: 0.42,
      last_played_file_path: "/music/album/track.wav",
      last_played_position_ms: 42_500,
      last_played_duration_ms: 180_000,
    });
    expect(result.current.preferences).not.toHaveProperty("transport_state");
  });

  it("writes every interaction immediately with an increasing revision", async () => {
    const { result } = renderHook(() => usePlayerPreferences());
    await waitFor(() => expect(result.current.isLoaded).toBe(true));

    act(() => result.current.update({ volume: 0.5 }));
    act(() => result.current.update({ muted: true }));

    expect(api.save).toHaveBeenCalledTimes(2);
    expect(api.save.mock.calls[0][0]).toMatchObject({
      volume: 0.5,
      revision: 1,
    });
    expect(api.save.mock.calls[1][0]).toMatchObject({
      volume: 0.5,
      muted: true,
      revision: 2,
    });
  });

  it("defaults the theme preference to system", () => {
    expect(DEFAULT_PLAYER_PREFERENCES.theme).toBe("system");
  });

  it("persists theme changes immediately", async () => {
    const { result } = renderHook(() => usePlayerPreferences());
    await waitFor(() => expect(result.current.isLoaded).toBe(true));

    act(() => result.current.update({ theme: "dark" }));

    expect(api.save).toHaveBeenCalledTimes(1);
    expect(api.save.mock.calls[0][0]).toMatchObject({
      theme: "dark",
      revision: 1,
    });
  });
});
