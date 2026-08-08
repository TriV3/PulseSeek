import { useEffect, useReducer, useRef } from "react";
import { getWaveform, type WaveformLevel } from "../api/waveform";
import { onWaveformReady } from "../api/playbackEvents";

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
  | { type: "loading"; preserveWaveform: boolean }
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
      return {
        status: "loading",
        waveform: action.preserveWaveform ? _state.waveform : null,
        error: null,
      };
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
  const requestedEntryPath = useRef<string | null>(null);

  useEffect(() => {
    if (!entryPath || targetPeaks <= 0) {
      requestId.current += 1;
      requestedEntryPath.current = null;
      dispatch({ type: "reset" });
      return;
    }

    const preserveWaveform = requestedEntryPath.current === entryPath;
    requestedEntryPath.current = entryPath;
    dispatch({ type: "loading", preserveWaveform });
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const load = () => {
      const current = ++requestId.current;
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
              error instanceof Error
                ? error.message
                : "Unable to load waveform.",
          });
        });
    };

    void Promise.resolve(
      onWaveformReady((payload) => {
        if (disposed || payload.path !== entryPath) return;
        dispatch({ type: "loading", preserveWaveform: true });
        load();
      }),
    )
      .then((cleanup) => {
        if (disposed) cleanup();
        else {
          unlisten = cleanup;
          load();
        }
      })
      .catch(() => {
        if (!disposed) load();
      });

    return () => {
      disposed = true;
      requestId.current += 1;
      unlisten?.();
    };
  }, [entryPath, targetPeaks]);

  return state;
}
