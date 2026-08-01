import { useEffect, useReducer, useRef } from "react";
import { getWaveform, type WaveformLevel } from "../api/waveform";

export type WaveformStatus = "idle" | "loading" | "ready" | "error";

export interface WaveformState {
  status: WaveformStatus;
  waveform: WaveformLevel | null;
  error: string | null;
}

export const INITIAL_WAVEFORM_STATE: WaveformState = {
  status: "idle",
  waveform: null,
  error: null,
};

type WaveformAction =
  | { type: "reset" }
  | { type: "loading" }
  | { type: "ready"; waveform: WaveformLevel }
  | { type: "error"; message: string };

function waveformReducer(
  _state: WaveformState,
  action: WaveformAction,
): WaveformState {
  switch (action.type) {
    case "reset":
      return INITIAL_WAVEFORM_STATE;
    case "loading":
      return { status: "loading", waveform: null, error: null };
    case "ready":
      return { status: "ready", waveform: action.waveform, error: null };
    case "error":
      return { status: "error", waveform: null, error: action.message };
  }
}

/**
 * Loads the waveform level for the selected file.
 *
 * Re-fetches when the file path or the requested resolution target changes.
 * Stale responses from a previous file are ignored so a quick selection
 * change cannot paint the wrong waveform.
 */
export function useWaveform(entryPath: string | null, targetPeaks: number) {
  const [state, dispatch] = useReducer(waveformReducer, INITIAL_WAVEFORM_STATE);
  const requestId = useRef(0);

  useEffect(() => {
    if (!entryPath || targetPeaks <= 0) {
      requestId.current += 1;
      dispatch({ type: "reset" });
      return;
    }

    const current = ++requestId.current;
    dispatch({ type: "loading" });
    let disposed = false;

    void getWaveform(entryPath, targetPeaks)
      .then((waveform) => {
        if (disposed || current !== requestId.current) return;
        dispatch({ type: "ready", waveform });
      })
      .catch((error: unknown) => {
        if (disposed || current !== requestId.current) return;
        dispatch({
          type: "error",
          message:
            error instanceof Error ? error.message : "Unable to load waveform.",
        });
      });

    return () => {
      disposed = true;
    };
  }, [entryPath, targetPeaks]);

  return state;
}
