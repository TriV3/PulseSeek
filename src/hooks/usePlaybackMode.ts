import { useRef, useState } from "react";
import { setPlaybackMode, type PlaybackMode } from "../api/commandEnvelope";

export function usePlaybackMode() {
  const [mode, setMode] = useState<PlaybackMode>("one-shot");
  const [error, setError] = useState<string | null>(null);
  const [isChanging, setIsChanging] = useState(false);
  const generation = useRef(0);

  async function selectMode(
    nextMode: PlaybackMode,
  ): Promise<PlaybackMode | null> {
    const requestGeneration = ++generation.current;
    const previousMode = mode;
    setMode(nextMode);
    setError(null);
    setIsChanging(true);
    try {
      const confirmedMode = await setPlaybackMode(nextMode);
      if (requestGeneration === generation.current) {
        setMode(confirmedMode);
        return confirmedMode;
      }
      return null;
    } catch (cause: unknown) {
      if (requestGeneration === generation.current) {
        setMode(previousMode);
        setError(cause instanceof Error ? cause.message : "Mode unavailable.");
      }
      return null;
    } finally {
      if (requestGeneration === generation.current) setIsChanging(false);
    }
  }

  return { mode, error, isChanging, selectMode };
}
