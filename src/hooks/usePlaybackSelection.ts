import { useCallback, useRef, useState } from "react";
import { play, seek } from "../api/commandEnvelope";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";

export type PlaybackSelectionStatus = "idle" | "loading" | "playing" | "failed";

export interface PlaybackSelection {
  entryId: string | null;
  status: PlaybackSelectionStatus;
  error: string | null;
  generation: number;
}

const INITIAL_PLAYBACK: PlaybackSelection = {
  entryId: null,
  status: "idle",
  error: null,
  generation: 0,
};

export function usePlaybackSelection() {
  const [playback, setPlayback] = useState<PlaybackSelection>(INITIAL_PLAYBACK);
  const generation = useRef(0);

  const select = useCallback(
    async (
      entry: BrowserEntry,
      startPositionMs = 0,
      options?: { alreadyPlaying?: boolean },
    ): Promise<boolean> => {
      const requestGeneration = ++generation.current;
      if (options?.alreadyPlaying) {
        setPlayback({
          entryId: entry.id,
          status: "playing",
          error: null,
          generation: requestGeneration,
        });
        return true;
      }
      setPlayback({
        entryId: entry.id,
        status: "loading",
        error: null,
        generation: requestGeneration,
      });

      try {
        if (!options?.alreadyPlaying) {
          await play(entry.id);
          if (startPositionMs > 0) await seek(startPositionMs);
        }
        if (requestGeneration === generation.current) {
          setPlayback({
            entryId: entry.id,
            status: "playing",
            error: null,
            generation: requestGeneration,
          });
          return true;
        }
        return false;
      } catch (error: unknown) {
        if (requestGeneration !== generation.current) return false;
        setPlayback({
          entryId: entry.id,
          status: "failed",
          error:
            error instanceof Error ? error.message : "Unable to play file.",
          generation: requestGeneration,
        });
        return false;
      }
    },
    [],
  );

  const restore = useCallback((entryId: string) => {
    const requestGeneration = ++generation.current;
    setPlayback({
      entryId,
      status: "idle",
      error: null,
      generation: requestGeneration,
    });
  }, []);

  const reconcile = useCallback((oldId: string, newId: string) => {
    setPlayback((current) =>
      current.entryId === oldId
        ? { ...current, entryId: newId, generation: current.generation }
        : current,
    );
  }, []);

  return { playback, select, restore, reconcile };
}
