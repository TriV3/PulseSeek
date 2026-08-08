import { useCallback, useEffect, useRef, useState } from "react";
import {
  loadPlayerPreferences,
  savePlayerPreferences,
  type PlayerPreferences,
} from "../api/commandEnvelope";

export const DEFAULT_PLAYER_PREFERENCES: PlayerPreferences = {
  schema_version: 1,
  revision: 0,
  playback_mode: "one-shot",
  output_device_id: null,
  volume: 1,
  muted: false,
  waveform_size: 38,
  browser_size: 24,
  selected_folder_path: null,
  expanded_folder_paths: [],
  last_played_file_path: null,
  last_played_position_ms: 0,
  last_played_duration_ms: null,
  theme: "system",
  waveform_style: "outline",
  show_hidden_folders: false,
};

export function usePlayerPreferences() {
  const [preferences, setPreferences] = useState(DEFAULT_PLAYER_PREFERENCES);
  const [isLoaded, setIsLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = useRef(DEFAULT_PLAYER_PREFERENCES);

  useEffect(() => {
    let active = true;
    void loadPlayerPreferences()
      .then((loaded) => {
        if (!active) return;
        current.current = loaded;
        setPreferences(loaded);
      })
      .catch((cause: unknown) => {
        if (active) {
          setError(
            cause instanceof Error
              ? cause.message
              : "Player preferences unavailable.",
          );
        }
      })
      .finally(() => {
        if (active) setIsLoaded(true);
      });
    return () => {
      active = false;
    };
  }, []);

  const update = useCallback((change: Partial<PlayerPreferences>) => {
    const next: PlayerPreferences = {
      ...current.current,
      ...change,
      schema_version: 1,
      revision: current.current.revision + 1,
    };
    current.current = next;
    setPreferences(next);
    setError(null);
    void savePlayerPreferences(next)
      .then((confirmed) => {
        if (confirmed.revision < current.current.revision) return;
        current.current = confirmed;
        setPreferences(confirmed);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "Could not save player preferences.",
        );
      });
  }, []);

  return { preferences, isLoaded, error, update };
}
