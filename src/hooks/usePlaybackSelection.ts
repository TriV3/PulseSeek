import { useRef, useState } from "react";
import { play } from "../api/commandEnvelope";
import type { BrowserEntry } from "../components/FolderTree/folderTreeTypes";

export type PlaybackSelectionStatus = "idle" | "loading" | "playing" | "failed";

export interface PlaybackSelection {
  entryId: string | null;
  status: PlaybackSelectionStatus;
  error: string | null;
}

const INITIAL_PLAYBACK: PlaybackSelection = {
  entryId: null,
  status: "idle",
  error: null,
};

export function usePlaybackSelection() {
  const [playback, setPlayback] = useState<PlaybackSelection>(INITIAL_PLAYBACK);
  const generation = useRef(0);

  async function select(entry: BrowserEntry) {
    const requestGeneration = ++generation.current;
    setPlayback({ entryId: entry.id, status: "loading", error: null });

    try {
      await play(entry.id);
      if (requestGeneration === generation.current) {
        setPlayback({ entryId: entry.id, status: "playing", error: null });
      }
    } catch (error: unknown) {
      if (requestGeneration !== generation.current) return;
      setPlayback({
        entryId: entry.id,
        status: "failed",
        error: error instanceof Error ? error.message : "Unable to play file.",
      });
    }
  }

  return { playback, select };
}
